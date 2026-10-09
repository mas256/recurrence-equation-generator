//! Full-index certificates for both members of a rational recurrence system.
use crate::generator::{GeneratedProblem, RecurrenceKind};
use crate::math::{Expr, NatExpr};
use crate::verifier::render;

fn leading(expr: &Expr) -> Result<Expr, String> {
    Ok(match expr {
        Expr::Term { sequence, index } if sequence == "a" && *index == NatExpr::offset(1) => {
            Expr::integer(1)
        }
        Expr::Mul { factors } => {
            Expr::mul(factors.iter().map(leading).collect::<Result<Vec<_>, _>>()?)
        }
        Expr::Rational { .. } | Expr::NatCast { .. } | Expr::Add { .. } => expr.clone(),
        _ => return Err("連立の左辺の倍率を検出できません。".into()),
    })
}
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !crate::extensions::systems::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    if p.kind != RecurrenceKind::System
        || p.secondary_recurrences.len() != 1
        || p.secondary_general_terms.len() != 1
        || p.initials.len() != 2
    {
        return Err("連立の証明に必要な2数列のデータが不足しています。".into());
    }
    let formula_a = render(&p.general_term.rhs, "f", None, false)?;
    let formula_b = render(&p.secondary_general_terms[0].rhs, "f", None, false)?;
    let first = render(&p.initials[0].rhs, "f", None, false)?;
    let second = render(&p.initials[1].rhs, "f", None, false)?;
    let lead = render(&leading(&p.recurrence.lhs)?, "f", None, false)?;
    let step_a = render(&p.recurrence.rhs, "a", None, true)?;
    let step_b = render(&p.secondary_recurrences[0].rhs, "a", None, true)?;
    let lhs_f = render(&p.recurrence.lhs, "f", None, false)?;
    let lhs_g = render(&p.secondary_recurrences[0].lhs, "f", None, false)?;
    let rhs_f = render(&p.recurrence.rhs, "f", None, false)?;
    let rhs_g = render(&p.secondary_recurrences[0].rhs, "f", None, false)?;
    let lhs_a = render(&p.recurrence.lhs, "a", None, false)?;
    let lhs_b = render(&p.secondary_recurrences[0].lhs, "a", None, false)?;
    let rhs_a = render(&p.recurrence.rhs, "a", None, false)?;
    let rhs_b = render(&p.secondary_recurrences[0].rhs, "a", None, false)?;
    let mut out=format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- Lean index 0 represents the public terms a₁ and b₁.\ndef f (n : ℕ) : ℚ := {formula_a}\ndef g (n : ℕ) : ℚ := {formula_b}\ndef lead (n : ℕ) : ℚ := {lead}\ndef stepA (n : ℕ) (x y : ℚ) : ℚ := {step_a} / lead n\ndef stepB (n : ℕ) (x y : ℚ) : ℚ := {step_b} / lead n\n\ntheorem lead_ne_zero (n : ℕ) : lead n ≠ 0 := by\n  dsimp [lead]\n  try simp only [Nat.cast_add, Nat.cast_one]\n  positivity\n\ntheorem problem_valid : Recurrence.SystemCertificate f g {first} {second} stepA stepB := by\n  constructor\n  · norm_num [f]\n  · norm_num [g]\n",problem.id);
    for step in ["stepA", "stepB"] {
        out.push_str(&format!("  · intro n\n    dsimp [{step}]\n    apply (eq_div_iff (lead_ne_zero n)).2\n    simp only [f, g, lead, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    norm_num <;> ring\n"));
    }
    out.push_str(&format!("\ntheorem problem_recurrence (n : ℕ) :\n    {lhs_f} = {rhs_f} ∧ {lhs_g} = {rhs_g} := by\n  constructor\n  · have h := problem_valid.recurrence_a n\n    dsimp [stepA] at h\n    have hc := (eq_div_iff (lead_ne_zero n)).mp h\n    simpa only [lead, mul_comm, div_one, mul_one] using hc\n  · have h := problem_valid.recurrence_b n\n    dsimp [stepB] at h\n    have hc := (eq_div_iff (lead_ne_zero n)).mp h\n    simpa only [lead, mul_comm, div_one, mul_one] using hc\n\ntheorem problem_unique (a b : ℕ → ℚ)\n    (hi : a 0 = {first}) (hj : b 0 = {second})\n    (ha : ∀ (n : ℕ), {lhs_a} = {rhs_a})\n    (hb : ∀ (n : ℕ), {lhs_b} = {rhs_b}) : ∀ n, a n = f n ∧ b n = g n := by\n  apply Recurrence.system_unique problem_valid\n  refine ⟨hi, hj, ?_, ?_⟩\n  · intro n\n    dsimp [stepA]\n    apply (eq_div_iff (lead_ne_zero n)).2\n    simpa only [lead, mul_comm, div_one, mul_one] using ha n\n  · intro n\n    dsimp [stepB]\n    apply (eq_div_iff (lead_ne_zero n)).2\n    simpa only [lead, mul_comm, div_one, mul_one] using hb n\n"));
    let conditions = p
        .conditions
        .iter()
        .map(|c| {
            let expr = render(&c.expression, "f", None, false)?;
            match c.kind.as_str() {
                "nonzero" => Ok(format!("{expr} ≠ 0")),
                "positive" => Ok(format!("0 < {expr}")),
                _ => Err("連立の定義域条件が未登録です。".into()),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if conditions.is_empty() {
        out.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        out.push_str(&format!(
            "\ntheorem problem_conditions (n : ℕ) : {} := by\n",
            conditions.join(" ∧ ")
        ));
        if conditions.len() > 1 {
            out.push_str("  constructor\n  all_goals\n    try simp only [f, g, Nat.cast_add, Nat.cast_one]\n    positivity\n");
        } else {
            out.push_str("  try simp only [f, g, Nat.cast_add, Nat.cast_one]\n  positivity\n");
        }
    }
    for theorem in [
        "problem_valid",
        "problem_recurrence",
        "problem_unique",
        "problem_conditions",
    ] {
        out.push_str(&format!("\n#print axioms {theorem}\n"));
    }
    out.push_str("\nend GeneratedProblem\n");
    Ok(Some(out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator;
    use crate::math::Rational;
    use std::{collections::BTreeMap, fs, path::PathBuf, process::Command};

    #[test]
    #[ignore = "requires Lean 4.19 and built Recurrence.Theory"]
    fn all_system_profiles_have_real_lean_certificates() {
        let lean = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean");
        let temporary =
            std::env::temp_dir().join(format!("recurrence-systems-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temporary).unwrap();
        for (family, profiles) in [
            ("coupled_symmetric", vec![0]),
            ("coupled_weighted", vec![0]),
            ("coupled_scaled", vec![0, 1, 2]),
            ("coupled_forced", vec![0, 1]),
        ] {
            for profile in profiles {
                let parameters: BTreeMap<_, _> =
                    [("r", 2), ("s", 3), ("c", 2), ("d", 4), ("profile", profile)]
                        .into_iter()
                        .map(|(k, v)| (k.to_string(), Rational::integer(v)))
                        .collect();
                let recipe =
                    crate::extensions::systems::build_recipe(family, 42, parameters).unwrap();
                let p = generator::replay(&recipe).unwrap();
                let path = temporary.join(format!("{family}_{profile}.lean"));
                fs::write(&path, source(&p).unwrap().unwrap()).unwrap();
                let result = Command::new("lake")
                    .current_dir(&lean)
                    .args(["env", "lean"])
                    .arg(&path)
                    .output()
                    .unwrap();
                let stdout = String::from_utf8_lossy(&result.stdout);
                let stderr = String::from_utf8_lossy(&result.stderr);
                assert!(
                    result.status.success(),
                    "{family}/{profile}: {stdout}\n{stderr}\nsource: {}",
                    path.display()
                );
                for theorem in [
                    "problem_valid",
                    "problem_recurrence",
                    "problem_unique",
                    "problem_conditions",
                ] {
                    assert!(
                        stdout.contains(&format!("'GeneratedProblem.{theorem}'")),
                        "{family}/{profile}: missing {theorem} audit"
                    );
                }
                assert!(
                    !stdout.contains("sorryAx"),
                    "{family}/{profile}: incomplete proof"
                );
                println!("{family}/{profile}: both sequences Lean verified");
            }
        }
        fs::remove_dir_all(temporary).unwrap();
    }
}
