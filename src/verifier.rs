//! One isolated Lean module per problem, with transitive axiom auditing.
use crate::{
    generator::{self, GeneratedProblem, RecurrenceKind},
    math::{Expr, NatExpr, Rational},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fmt, fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const LEAN_VERSION: &str = "4.19.0";
const INPUTS: &[&str] = &[
    "lean-toolchain",
    "lakefile.toml",
    "lake-manifest.json",
    "Recurrence.lean",
    "Recurrence/Theory.lean",
    "Audit.lean",
    ".lake/build/lib/lean/Recurrence/Theory.olean",
    ".prepared.version",
    ".prepared.audit",
];
#[derive(Clone)]
pub struct Verifier {
    directory: PathBuf,
    lake: PathBuf,
}
#[derive(Clone, Serialize)]
pub struct VerificationEnvironment {
    pub ready: bool,
    pub version: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerificationRecord {
    pub status: String,
    pub lean_version: String,
    pub proof_hash: String,
    pub problem_hash: String,
    pub environment_hash: String,
    pub rules_version: String,
    pub elapsed_ms: u128,
    pub axioms: Vec<String>,
    pub source_path: String,
    pub exit_code: i32,
    pub checked_theorems: Vec<String>,
}
#[derive(Debug)]
pub enum VerificationError {
    Failed(String),
    Unavailable(String),
    Timeout,
    Cancelled,
}
impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(e) => write!(f, "Lean検証に失敗しました: {e}"),
            Self::Unavailable(e) => write!(f, "検証環境を準備してください: {e}"),
            Self::Timeout => write!(f, "Lean検証が時間上限に達しました。"),
            Self::Cancelled => write!(f, "生成を中止しました。"),
        }
    }
}
impl std::error::Error for VerificationError {}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}
impl Verifier {
    pub fn new(directory: PathBuf) -> Self {
        let lake = std::env::var_os("RECURRENCE_LAKE")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".elan/bin/lake")))
            .filter(|p| p.is_file())
            .unwrap_or_else(|| PathBuf::from("lake"));
        Self { directory, lake }
    }
    fn environment_hash(&self) -> Result<String, String> {
        let input = fs::read_to_string(self.directory.join(".prepared-inputs")).map_err(|_| {
            "初回準備を実行してください（bash scripts/setup-lean.sh）。".to_string()
        })?;
        let entries: Vec<_> = input.lines().filter_map(|l| l.split_once("  ")).collect();
        if entries.len() != INPUTS.len() {
            return Err("準備記録が不完全です。再度準備してください。".into());
        }
        for name in INPUTS {
            let recorded = entries
                .iter()
                .find(|(_, p)| p == name)
                .ok_or_else(|| format!("準備記録に{name}がありません。"))?
                .0;
            let bytes = fs::read(self.directory.join(name))
                .map_err(|_| format!("{name}が見つかりません。"))?;
            if digest(bytes) != recorded {
                return Err(format!(
                    "{name}が準備時から変更されました。再度準備してください。"
                ));
            }
        }
        let version = fs::read_to_string(self.directory.join(".prepared.version"))
            .map_err(|e| e.to_string())?;
        if !version.contains("version 4.19.0,") {
            return Err("Leanの版が4.19.0と一致しません。".into());
        }
        let audit = fs::read_to_string(self.directory.join(".prepared.audit"))
            .map_err(|e| e.to_string())?;
        audit_axioms(
            &audit,
            &[
                "Recurrence.first_unique",
                "Recurrence.second_unique",
                "Recurrence.weighted_sum_unique",
            ],
        )?;
        Ok(digest(input))
    }
    pub fn status(&self) -> VerificationEnvironment {
        match self.environment_hash() {
            Ok(_) => VerificationEnvironment {
                ready: true,
                version: LEAN_VERSION.into(),
                message: "Lean 4.19.0と固定した共通モジュールの準備が完了しています。".into(),
            },
            Err(message) => VerificationEnvironment {
                ready: false,
                version: LEAN_VERSION.into(),
                message,
            },
        }
    }
    pub fn verify(
        &self,
        problem: &GeneratedProblem,
        cancel: Arc<AtomicBool>,
        timeout: Duration,
    ) -> Result<VerificationRecord, VerificationError> {
        let started = Instant::now();
        let environment_hash = self
            .environment_hash()
            .map_err(VerificationError::Unavailable)?;
        // Regenerate the entire IR; a successful old record cannot bless modified content.
        let replayed = generator::replay(&problem.recipe).map_err(VerificationError::Failed)?;
        if replayed.problem != problem.problem
            || replayed.id != problem.id
            || generator::content_hash_for_version(&problem.problem, &problem.recipe.rules_version)
                .map_err(VerificationError::Failed)?
                != problem.id
        {
            return Err(VerificationError::Failed(
                "Recipeと問題の内容が一致しません。".into(),
            ));
        }
        let source = proof_source(problem).map_err(VerificationError::Failed)?;
        let proof_hash = digest(source.as_bytes());
        let work_dir = self
            .directory
            .join("generated")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&work_dir).map_err(|e| VerificationError::Failed(e.to_string()))?;
        let source_path = work_dir.join("Problem.lean");
        fs::write(&source_path, &source).map_err(|e| VerificationError::Failed(e.to_string()))?;
        let log_path = work_dir.join("verification.log");
        let log =
            fs::File::create(&log_path).map_err(|e| VerificationError::Failed(e.to_string()))?;
        let mut command = Command::new(&self.lake);
        command
            .current_dir(&self.directory)
            .args(["env", "lean"])
            .arg(&source_path)
            .stdin(Stdio::null())
            .stdout(
                log.try_clone()
                    .map_err(|e| VerificationError::Failed(e.to_string()))?,
            )
            .stderr(log);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(VerificationError::Cancelled);
        }
        let mut child = command
            .spawn()
            .map_err(|e| VerificationError::Unavailable(e.to_string()))?;
        let process = child.id();
        let exit = loop {
            if cancel.load(Ordering::SeqCst) || started.elapsed() >= timeout {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(process as i32), libc::SIGKILL);
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err(if cancel.load(Ordering::SeqCst) {
                    VerificationError::Cancelled
                } else {
                    VerificationError::Timeout
                });
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(25)),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(VerificationError::Failed(e.to_string()));
                }
            }
        };
        let output =
            fs::read_to_string(&log_path).map_err(|e| VerificationError::Failed(e.to_string()))?;
        if !exit.success() {
            return Err(VerificationError::Failed(
                output.chars().take(6000).collect(),
            ));
        }
        if !output.lines().any(|line| line.trim() == "\"4.19.0\"") {
            return Err(VerificationError::Failed(
                "実行したLeanの版が4.19.0と一致しません。".into(),
            ));
        }
        let checked = [
            "GeneratedProblem.problem_valid",
            "GeneratedProblem.problem_recurrence",
            "GeneratedProblem.problem_unique",
            "GeneratedProblem.problem_conditions",
        ];
        let axioms = audit_axioms(&output, &checked).map_err(VerificationError::Failed)?;
        if cancel.load(Ordering::SeqCst) {
            return Err(VerificationError::Cancelled);
        }
        if self
            .environment_hash()
            .map_err(VerificationError::Unavailable)?
            != environment_hash
        {
            return Err(VerificationError::Failed(
                "検証中に共通モジュールが変更されました。".into(),
            ));
        }
        Ok(VerificationRecord {
            status: "success".into(),
            lean_version: LEAN_VERSION.into(),
            proof_hash,
            problem_hash: problem.id.clone(),
            environment_hash,
            rules_version: problem.recipe.rules_version.clone(),
            elapsed_ms: started.elapsed().as_millis(),
            axioms,
            source_path: source_path.to_string_lossy().into(),
            exit_code: exit.code().unwrap_or(-1),
            checked_theorems: checked.iter().map(|x| x.to_string()).collect(),
        })
    }
}
fn audit_axioms(output: &str, required: &[&str]) -> Result<Vec<String>, String> {
    let mut axioms = BTreeSet::new();
    for name in required {
        if !output.lines().any(|line| {
            line.contains(&format!("'{name}'"))
                && (line.contains("depends on axioms:")
                    || line.contains("does not depend on any axioms"))
        }) {
            return Err(format!("必須の定理{name}の公理監査結果がありません。"));
        }
    }
    for line in output.lines() {
        if let Some((_, list)) = line.split_once("depends on axioms:") {
            for axiom in list
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(str::trim)
                .filter(|a| !a.is_empty())
            {
                if !["propext", "Classical.choice", "Quot.sound"].contains(&axiom) {
                    return Err(format!("未許可の公理依存: {axiom}"));
                }
                axioms.insert(axiom.to_string());
            }
        }
        if line.contains("sorryAx") || line.contains("declaration uses 'sorry'") {
            return Err("未完成の証明を検出しました。".into());
        }
    }
    Ok(axioms.into_iter().collect())
}
// Public n>=1 is translated to Lean's n+1. A sum from k=1 to n+1 is
// translated by the bijection k=j+1 to range(n+1), preserving its boundary.
pub(crate) fn natural(value: &NatExpr, bound: Option<&str>) -> Result<String, String> {
    Ok(match value {
        NatExpr::Index => "(n + 1)".into(),
        NatExpr::Constant { value } => value.to_string(),
        NatExpr::Bound { name } if bound == Some(name) => format!("({name} + 1)"),
        NatExpr::Bound { .. } => return Err("未登録の束縛変数です。".into()),
        NatExpr::Add { left, right } => {
            format!("({} + {})", natural(left, bound)?, natural(right, bound)?)
        }
        NatExpr::Sub { left, right }
            if **left == NatExpr::Index && **right == NatExpr::constant(1) =>
        {
            "n".into()
        }
        NatExpr::Sub { left, right } => {
            format!("({} - {})", natural(left, bound)?, natural(right, bound)?)
        }
        NatExpr::Mul { left, right } => {
            format!("({} * {})", natural(left, bound)?, natural(right, bound)?)
        }
        NatExpr::Div {
            numerator,
            denominator,
        } => format!(
            "({} / {})",
            natural(numerator, bound)?,
            natural(denominator, bound)?
        ),
        NatExpr::Pow { base, exponent } => format!("({} ^ {exponent})", natural(base, bound)?),
        NatExpr::Choose { n, k } => format!("(Nat.choose {} {k})", natural(n, bound)?),
    })
}
pub(crate) fn render(
    expr: &Expr,
    sequence: &str,
    bound: Option<&str>,
    step: bool,
) -> Result<String, String> {
    Ok(match expr {
        Expr::Rational { value } => value.lean(),
        Expr::NatCast { value } => format!("({} : ℚ)", natural(value, bound)?),
        Expr::Term { sequence: s, index } => {
            if s != "a" && s != "b" {
                return Err("未登録の数列です。".into());
            }
            if step && *index == NatExpr::Index {
                if s == "a" {
                    "x".into()
                } else {
                    "y".into()
                }
            } else {
                let idx = match index {
                    NatExpr::Index => "n".into(),
                    NatExpr::Constant { value } => value.saturating_sub(1).to_string(),
                    NatExpr::Add { left, right } if **left == NatExpr::Index => {
                        format!("(n + {})", natural(right, bound)?)
                    }
                    NatExpr::Bound { name } if bound == Some(name) => name.clone(),
                    _ => format!("({} - 1)", natural(index, bound)?),
                };
                let mapped = if s == "a" {
                    sequence
                } else if sequence == "f" {
                    "g"
                } else {
                    "b"
                };
                format!("({mapped} {idx})")
            }
        }
        Expr::Add { terms } => format!(
            "({})",
            terms
                .iter()
                .map(|e| render(e, sequence, bound, step))
                .collect::<Result<Vec<_>, _>>()?
                .join(" + ")
        ),
        Expr::Mul { factors } => format!(
            "({})",
            factors
                .iter()
                .map(|e| render(e, sequence, bound, step))
                .collect::<Result<Vec<_>, _>>()?
                .join(" * ")
        ),
        Expr::Div {
            numerator,
            denominator,
        } => format!(
            "({} / {})",
            render(numerator, sequence, bound, step)?,
            render(denominator, sequence, bound, step)?
        ),
        Expr::Pow { base, exponent } => format!(
            "({} ^ {})",
            render(base, sequence, bound, step)?,
            natural(exponent, bound)?
        ),
        Expr::Factorial { index } => format!("(Nat.factorial {} : ℚ)", natural(index, bound)?),
        Expr::Log { .. } => return Err("対数の補助数列は有理数の証明文に使用できません。".into()),
        Expr::Sum {
            variable,
            lower,
            upper,
            body,
        } if *lower == NatExpr::constant(1) && variable == "k" => {
            format!(
                "(∑ k ∈ Finset.range {}, {})",
                natural(upper, bound)?,
                render(body, sequence, Some(variable), step)?
            )
        }
        Expr::Sum { .. } => return Err("未登録の有限和の境界です。".into()),
    })
}
pub fn proof_source(problem: &GeneratedProblem) -> Result<String, String> {
    if let Some(source) = crate::proofs::source(problem)? {
        return Ok(source);
    }
    let p = &problem.problem;
    let params = &problem.recipe.parameters;
    let formula = render(&p.general_term.rhs, "f", None, false)?;
    let initial = render(&p.initials[0].rhs, "f", None, false)?;
    let recurrence_f = render(&p.recurrence.rhs, "f", None, false)?;
    let recurrence_a = render(&p.recurrence.rhs, "a", None, false)?;
    let mut source=format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- public a₁ is f 0; public n is Lean n+1\ndef f (n : ℕ) : ℚ := {formula}\n\n",problem.id);
    match p.kind {
        RecurrenceKind::FirstOrder => {
            let step = render(&p.recurrence.rhs, "a", None, true)?;
            source.push_str(&format!("def step (n : ℕ) (x : ℚ) : ℚ := {step}\n\ntheorem problem_valid : Recurrence.FirstCertificate f {initial} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n"));
            if problem.recipe.family == "reciprocal_affine" {
                let Expr::Div { denominator, .. } = &p.general_term.rhs else {
                    return Err("逆数型の一般項が登録規則と一致しません。".into());
                };
                let den = render(denominator, "f", None, false)?;
                let den_k = den.replace("(n : ℚ)", "(k : ℚ)");
                let Expr::Div {
                    denominator: original,
                    ..
                } = &p.recurrence.rhs
                else {
                    return Err("逆数型の漸化式が登録規則と一致しません。".into());
                };
                let original = render(original, "f", None, false)?;
                source.push_str(&format!("    have hp : 0 < f n := by\n      dsimp [f]\n      exact div_pos (by norm_num) (by positivity)\n    have hd (k : ℕ) : {den_k} ≠ 0 := by positivity\n    have ho : {original} ≠ 0 := by positivity\n    simp only [f, Nat.cast_add, Nat.cast_one] at ho\n    simp only [f, step, Nat.cast_add, Nat.cast_one]\n    field_simp [hd n, hd (n + 1), ho] <;> ring\n"));
            } else {
                source.push_str("    simp only [f, step, Nat.cast_add, Nat.cast_one, pow_succ, pow_add, Nat.factorial_succ, Nat.cast_mul]\n");
                if problem.recipe.family == "scaled_constant" {
                    source.push_str("    have hn : (n : ℚ) + 1 ≠ 0 := by positivity\n");
                    source.push_str("    field_simp [hn] <;> ring\n");
                } else {
                    source.push_str("    ring\n");
                }
            }
            source.push_str(&format!("\ntheorem problem_recurrence (n : ℕ) : f (n + 1) = {recurrence_f} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial})\n    (hs : ∀ n, a (n + 1) = {recurrence_a}) : ∀ n, a n = f n :=\n  Recurrence.first_unique problem_valid ⟨hi, hs⟩\n"));
        }
        RecurrenceKind::SecondOrder => {
            let second = render(&p.initials[1].rhs, "f", None, false)?;
            let coeff = params["r"].add(&params["s"]).lean();
            let q = Rational::integer(-1)
                .mul(&params["r"])
                .mul(&params["s"])
                .lean();
            source.push_str(&format!("theorem problem_valid : Recurrence.SecondCertificate f {initial} {second} {coeff} {q} := by\n  constructor\n  · norm_num [f]\n  · norm_num [f]\n  · intro n\n    simp only [f, pow_succ, pow_add]\n    ring\n\ntheorem problem_recurrence (n : ℕ) : f (n + 2) = {recurrence_f} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial}) (hj : a 1 = {second})\n    (hs : ∀ n, a (n + 2) = {recurrence_a}) : ∀ n, a n = f n :=\n  Recurrence.second_unique problem_valid ⟨hi, hj, hs⟩\n"));
        }
        RecurrenceKind::WeightedSum => {
            let c = &params["c"];
            let r = &params["r"];
            let s = &params["s"];
            let alpha = r.mul(s);
            let beta = r.add(s).sub(&Rational::integer(1)).sub(&alpha);
            let aa = c.mul(&r.sub(&Rational::integer(1))).div(&r.sub(s))?;
            let bb = c.mul(&Rational::integer(1).sub(s)).div(&r.sub(s))?;
            let b = format!(
                "{} * {} ^ n + {} * {} ^ n",
                aa.lean(),
                r.lean(),
                bb.lean(),
                s.lean()
            );
            let second = alpha.add(&beta).mul(c).lean();
            let alpha = alpha.lean();
            let beta = beta.lean();
            source.push_str(&format!("def b (n : ℕ) : ℚ := {b}\ndef g (n : ℕ) : ℚ := (n + 1 : ℕ)\n\ntheorem b_valid : Recurrence.SecondCertificate b {initial} {second} ({alpha} + {beta} + 1) (-{alpha}) := by\n  constructor\n  · norm_num [b]\n  · norm_num [b]\n  · intro n\n    simp only [b, pow_succ, pow_add]\n    ring\n\ntheorem problem_valid : Recurrence.WeightedSumCertificate f {initial} g {alpha} {beta} := by\n  apply Recurrence.weighted_sum_from_second f b g {initial} {initial} {second} {alpha} {beta}\n  · exact b_valid\n  · norm_num\n  · intro n; simp only [g, Nat.cast_add, Nat.cast_one]; positivity\n  · intro n; simp only [f, g, b, Nat.cast_add, Nat.cast_one] <;> ring\n  · norm_num [f]\n\ntheorem problem_recurrence (n : ℕ) : f (n + 1) = {recurrence_f} := by\n  simpa only [g, Nat.cast_add, Nat.cast_one] using problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial})\n    (hs : ∀ n, a (n + 1) = {recurrence_a}) : ∀ n, a n = f n := by\n  apply Recurrence.weighted_sum_unique problem_valid\n  refine ⟨hi, problem_valid.scale_ne_zero, ?_⟩\n  intro n\n  simpa only [g, Nat.cast_add, Nat.cast_one] using hs n\n"));
        }
        _ => return Err("この型の証明生成器が登録されていません。".into()),
    }
    let conditions = p
        .conditions
        .iter()
        .map(|c| {
            let expression = render(&c.expression, "f", None, false)?;
            match c.kind.as_str() {
                "nonzero" => Ok(format!("{expression} ≠ 0")),
                "positive" => Ok(format!("0 < {expression}")),
                _ => Err(format!("未登録の定義域条件です: {}", c.kind)),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    if conditions.is_empty() {
        source.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        source.push_str(&format!(
            "\ntheorem problem_conditions (n : ℕ) : {} := by\n",
            conditions.join(" ∧ ")
        ));
        if problem.recipe.family == "reciprocal_affine" {
            source.push_str("  have hp : 0 < f n := by\n    dsimp [f]\n    exact div_pos (by norm_num) (by positivity)\n  constructor\n  · positivity\n  · exact ne_of_gt hp\n");
        } else if conditions.len() > 1 {
            source.push_str("  constructor\n  all_goals\n    try simp only [f, Nat.cast_add, Nat.cast_one]\n    positivity\n");
        } else {
            source.push_str("  try simp only [f, Nat.cast_add, Nat.cast_one]\n  positivity\n");
        }
    }
    for theorem in [
        "problem_valid",
        "problem_recurrence",
        "problem_unique",
        "problem_conditions",
    ] {
        source.push_str(&format!("\n#print axioms {theorem}\n"));
    }
    source.push_str("\nend GeneratedProblem\n");
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incomplete_or_custom_axiom_is_rejected() {
        assert!(audit_axioms("'x' depends on axioms: [sorryAx]", &["x"]).is_err());
        assert!(audit_axioms("'x' depends on axioms: [evil]", &["x"]).is_err());
        assert!(audit_axioms(
            "'x' depends on axioms: [propext, Classical.choice, Quot.sound]",
            &["x"]
        )
        .is_ok());
        assert!(audit_axioms("", &["x"]).is_err());
    }
    #[test]
    fn lean_proofs_use_problem_formula_and_recurrence() {
        for family in generator::family_catalog() {
            let p = generator::generate(family.levels[0], Some(&family.id), 11).unwrap();
            let proof = proof_source(&p).unwrap();
            assert!(proof.contains(&render(&p.problem.general_term.rhs, "f", None, false).unwrap()));
            assert!(proof.contains(&render(&p.problem.recurrence.rhs, "a", None, false).unwrap()));
            assert!(!proof.contains("sorry"));
        }
    }
}
