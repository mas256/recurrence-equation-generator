//! Universal natural-exponent proofs; no numeric sampling or logarithm axioms.
use crate::generator::{GeneratedProblem, RecurrenceKind};
use crate::math::{Expr, NatExpr};
use crate::verifier::render;

fn power_factor(expr: &Expr) -> Option<(&Expr, &NatExpr)> {
    match expr {
        Expr::Pow { base, exponent } if matches!(**base, Expr::Rational { .. }) => {
            Some((base, exponent))
        }
        Expr::Mul { factors } => factors.iter().find_map(power_factor),
        _ => None,
    }
}
fn exponent_proof(profile: u32, second: bool) -> &'static str {
    if !second {
        return match profile {
            0 => "  dsimp [E, F]\n  rw [Nat.choose_succ_succ' (n + 1) 1, Nat.choose_one_right]\n  norm_num only [Nat.add_assoc] at *\n  omega\n",
            1 => "  simp only [E, F, Nat.add_assoc, Nat.reduceAdd]\n  ring\n",
            _ => "  dsimp [E, F]\n  rw [Nat.choose_succ_succ' (n + 2) 2]\n  norm_num only [Nat.add_assoc] at *\n  omega\n",
        };
    }
    match profile {
        0 => "  have h1 : (n + 1).choose 2 = n + n.choose 2 := by\n    simpa only [Nat.reduceAdd, Nat.choose_one_right] using Nat.choose_succ_succ' n 1\n  have h2 : (n + 2).choose 2 = (n + 1) + (n + 1).choose 2 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd, Nat.choose_one_right] using Nat.choose_succ_succ' (n + 1) 1\n  simp only [E, F, Nat.add_assoc, Nat.reduceAdd]\n  rw [h2, h1]\n  ring\n",
        1 => "  have h1 : (n + 2).choose 3 = (n + 1).choose 2 + (n + 1).choose 3 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd] using Nat.choose_succ_succ' (n + 1) 2\n  have h2 : (n + 3).choose 3 = (n + 2).choose 2 + (n + 2).choose 3 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd] using Nat.choose_succ_succ' (n + 2) 2\n  have h3 : (n + 2).choose 2 = (n + 1) + (n + 1).choose 2 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd, Nat.choose_one_right] using Nat.choose_succ_succ' (n + 1) 1\n  simp only [E, F, Nat.add_assoc, Nat.reduceAdd]\n  rw [h2, h3, h1]\n  ring\n",
        _ => "  have h1 : (n + 2).choose 3 = (n + 1).choose 2 + (n + 1).choose 3 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd] using Nat.choose_succ_succ' (n + 1) 2\n  have h2 : (n + 3).choose 3 = (n + 2).choose 2 + (n + 2).choose 3 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd] using Nat.choose_succ_succ' (n + 2) 2\n  have h3 : (n + 2).choose 2 = (n + 1) + (n + 1).choose 2 := by\n    simpa only [Nat.add_assoc, Nat.reduceAdd, Nat.choose_one_right] using Nat.choose_succ_succ' (n + 1) 1\n  have h4 : (n + 1).choose 2 = n + n.choose 2 := by\n    simpa only [Nat.reduceAdd, Nat.choose_one_right] using Nat.choose_succ_succ' n 1\n  simp only [E, F, Nat.add_assoc, Nat.reduceAdd]\n  rw [h2, h3, h1, h4]\n  ring\n",
    }
}
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !crate::extensions::powers::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    let profile = crate::extensions::powers::profile(&problem.recipe.parameters)?;
    let (_, exponent) =
        power_factor(&p.general_term.rhs).ok_or("一般項の自然数指数を検出できません。")?;
    let (base, forcing) =
        power_factor(&p.recurrence.rhs).ok_or("漸化式の自然数指数を検出できません。")?;
    let base = render(base, "f", None, false)?;
    let formula = render(&p.general_term.rhs, "f", None, false)?;
    let initial = render(&p.initials[0].rhs, "f", None, false)?;
    let lhs_f = render(&p.recurrence.lhs, "f", None, false)?;
    let rhs_f = render(&p.recurrence.rhs, "f", None, false)?;
    let lhs_a = render(&p.recurrence.lhs, "a", None, false)?;
    let rhs_a = render(&p.recurrence.rhs, "a", None, false)?;
    let second = p.kind == RecurrenceKind::MultiplicativeSecond;
    let mut s = format!("import Recurrence.Theory\nimport Mathlib.Data.Nat.Choose.Basic\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- public a₁ is f 0; public n is Lean n+1\ndef E (n : ℕ) : ℕ := {}\ndef F (n : ℕ) : ℕ := {}\ndef f (n : ℕ) : ℚ := {formula}\n\ntheorem base_positive : (0 : ℚ) < {base} := by norm_num\n\n", problem.id, exponent.lean(), forcing.lean());
    if !second {
        let step = render(&p.recurrence.rhs, "a", None, true)?;
        s.push_str(&format!("def step (n : ℕ) (x : ℚ) : ℚ := {step}\n\ntheorem formula_eq (n : ℕ) : f n = {initial} * {base} ^ E n := by\n  simp only [f, E] <;> norm_num\n\ntheorem exponent_step (n : ℕ) : E (n + 1) = E n + F n := by\n{}\ntheorem problem_valid : Recurrence.FirstCertificate f {initial} step := by\n  constructor\n  · norm_num [f, Nat.choose]\n  · intro n\n    rw [formula_eq (n + 1)]\n    simp only [step, formula_eq]\n    change {initial} * {base} ^ E (n + 1) = {base} ^ F n * ({initial} * {base} ^ E n)\n    rw [exponent_step, pow_add]\n    ring\n\ntheorem problem_recurrence (n : ℕ) : {lhs_f} = {rhs_f} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial})\n    (hs : ∀ n, {lhs_a} = {rhs_a}) : ∀ n, a n = f n :=\n  Recurrence.first_unique problem_valid ⟨hi, hs⟩\n", exponent_proof(profile, false)));
    } else {
        let second_initial = render(&p.initials[1].rhs, "f", None, false)?;
        s.push_str(&format!("def step (n : ℕ) (x y : ℚ) : ℚ := {base} ^ F n * y ^ 2 / x\n\ntheorem exponent_step (n : ℕ) : E (n + 2) + E n = F n + 2 * E (n + 1) := by\n{}\ntheorem problem_valid : Recurrence.GeneralSecondCertificate f {initial} {second_initial} step := by\n  constructor\n  · norm_num [f, Nat.choose]\n  · norm_num [f, Nat.choose]\n  · intro n\n    have h := (Recurrence.multiplicative_certificate {base} E F base_positive exponent_step).1.recurrence n\n    simpa only [f, step, E, F] using h\n\ntheorem term_positive (n : ℕ) : 0 < f n := by\n  exact pow_pos base_positive _\n\ntheorem problem_recurrence (n : ℕ) : {lhs_f} = {rhs_f} := by\n  have h := (eq_div_iff (ne_of_gt (term_positive n))).mp (problem_valid.recurrence n)\n  simpa only [step, F] using h\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial}) (hj : a 1 = {second_initial})\n    (hs : ∀ n, {lhs_a} = {rhs_a}) : ∀ n, a n = f n := by\n  apply Recurrence.general_second_unique problem_valid\n  apply Recurrence.multiplicative_from_relation a {initial} {second_initial} (fun n => {base} ^ F n) hi hj\n  · norm_num\n  · norm_num\n  · intro n; exact pow_pos base_positive _\n  · simpa only [F] using hs\n", exponent_proof(profile, true)));
    }
    let conditions = p
        .conditions
        .iter()
        .map(|condition| {
            let expr = render(&condition.expression, "f", None, false)?;
            match condition.kind.as_str() {
                "positive" => Ok(format!("0 < {expr}")),
                "nonzero" => Ok(format!("{expr} ≠ 0")),
                _ => Err("未登録の指数型の定義域条件です。".to_string()),
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    if conditions.is_empty() {
        s.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        s.push_str(&format!("\ntheorem problem_conditions (n : ℕ) : {} := by\n  {}\n  all_goals\n    dsimp [f]\n    positivity\n", conditions.join(" ∧ "), if conditions.len() == 1 { "skip" } else { "repeat' constructor" }));
    }
    for theorem in [
        "problem_valid",
        "problem_recurrence",
        "problem_unique",
        "problem_conditions",
    ] {
        s.push_str(&format!("\n#print axioms {theorem}\n"));
    }
    s.push_str("\nend GeneratedProblem\n");
    Ok(Some(s))
}
