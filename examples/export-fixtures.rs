use recurrence_lab::generator;
use std::collections::{BTreeMap, BTreeSet};

// These are the currently registered variants, not the complete legacy matrix.
fn profiles(family: &str) -> Vec<Option<u32>> {
    let codes: Vec<u32> = match family {
        "shifted_scaled" => (0..12).collect(),
        "scaled_affine" | "reciprocal_scaled" => vec![0, 1, 5, 6, 7, 8, 9, 10, 11],
        "polynomial_difference" => (0..6).collect(),
        "scaled_second_order"
        | "difference_scaled"
        | "coupled_scaled"
        | "ratio_power"
        | "multiplicative_second" => vec![0, 1, 2],
        "pure_sum_scaled" => vec![0, 1, 5],
        "pure_sum" | "sum_relation" | "coupled_forced" => vec![0, 1],
        "coupled_symmetric" | "coupled_weighted" => vec![0],
        _ => return vec![None],
    };
    codes.into_iter().map(Some).collect()
}

fn main() {
    let samples = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>().expect("positive samples per profile"))
        .unwrap_or(3);
    assert!(samples > 0 && samples <= 100);
    let mut fixtures = Vec::new();
    for family in generator::family_catalog() {
        let expected: BTreeSet<_> = profiles(&family.id).into_iter().collect();
        let mut covered = BTreeMap::new();
        let mut levels = BTreeMap::new();
        'search: for seed in 0..400 {
            for &level in &family.levels {
                let problem = generator::generate(level, Some(&family.id), seed).unwrap();
                let profile = problem.recipe.parameters.get("profile").map(|p| {
                    p.integer_u32()
                        .expect("registered profile must be a natural integer")
                });
                assert!(
                    expected.contains(&profile),
                    "{}: unexpected profile {profile:?}",
                    family.id
                );
                let count = *covered.get(&profile).unwrap_or(&0);
                let level_count = *levels.get(&level).unwrap_or(&0);
                if count < samples || level_count < samples {
                    *covered.entry(profile).or_insert(0) += 1;
                    *levels.entry(level).or_insert(0) += 1;
                    fixtures.push(problem);
                }
                if expected
                    .iter()
                    .all(|p| covered.get(p).is_some_and(|n| *n >= samples))
                    && family
                        .levels
                        .iter()
                        .all(|lv| levels.get(lv).is_some_and(|n| *n >= samples))
                {
                    break 'search;
                }
            }
        }
        assert!(
            expected
                .iter()
                .all(|p| covered.get(p).is_some_and(|n| *n >= samples)),
            "{}: missing profile coverage {covered:?}",
            family.id
        );
        assert!(
            family
                .levels
                .iter()
                .all(|lv| levels.get(lv).is_some_and(|n| *n >= samples)),
            "{}: missing level coverage {levels:?}",
            family.id
        );
    }
    println!("{}", serde_json::to_string_pretty(&fixtures).unwrap());
}
