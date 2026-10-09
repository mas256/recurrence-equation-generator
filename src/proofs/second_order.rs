//! Kernel-checked universal certificates for additive second-order problems.
use crate::generator::GeneratedProblem;
use crate::math::{Expr, NatExpr};

fn render(e: &Expr, sequence: &str, step: bool) -> Result<String, String> {
    Ok(match e {
        Expr::Term { sequence: s, index } if s == "a" => match index {
            NatExpr::Index => {
                if step {
                    "x".into()
                } else {
                    format!("({sequence} n)")
                }
            }
            NatExpr::Add { left, right } if **left == NatExpr::Index => {
                let NatExpr::Constant { value } = **right else {
                    return Err("3項間の添字が定数ではありません。".into());
                };
                if step {
                    if value == 1 {
                        "y".into()
                    } else {
                        return Err("次々項が右辺に含まれています。".into());
                    }
                } else {
                    format!("({sequence} (n + {value}))")
                }
            }
            _ => return Err("未登録の3項間の添字です。".into()),
        },
        Expr::Add { terms } => format!(
            "({})",
            terms
                .iter()
                .map(|x| render(x, sequence, step))
                .collect::<Result<Vec<_>, _>>()?
                .join(" + ")
        ),
        Expr::Mul { factors } => format!(
            "({})",
            factors
                .iter()
                .map(|x| render(x, sequence, step))
                .collect::<Result<Vec<_>, _>>()?
                .join(" * ")
        ),
        Expr::Div {
            numerator,
            denominator,
        } => format!(
            "({} / {})",
            render(numerator, sequence, step)?,
            render(denominator, sequence, step)?
        ),
        Expr::Pow { base, exponent } => {
            format!("({} ^ {})", render(base, sequence, step)?, exponent.lean())
        }
        Expr::Term { .. } | Expr::Sum { .. } => return Err("未登録の数列を含む証明式です。".into()),
        _ => e.lean(),
    })
}
fn lhs_coefficient(lhs: &Expr) -> Result<Expr, String> {
    if *lhs == Expr::term("a", 2) {
        return Ok(Expr::integer(1));
    }
    if let Expr::Mul { factors } = lhs {
        let terms = factors.iter().filter(|f| **f == Expr::term("a", 2)).count();
        if terms == 1 {
            return Ok(Expr::mul(
                factors
                    .iter()
                    .filter(|f| **f != Expr::term("a", 2))
                    .cloned()
                    .collect(),
            ));
        }
    }
    Err("次々項の一次係数を検出できません。".into())
}
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !crate::extensions::second_order::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    let formula = render(&p.general_term.rhs, "f", false)?;
    let initial = render(&p.initials[0].rhs, "f", false)?;
    let second = render(&p.initials[1].rhs, "f", false)?;
    let coefficient = render(&lhs_coefficient(&p.recurrence.lhs)?, "f", false)?;
    let step = render(&p.recurrence.rhs, "a", true)?;
    let lhs_f = render(&p.recurrence.lhs, "f", false)?;
    let lhs_a = render(&p.recurrence.lhs, "a", false)?;
    let rhs_f = render(&p.recurrence.rhs, "f", false)?;
    let rhs_a = render(&p.recurrence.rhs, "a", false)?;
    let mut s=format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- public a₁ is f 0; public n is Lean n+1\ndef f (n : ℕ) : ℚ := {formula}\ndef step (n : ℕ) (x y : ℚ) : ℚ := {step} / {coefficient}\n\ntheorem coefficient_nonzero (n : ℕ) : {coefficient} ≠ 0 := by\n  try simp only [Nat.cast_add, Nat.cast_one]\n  have hn : (0 : ℚ) ≤ (n : ℚ) := Nat.cast_nonneg n\n  have hp : (0 : ℚ) < {coefficient} := by\n    try simp only [Nat.cast_add, Nat.cast_one]\n    nlinarith [sq_nonneg (n : ℚ)]\n  simpa only [Nat.cast_add, Nat.cast_one] using ne_of_gt hp\n\ntheorem problem_valid : Recurrence.GeneralSecondCertificate f {initial} {second} step := by\n  constructor\n  · norm_num [f]\n  · norm_num [f]\n  · intro n\n    have hp := coefficient_nonzero n\n    try simp only [Nat.cast_add, Nat.cast_one] at hp\n    simp only [f, step, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    field_simp [hp] <;> ring\n\ntheorem problem_recurrence (n : ℕ) : {lhs_f} = {rhs_f} := by\n  have hv := problem_valid.recurrence n\n  dsimp [step] at hv\n  have he := (eq_div_iff (coefficient_nonzero n)).mp hv\n  simpa only [mul_comm, div_one, mul_one] using he\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial}) (hj : a 1 = {second})\n    (hs : ∀ (n : ℕ), {lhs_a} = {rhs_a}) : ∀ n, a n = f n := by\n  apply Recurrence.general_second_unique problem_valid\n  refine ⟨hi, hj, ?_⟩\n  intro n\n  dsimp [step]\n  apply (eq_div_iff (coefficient_nonzero n)).2\n  simpa only [mul_comm, div_one, mul_one] using hs n\n",problem.id);
    let conditions = p
        .conditions
        .iter()
        .map(|c| {
            let expression = render(&c.expression, "f", false)?;
            match c.kind.as_str() {
                "nonzero" => Ok(format!("{expression} ≠ 0")),
                "positive" => Ok(format!("0 < {expression}")),
                _ => Err("未登録の3項間の定義域条件です。".into()),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if conditions.is_empty() {
        s.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        s.push_str(&format!("\ntheorem problem_conditions (n : ℕ) : {} := by\n  try simp only [Nat.cast_add, Nat.cast_one]\n  have hn : (0 : ℚ) ≤ (n : ℚ) := Nat.cast_nonneg n\n  {}\n  all_goals\n    try apply ne_of_gt\n    nlinarith [sq_nonneg (n : ℚ)]\n",conditions.join(" ∧ "),if conditions.len()==1{"skip"}else{"constructor"}));
    }
    for name in [
        "problem_valid",
        "problem_recurrence",
        "problem_unique",
        "problem_conditions",
    ] {
        s.push_str(&format!("\n#print axioms {name}\n"));
    }
    s.push_str("\nend GeneratedProblem\n");
    Ok(Some(s))
}

#[cfg(test)]
mod tests {
    use crate::math::Rational;
    use std::{
        collections::BTreeMap,
        path::PathBuf,
        sync::{atomic::AtomicBool, Arc},
        time::Duration,
    };
    #[test]
    #[ignore = "requires prepared Lean 4.19 + mathlib"]
    fn all_second_order_profiles_have_universal_lean_certificates() {
        let verifier =
            crate::verifier::Verifier::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean"));
        assert!(verifier.status().ready, "{}", verifier.status().message);
        for family in crate::extensions::second_order::catalog() {
            for profile in if family.id == "scaled_second_order" || family.id == "difference_scaled"
            {
                0..=2
            } else {
                0..=0
            } {
                let mut p: BTreeMap<String, Rational> = [
                    ("c".into(), Rational::fraction(1, 2)),
                    ("d".into(), Rational::integer(1)),
                ]
                .into_iter()
                .collect();
                if family.id == "arithmetic_difference" {
                    p.insert("s".into(), Rational::integer(3));
                } else {
                    p.insert("r".into(), Rational::integer(2));
                    if family.id != "difference_scaled" {
                        p.insert("s".into(), Rational::integer(3));
                    }
                }
                if matches!(
                    family.id.as_str(),
                    "scaled_second_order" | "difference_scaled"
                ) {
                    p.insert("profile".into(), Rational::integer(profile));
                    p.insert("profile_version".into(), Rational::integer(1));
                }
                let recipe =
                    crate::extensions::second_order::build_recipe(&family.id, 31, p).unwrap();
                let generated = crate::generator::replay(&recipe).unwrap();
                let record = verifier
                    .verify(
                        &generated,
                        Arc::new(AtomicBool::new(false)),
                        Duration::from_secs(60),
                    )
                    .unwrap_or_else(|e| panic!("{} profile{}: {}", family.id, profile, e));
                assert_eq!(record.checked_theorems.len(), 4);
                println!(
                    "{} profile{}: Lean verified {} ms",
                    family.id, profile, record.elapsed_ms
                );
            }
        }
    }
}
