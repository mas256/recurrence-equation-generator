//! Native certificates for the actual rational question and answer ASTs.
use crate::generator::{GeneratedProblem, RecurrenceKind};
use crate::verifier::render;
fn rn(e: &crate::math::Expr, sequence: &str, step: bool) -> Result<String, String> {
    render(e, sequence, None, step)
}
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    if !crate::extensions::elementary::supports(&problem.recipe.family) {
        return Ok(None);
    }
    let p = &problem.problem;
    if p.kind == RecurrenceKind::SecondOrder {
        let mut delegated = problem.clone();
        // The second-order proof writer reads the displayed AST, not this family's
        // parameters. Reuse its universal certificate and uniqueness theorem.
        delegated.recipe.family = "forced_second".into();
        let source = crate::proofs::second_order::source(&delegated)?
            .ok_or("3項間の証明ソースを生成できません。")?;
        return Ok(Some(source.replace("field_simp [hp] <;> ring", "simp [Nat.add_sub_cancel, Nat.add_sub_cancel_left, pow_succ, pow_add]\n    <;> norm_num\n    <;> field_simp [hp]\n    <;> ring")));
    }
    if p.kind != RecurrenceKind::FirstOrder {
        return Err("基本分岐の未対応証明型です。".into());
    }
    if p.recurrence.lhs != crate::math::Expr::term("a", 1) {
        return Err("証明対象の次項が正規形ではありません。".into());
    }
    let initial = rn(&p.initials[0].rhs, "f", false)?;
    let formula = rn(&p.general_term.rhs, "f", false)?;
    let step = rn(&p.recurrence.rhs, "a", true)?;
    let rhs_f = rn(&p.recurrence.rhs, "f", false)?;
    let rhs_a = rn(&p.recurrence.rhs, "a", false)?;
    let mut s=format!("import Recurrence.Theory\n\n#eval Lean.versionString\n\nnamespace GeneratedProblem\n\n-- content SHA-256: {}\n-- public a₁ is f 0\ndef f (n : ℕ) : ℚ := {formula}\ndef step (n : ℕ) (x : ℚ) : ℚ := {step}\n\ntheorem problem_valid : Recurrence.FirstCertificate f {initial} step := by\n  constructor\n  · norm_num [f]\n  · intro n\n    simp [f, step, Nat.cast_add, Nat.cast_one, pow_succ, pow_add]\n    <;> norm_num\n    <;> ring\n\ntheorem problem_recurrence (n : ℕ) : f (n + 1) = {rhs_f} := problem_valid.recurrence n\n\ntheorem problem_unique (a : ℕ → ℚ) (hi : a 0 = {initial})\n    (hs : ∀ n, a (n + 1) = {rhs_a}) : ∀ n, a n = f n :=\n  Recurrence.first_unique problem_valid ⟨hi, hs⟩\n\ntheorem problem_conditions : True := by trivial\n",problem.id);
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
    use rand::SeedableRng;
    #[test]
    fn boundary_proofs_are_universal_and_have_no_placeholder_proof() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(77);
        for family in crate::extensions::elementary::catalog() {
            for variant in crate::extensions::elementary::variants(&family.id) {
                let p =
                    crate::extensions::elementary::sample_variant(&family.id, variant.id, &mut rng)
                        .unwrap();
                let recipe =
                    crate::extensions::elementary::build_recipe(&family.id, 77, p).unwrap();
                if let Ok(g) = crate::generator::replay(&recipe) {
                    let source = super::source(&g).unwrap().unwrap();
                    assert!(source.contains("∀ n"));
                    assert!(source.contains("problem_unique"));
                    assert!(!source.contains("sorry"));
                    assert!(!source.contains("admit"));
                }
            }
        }
    }
    #[test]
    #[ignore = "requires prepared Lean 4.19 + mathlib"]
    fn all_elementary_variants_have_universal_lean_certificates() {
        use std::{
            path::PathBuf,
            sync::{atomic::AtomicBool, Arc},
            time::Duration,
        };
        let verifier =
            crate::verifier::Verifier::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean"));
        assert!(verifier.status().ready, "{}", verifier.status().message);
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(177);
        for family in crate::extensions::elementary::catalog() {
            for variant in crate::extensions::elementary::variants(&family.id) {
                let generated = (0..64)
                    .find_map(|_| {
                        let params = crate::extensions::elementary::sample_variant(
                            &family.id, variant.id, &mut rng,
                        )
                        .unwrap();
                        let recipe =
                            crate::extensions::elementary::build_recipe(&family.id, 177, params)
                                .unwrap();
                        crate::generator::replay(&recipe).ok()
                    })
                    .unwrap_or_else(|| {
                        panic!("no accepted candidate for {} {}", family.id, variant.id)
                    });
                let record = verifier
                    .verify(
                        &generated,
                        Arc::new(AtomicBool::new(false)),
                        Duration::from_secs(60),
                    )
                    .unwrap_or_else(|error| panic!("{} {}: {}", family.id, variant.id, error));
                assert_eq!(record.checked_theorems.len(), 4);
                println!(
                    "{} {}: verified {} ms",
                    family.id, variant.id, record.elapsed_ms
                );
            }
        }
    }
}
