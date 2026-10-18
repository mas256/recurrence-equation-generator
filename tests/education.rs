use recurrence_lab::{
    generator, learning,
    math::{Equation, Expr},
    route_check, selection,
};
use serde_json::Value;

#[test]
fn legacy_saved_recipes_keep_identity_and_assessment() {
    let fixtures: Vec<Value> = serde_json::from_str(include_str!("browser-fixtures.json")).unwrap();
    for f in fixtures {
        let recipe = serde_json::from_value(f["recipe"].clone()).unwrap();
        let p =
            generator::replay(&recipe).unwrap_or_else(|e| panic!("{}: {e}", f["recipe"]["family"]));
        let actual = serde_json::to_value(p).unwrap();
        for key in ["id", "statement_latex", "answer_latex", "classification"] {
            assert_eq!(actual[key], f[key], "{} / {key}", f["recipe"]["family"]);
        }
    }
}

#[test]
fn easier_complete_methods_determine_new_levels() {
    for (family, level) in [
        ("arithmetic_difference", 2),
        ("sum_relation", 3),
        ("reciprocal_affine", 3),
    ] {
        let p = generator::generate(level, Some(family), 42).unwrap();
        assert_eq!(p.classification.level, level);
        assert_eq!(
            p.classification.score,
            p.routes
                .iter()
                .filter(|r| r.complete)
                .map(|r| r.score)
                .min()
                .unwrap()
        );
        assert_eq!(p.classification.rules_version, learning::ASSESSMENT_VERSION);
    }
    let affine = generator::generate(2, Some("affine"), 42).unwrap();
    let methods = affine
        .routes
        .iter()
        .map(|r| r.method_id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    for method in ["fixed_point", "normalize", "difference"] {
        assert!(methods.contains(method), "{methods:?}");
    }
}

#[test]
fn inconsistent_alternative_is_rejected() {
    let p = generator::generate(2, Some("affine"), 42).unwrap();
    let mut d = p
        .routes
        .iter()
        .find(|r| r.method_id == "difference")
        .unwrap()
        .derivation
        .clone();
    d.steps.last_mut().unwrap().equation =
        Some(Equation::new(Expr::term("a", 0), Expr::integer(999)));
    assert!(route_check::validate(&p.problem, &d).is_err());
}

#[test]
fn alternatives_exercises_always_have_multiple_complete_routes() {
    let known = learning::methods()
        .into_iter()
        .map(|m| m.id)
        .collect::<std::collections::BTreeSet<_>>();
    for level in 1..=5 {
        match selection::generate(level, None, 42, &[], &[], "alternatives", 200) {
            Ok(p) => {
                assert!(p.routes.len() >= 2);
                for r in p.routes {
                    assert!(known.contains(&r.method_id));
                    assert!(!r.solution.is_empty());
                    assert!(!r.validation.formal_verification);
                }
            }
            Err(e) => assert!(e.contains("別解"), "{e}"),
        }
    }
}
