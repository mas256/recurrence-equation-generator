//! Universal certificates for exponential forcing and positive power relations.
use crate::extensions::exponential::{self, Route};
use crate::generator::GeneratedProblem;
use crate::verifier::render;

pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !exponential::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    let route = exponential::detect(p).ok_or("原式から指数・対数型の証明経路を復元できません。")?;
    if p.general_term.rhs.canonical() != exponential::formula(&route).canonical() {
        return Err("原式と一般項が一致しません。".into());
    }
    let formula = render(&p.general_term.rhs, "f", None, false)?;
    let step = render(&p.recurrence.rhs, "a", None, true)?;
    let init = render(&p.initials[0].rhs, "f", None, false)?;
    let rhsf = render(&p.recurrence.rhs, "f", None, false)?;
    let rhsa = render(&p.recurrence.rhs, "a", None, false)?;
    let mut s=format!("import Recurrence.Theory\n\n#eval Lean.versionString\nnamespace GeneratedProblem\n-- content SHA-256: {}\ndef f (n : ℕ) : ℚ := {formula}\ndef step (n : ℕ) (x : ℚ) : ℚ := {step}\n\n",problem.id);
    match &route {
        Route::Forcing { .. } => {
            s.push_str(&format!("theorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n    cases n with\n    | zero => norm_num [f, step]\n    | succ k =>\n      dsimp [f, step]\n      have h1 : k + 1 + 1 - 2 = k := by omega\n      have h2 : k + 1 + 1 + 1 - 2 = k + 1 := by omega\n      simp only [h1, h2, Nat.cast_add, Nat.cast_one, pow_succ]\n      norm_num <;> ring\n"));
        }
        Route::Logarithmic {
            base,
            q,
            d,
            e,
            shift,
        } => {
            let b = base.lean();
            let h = shift.lean();
            if *q == 0 || (*d == 0 && *e == 0) {
                s.push_str(&format!("theorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n    cases n with\n    | zero => norm_num [f, step]\n    | succ k => simp [f, step, Nat.succ_eq_add_one, pow_succ] <;> norm_num\n"));
            } else {
                let exponent = exponential::logarithmic_exponent(*q, *d, *e).lean();
                s.push_str(&format!("def E (n : ℕ) : ℕ := {exponent}\n\ntheorem formula_eq (n : ℕ) : f n = {h} + {b} ^ E n := by\n  dsimp [f, E]\n  ring\n\ntheorem exponent_step (n : ℕ) : E (n + 1) = {d} + E n * {q} := by\n"));
                if *q == 1 {
                    s.push_str("  simp only [E, Nat.add_assoc]\n  ring\n");
                } else {
                    s.push_str(&format!(
                        "  have hp : 1 ≤ ({q} : ℕ) ^ n := Nat.one_le_pow n {q} (by norm_num)\n"
                    ));
                    if *q == 3 {
                        s.push_str("  have hm : (3 : ℕ) ^ n % 2 = 1 := by\n    clear hp\n    induction n with\n    | zero => norm_num\n    | succ k ih => simp [pow_succ, Nat.mul_mod, ih]\n");
                    }
                    s.push_str(
                        "  dsimp [E]\n  simp only [Nat.add_sub_cancel, pow_succ]\n  omega\n",
                    );
                }
                s.push_str(&format!("\ntheorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f, E]\n  · intro n\n    rw [formula_eq (n + 1)]\n    simp only [step, formula_eq]\n    rw [exponent_step]\n    simp only [pow_add, pow_mul]\n    ring\n"));
            }
        }
    }
    s.push_str(&format!("\ntheorem problem_recurrence (n : ℕ) : f (n + 1) = {rhsf} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {init})\n    (hs : ∀ n, a (n + 1) = {rhsa}) : ∀ n, a n = f n :=\n  Recurrence.first_unique problem_valid ⟨hi, hs⟩\n"));
    if p.conditions.is_empty() {
        s.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        if p.conditions.len() != 1 || p.conditions[0].kind != "positive" {
            return Err("未対応の対数型の定義域条件です。".into());
        }
        let condition = render(&p.conditions[0].expression, "f", None, false)?;
        let Route::Logarithmic { base, q, d, e, .. } = route else {
            return Err("未対応の正値条件です。".into());
        };
        s.push_str(&format!(
            "\ntheorem problem_conditions (n : ℕ) : 0 < {condition} := by\n"
        ));
        if q == 0 || (d == 0 && e == 0) {
            s.push_str("  cases n with\n  | zero => norm_num [f]\n  | succ k => simp [f, Nat.succ_eq_add_one, pow_succ] <;> norm_num\n");
        } else {
            s.push_str(&format!("  rw [formula_eq]\n  have hb : (0 : ℚ) < {} := by norm_num\n  convert pow_pos hb (E n) using 1 <;> ring\n",base.lean()));
        }
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
