//! Proofs of actual finite sums, including the n=1 boundary and uniqueness.
use crate::extensions::sums;
use crate::generator::{GeneratedProblem, RecurrenceKind};
use crate::math::Expr;
use crate::verifier::render;

pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !sums::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    let formula = render(&p.general_term.rhs, "f", None, false)?;
    let partial_formula = render(&sums::sum_formula(&problem.recipe)?, "f", None, false)?;
    let initial = render(&p.initials[0].rhs, "f", None, false)?;
    let lhsf = render(&p.recurrence.lhs, "f", None, false)?;
    let rhsf = render(&p.recurrence.rhs, "f", None, false)?;
    let lhsa = render(&p.recurrence.lhs, "a", None, false)?;
    let rhsa = render(&p.recurrence.rhs, "a", None, false)?;
    let mut source = format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- Public sum k=1..n is the Lean sum over range(n+1).\ndef f (n : ℕ) : ℚ := {formula}\ndef S (n : ℕ) : ℚ := {partial_formula}\n\ntheorem problem_prefix : ∀ n, Recurrence.partialSum f n = S n := by\n  intro n\n  induction n with\n  | zero => norm_num [Recurrence.partialSum, f, S]\n  | succ n ih =>\n    rw [Recurrence.prefix_succ, ih]\n    simp only [f, S, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    norm_num <;> ring\n\n", problem.id);
    if p.kind == RecurrenceKind::SumRelation {
        let (alpha, forcing) = sums::split(&p.recurrence.rhs, &Expr::term("a", 0))?;
        let alpha = render(&alpha, "f", None, false)?;
        let forcing = render(&forcing, "f", None, false)?;
        source.push_str(&format!("def alpha : ℚ := {alpha}\ndef forcing (n : ℕ) : ℚ := {forcing}\n\ntheorem problem_valid : Recurrence.SumRelationCertificate f {initial} alpha forcing := by\n  constructor\n  · norm_num [f]\n  · intro n\n    rw [problem_prefix]\n    simp only [f, S, alpha, forcing, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    norm_num <;> ring\n\ntheorem problem_recurrence (n : ℕ) : {lhsf} = {rhsf} := by\n  simpa only [Recurrence.partialSum, alpha, forcing, add_assoc] using problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial})\n    (hs : ∀ (n : ℕ), {lhsa} = {rhsa}) : ∀ n, a n = f n := by\n  apply Recurrence.sum_relation_unique problem_valid\n  · refine ⟨hi, ?_⟩\n    intro n\n    simpa only [Recurrence.partialSum, alpha, forcing, add_assoc] using hs n\n  · norm_num [alpha]\n"));
    } else if p.initials.len() == 1 {
        let current_sum = render(&sums::sum(0), "a", None, false)?;
        let step = rhsa.replace(&current_sum, "x");
        source.push_str(&format!("def step (n : ℕ) (x : ℚ) : ℚ := {step}\n\ntheorem S_valid : Recurrence.FirstCertificate S {initial} step := by\n  constructor\n  · norm_num [S]\n  · intro n\n    simp only [S, step, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    norm_num <;> ring\n\ntheorem problem_valid : Recurrence.PureSumCertificate f {initial} step := by\n  have heq : Recurrence.partialSum f = S := funext problem_prefix\n  change Recurrence.FirstCertificate (Recurrence.partialSum f) {initial} step\n  rw [heq]\n  exact S_valid\n\ntheorem problem_recurrence (n : ℕ) : {lhsf} = {rhsf} := by\n  have h := problem_valid.recurrence n\n  simpa only [Recurrence.partialSum, step] using h\n\ntheorem problem_unique (a : ℕ → ℚ)\n    (hi : {})\n    (hs : ∀ (n : ℕ), {lhsa} = {rhsa}) : ∀ n, a n = f n := by\n  apply Recurrence.pure_sum_unique problem_valid\n  refine ⟨?_, ?_⟩\n  · simpa only [Recurrence.partialSum] using hi\n  · intro n\n    simpa only [Recurrence.partialSum, step] using hs n\n", render(&p.initials[0].lhs, "a", None, false)?.to_string() + " = " + &initial));
    } else {
        let second = render(&p.initials[1].rhs, "f", None, false)?;
        let (lead, _) = sums::split(&p.recurrence.lhs, &sums::sum(2))?;
        let lead = render(&lead, "f", None, false)?;
        let current_sum = render(&sums::sum(0), "a", None, false)?;
        let next_sum = render(&sums::sum(1), "a", None, false)?;
        let step = format!(
            "({} / lead n)",
            rhsa.replace(&next_sum, "y").replace(&current_sum, "x")
        );
        source.push_str(&format!("def lead (n : ℕ) : ℚ := {lead}\ndef step (n : ℕ) (x y : ℚ) : ℚ := {step}\n\ntheorem lead_nonzero (n : ℕ) : lead n ≠ 0 := by\n  dsimp [lead]\n  positivity\n\ntheorem S_valid : Recurrence.GeneralSecondCertificate S {initial} {second} step := by\n  constructor\n  · norm_num [S]\n  · norm_num [S]\n  · intro n\n    apply (eq_div_iff (lead_nonzero n)).2\n    simp only [S, step, lead, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    norm_num <;> ring\n\ntheorem problem_valid : Recurrence.PureSecondSumCertificate f {initial} {second} step := by\n  have heq : Recurrence.partialSum f = S := funext problem_prefix\n  change Recurrence.GeneralSecondCertificate (Recurrence.partialSum f) {initial} {second} step\n  rw [heq]\n  exact S_valid\n\ntheorem problem_recurrence (n : ℕ) : {lhsf} = {rhsf} := by\n  have h := (eq_div_iff (lead_nonzero n)).1 (problem_valid.recurrence n)\n  simpa only [Recurrence.partialSum, step, lead, mul_comm] using h\n\ntheorem problem_unique (a : ℕ → ℚ)\n    (hi : {} = {initial})\n    (hj : {} = {second})\n    (hs : ∀ (n : ℕ), {lhsa} = {rhsa}) : ∀ n, a n = f n := by\n  apply Recurrence.pure_second_sum_unique problem_valid\n  refine ⟨?_, ?_, ?_⟩\n  · simpa only [Recurrence.partialSum] using hi\n  · simpa only [Recurrence.partialSum] using hj\n  · intro n\n    apply (eq_div_iff (lead_nonzero n)).2\n    simpa only [Recurrence.partialSum, step, lead, mul_comm] using hs n\n", render(&p.initials[0].lhs, "a", None, false)?, render(&p.initials[1].lhs, "a", None, false)?));
    }
    let conditions = p
        .conditions
        .iter()
        .map(|condition| {
            render(&condition.expression, "f", None, false).map(|e| match condition.kind.as_str() {
                "positive" => format!("0 < {e}"),
                _ => format!("{e} ≠ 0"),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if conditions.is_empty() {
        source.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        source.push_str(&format!(
            "\ntheorem problem_conditions (n : ℕ) : {} := by\n  {}positivity\n",
            conditions.join(" ∧ "),
            if conditions.len() > 1 {
                "constructor\n  all_goals "
            } else {
                ""
            }
        ));
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
    Ok(Some(source))
}
