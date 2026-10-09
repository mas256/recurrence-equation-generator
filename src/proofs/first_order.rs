//! Proofs instantiate the actual typed expression and recurrence trees.
use crate::extensions::first_order::{self, Route};
use crate::generator::GeneratedProblem;
use crate::math::Expr;
use crate::verifier::render;

fn rn(e: &Expr, sequence: &str, step: bool) -> Result<String, String> {
    render(e, sequence, None, step)
}
fn header(problem: &GeneratedProblem) -> Result<String, String> {
    Ok(format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- public a₁ is f 0; public n is Lean n+1\ndef f (n : ℕ) : ℚ := {}\ndef step (n : ℕ) (x : ℚ) : ℚ := {}\n\n",problem.id,rn(&problem.problem.general_term.rhs,"f",false)?,rn(&problem.problem.recurrence.rhs,"a",true)?))
}
fn polynomial_tactics(defs: &str, indent: &str) -> String {
    format!("{indent}simp only [{defs}, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n{indent}have hn1 : (n : ℚ) + 1 ≠ 0 := by positivity\n{indent}have hn2 : (n : ℚ) + 1 + 1 ≠ 0 := by positivity\n{indent}have hn3 : (n : ℚ) + 1 + 2 ≠ 0 := by positivity\n{indent}have hn4 : (n : ℚ) + 1 + 1 + 1 ≠ 0 := by positivity\n{indent}have hn5 : (n : ℚ) + 1 + 1 + 2 ≠ 0 := by positivity\n{indent}have ho1 : 2 * ((n : ℚ) + 1) + 1 ≠ 0 := by positivity\n{indent}have ho2 : 2 * ((n : ℚ) + 1 + 1) + 1 ≠ 0 := by positivity\n{indent}field_simp [hn1, hn2, hn3, hn4, hn5, ho1, ho2] <;> ring\n")
}
fn base_expression(formula: &Expr) -> Option<Expr> {
    match formula {
        Expr::Div {
            numerator,
            denominator,
        } if **numerator == Expr::integer(1) => Some(*denominator.clone()),
        Expr::Add { terms } => terms.iter().find_map(base_expression),
        _ => None,
    }
}

pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !first_order::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    let route =
        first_order::detect(p).ok_or("問題の式から1次漸化式の証明経路を検出できません。")?;
    let mut source = header(problem)?;
    let init = rn(&p.initials[0].rhs, "f", false)?;
    let mut denom_witness = false;
    match &route {
        Route::Reciprocal { .. } | Route::Mobius { .. } => {
            let base =
                base_expression(&p.general_term.rhs).ok_or("逆数の基底一般項がありません。")?;
            let (coefficient, forcing, mobius) = match &route {
                Route::Reciprocal {
                    coefficient,
                    forcing,
                    ..
                } => (coefficient.clone(), forcing.clone(), None),
                Route::Mobius {
                    shift,
                    ratio,
                    forcing,
                } => (
                    Expr::rational(ratio.clone()),
                    Expr::rational(forcing.clone()),
                    Some(shift.clone()),
                ),
                _ => unreachable!(),
            };
            let first = base
                .evaluate(1, &|_, _| Err("基底の一般項に数列の項があります。".into()))?
                .lean();
            source.push_str(&format!("def v (n : ℕ) : ℚ := {}\ndef vStep (n : ℕ) (x : ℚ) : ℚ := ({} * x + {}) / 1\n\ntheorem v_valid : Recurrence.FirstCertificate v {first} vStep := by\n  constructor\n  · norm_num [v]\n  · intro n\n",rn(&base,"f",false)?,rn(&coefficient,"f",false)?,rn(&forcing,"f",false)?));
            source.push_str(&polynomial_tactics("v, vStep", "    "));
            let particular = matches!(&route,Route::Reciprocal{inner,..}if matches!(**inner,Route::Particular{..}));
            if particular {
                source.push_str("\ntheorem v_positive (n : ℕ) : 0 < v n := by\n  induction n with\n  | zero => norm_num [v]\n  | succ n ih =>\n    rw [v_valid.recurrence]\n    simp only [vStep, div_one, Nat.cast_add, Nat.cast_one]\n    exact add_pos (mul_pos (by norm_num) ih) (by positivity)\n");
            } else {
                source.push_str("\ntheorem v_positive (n : ℕ) : 0 < v n := by\n  simp only [v, Nat.cast_add, Nat.cast_one, div_one]\n");
                let inverse_scale = matches!(&route,Route::Reciprocal{inner,..} if matches!(**inner,Route::Scaled{profile,..}if profile>=6));
                if inverse_scale {
                    source.push_str(
                        "  exact mul_pos (div_pos (by norm_num) (by positivity)) (by positivity)\n",
                    );
                } else {
                    source.push_str("  positivity\n");
                }
            }
            let co = rn(&coefficient, "f", false)?;
            let fo = rn(&forcing, "f", false)?;
            source.push_str(&format!("\ndef Q (n : ℕ) : ℚ := {co}\ndef R (n : ℕ) : ℚ := {fo}\ntheorem reciprocal_valid : Recurrence.FirstCertificate (fun n => 1 / v n) (1 / {first})\n    (fun n x => 1 * x / (Q n + R n * x)) := by\n  exact (Recurrence.reciprocal_certificate v_valid v_positive\n    (fun _ => (by norm_num : 0 < (1 : ℚ)))).1\n\ntheorem reciprocal_den (n : ℕ) : Q n + R n * (1 / v n) ≠ 0 := by\n  exact (Recurrence.reciprocal_certificate v_valid v_positive\n    (fun _ => (by norm_num : 0 < (1 : ℚ)))).2.1 n\n"));
            if let Some(h) = mobius {
                source.push_str(&format!("\ntheorem shifted_valid : Recurrence.FirstCertificate (fun n => 1 / v n + {}) (1 / {first} + {})\n    (fun n x => ((1 + {} * R n) * x + {} * Q n - {} * 1 - {} ^ 2 * R n) /\n      (R n * x + Q n - {} * R n)) :=\n  Recurrence.shifted_fraction_certificate reciprocal_valid reciprocal_den {}\n\ntheorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n    have hs := shifted_valid.recurrence n\n    norm_num [f, v, step, Q, R] at hs ⊢\n    convert hs using 1 <;> congr 1 <;> ring\n",h.lean(),h.lean(),h.lean(),h.lean(),h.lean(),h.lean(),h.lean(),h.lean()));
            } else {
                source.push_str(&format!("\ntheorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n    have hs := reciprocal_valid.recurrence n\n    simpa only [f, v, step, Q, R, one_mul, div_one, add_comm] using hs\n"));
            }
            denom_witness = true;
            // The actual source denominator (including a shift) is certified below.
            let Expr::Div { denominator, .. } = &p.recurrence.rhs else {
                return Err("元の分母がありません。".into());
            };
            let den = rn(denominator, "f", false)?;
            source.push_str(&format!(
                "\ntheorem original_den (n : ℕ) : {den} ≠ 0 := by\n  have hd := reciprocal_den n\n"
            ));
            if let Route::Mobius { .. } = route {
                source.push_str("  norm_num [Q, R] at hd\n  have he : ");
                source.push_str(&format!("{den} = Q n + R n * (1 / v n) := by\n    dsimp [f, v, Q, R]\n    ring\n  rw [he]\n  exact reciprocal_den n\n"));
            } else {
                source.push_str("  simpa only [f, v, Q, R, div_one, add_comm] using hd\n");
            }
        }
        _ => {
            source.push_str(&format!("theorem problem_valid : Recurrence.FirstCertificate f {init} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n"));
            source.push_str(&polynomial_tactics("f, step", "    "));
        }
    }
    let rhsf = rn(&p.recurrence.rhs, "f", false)?;
    let rhsa = rn(&p.recurrence.rhs, "a", false)?;
    source.push_str(&format!("\ntheorem problem_recurrence (n : ℕ) : f (n + 1) = {rhsf} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {init})\n    (hs : ∀ n, a (n + 1) = {rhsa}) : ∀ n, a n = f n :=\n  Recurrence.first_unique problem_valid ⟨hi, hs⟩\n"));
    let mut conditions = Vec::new();
    for cond in &p.conditions {
        let e = rn(&cond.expression, "f", false)?;
        conditions.push(match cond.kind.as_str() {
            "nonzero" => format!("{e} ≠ 0"),
            "positive" => format!("0 < {e}"),
            _ => return Err("未登録の条件です。".into()),
        });
    }
    if conditions.is_empty() {
        source.push_str("\ntheorem problem_conditions : True := by trivial\n");
    } else {
        source.push_str(&format!(
            "\ntheorem problem_conditions (n : ℕ) : {} := by\n",
            conditions.join(" ∧ ")
        ));
        if denom_witness {
            source.push_str("  have hv := v_positive n\n  have hd := original_den n\n  have hnv : v n ≠ 0 := ne_of_gt hv\n");
        }
        // The conjunction is constructed in the same order as the IR conditions.
        if p.conditions.len() > 1 {
            source.push_str(&format!(
                "  refine ⟨{}⟩\n",
                vec!["?_"; p.conditions.len()].join(", ")
            ));
        } else {
            source.push_str("  refine ?_\n");
        }
        for cond in &p.conditions {
            source.push_str("  · ");
            let is_den = matches!(&p.recurrence.rhs,Expr::Div{denominator,..}if **denominator==cond.expression);
            if denom_witness && is_den {
                source.push_str("exact hd\n");
            } else if denom_witness && matches!(&cond.expression, Expr::Term { .. }) {
                source.push_str("simpa only [f, v, div_one] using one_div_ne_zero hnv\n");
            } else if denom_witness && matches!(&route, Route::Mobius { .. }) {
                source.push_str("have he : ");
                source.push_str(&format!("{} = 1 / v n := by\n      dsimp [f, v]\n      ring\n    rw [he]\n    exact one_div_ne_zero hnv\n",rn(&cond.expression,"f",false)?));
            } else {
                source.push_str("try simp only [Nat.cast_add, Nat.cast_one]\n    positivity\n");
            }
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
    Ok(Some(source))
}
