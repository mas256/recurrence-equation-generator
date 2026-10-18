//! Balance accepted textbook patterns, then their explicit branches.
//! Rejection sampling stays within a branch; rejected coefficients do not vote.
use crate::{
    generator::{self, GeneratedProblem, Recipe},
    math::Rational,
};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const VERSION: &str = "curriculum-balance-1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariantDescriptor {
    pub id: String,
    pub label: String,
    pub pattern: String,
    pub family: String,
    pub level: u8,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectionRecord {
    pub id: String,
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub variant: String,
}

fn pattern(family: &str) -> &'static str {
    match family {
        "arithmetic" | "geometric" | "affine" | "cases_constant" | "cases_reset"
        | "cases_arithmetic" | "cases_geometric" | "cases_affine_fixed" | "cases_affine" => {
            "constant_coefficient_first"
        }
        "scaled_constant" | "factorial_ratio" | "shifted_scaled" | "scaled_affine"
        | "difference_scaled" => "variable_coefficient",
        "polynomial_forcing" | "polynomial_difference" => "polynomial_forcing",
        "cases_exponential_forcing" => "exponential_forcing",
        "cases_logarithmic" | "cases_shifted_logarithmic" => "logarithmic",
        "reciprocal_affine"
        | "reciprocal_scaled"
        | "reciprocal_forcing"
        | "mobius"
        | "cases_mobius_no_real_fixed" => "fractional",
        "weighted_sum" | "pure_sum" | "pure_sum_scaled" | "sum_relation" => "partial_sums",
        "coupled_symmetric" | "coupled_weighted" | "coupled_scaled" | "coupled_forced" => "coupled",
        "ratio_power" | "multiplicative_second" => "multiplicative",
        _ => "second_order",
    }
}
fn descriptor(family: &str, id: &str, label: &str, level: u8) -> VariantDescriptor {
    VariantDescriptor {
        id: format!("{family}/{id}"),
        label: label.into(),
        pattern: pattern(family).into(),
        family: family.into(),
        level,
    }
}
fn profiles(family: &str) -> Vec<(u32, u8)> {
    match family {
        "shifted_scaled" => (0..12)
            .map(|p| (p, if [0, 2, 8].contains(&p) { 3 } else { 4 }))
            .collect(),
        "scaled_affine" => [0, 1, 5, 6, 7, 8, 9, 10, 11].map(|p| (p, 4)).to_vec(),
        "reciprocal_scaled" => [0, 1, 5, 6, 7, 8, 9, 10, 11].map(|p| (p, 5)).to_vec(),
        "polynomial_difference" => (0..6)
            .map(|p| (p, if [0, 1, 5].contains(&p) { 1 } else { 2 }))
            .collect(),
        "scaled_second_order" | "coupled_scaled" => (0..3).map(|p| (p, 4)).collect(),
        "difference_scaled" | "multiplicative_second" => (0..3).map(|p| (p, 5)).collect(),
        "pure_sum_scaled" => [0, 1, 5].map(|p| (p, 5)).to_vec(),
        "pure_sum" => vec![(0, 2), (1, 2)],
        "sum_relation" => vec![(0, 3), (1, 3)],
        "ratio_power" => vec![(0, 3), (1, 3), (2, 4)],
        "coupled_forced" => vec![(0, 3), (1, 5)],
        "coupled_symmetric" | "coupled_weighted" => vec![(0, 3)],
        _ => vec![],
    }
}
pub fn catalog() -> Vec<VariantDescriptor> {
    let mut result = vec![];
    for f in generator::family_catalog() {
        if crate::extensions::elementary::supports(&f.id) {
            for v in crate::extensions::elementary::variants(&f.id) {
                result.push(descriptor(&f.id, v.id, v.label, v.level));
            }
        } else if crate::extensions::exponential::supports(&f.id) {
            for v in crate::extensions::exponential::variants(&f.id) {
                result.push(descriptor(&f.id, v.id, v.label, v.level));
            }
        } else if f.id == "second_order" {
            for r in 2..=5 {
                for s in r + 1..=5 {
                    result.push(descriptor(
                        &f.id,
                        &format!("roots_{r}_{s}"),
                        &format!("異なる正整数根 {r}, {s}"),
                        3,
                    ));
                }
            }
        } else {
            let ps = profiles(&f.id);
            if ps.is_empty() {
                result.push(descriptor(&f.id, "base", &f.label, f.levels[0]));
            } else {
                for (p, l) in ps {
                    result.push(descriptor(
                        &f.id,
                        &format!("profile_{p}"),
                        &format!("{}・分岐{}", f.label, p),
                        l,
                    ));
                }
            }
        }
    }
    result
}
pub fn describe(recipe: &Recipe, level: u8) -> Option<VariantDescriptor> {
    // New modules encode their branch in parameters; do not infer from a formula's appearance.
    if crate::extensions::elementary::supports(&recipe.family) {
        return crate::extensions::elementary::variant_id(recipe).and_then(|id| {
            catalog()
                .into_iter()
                .find(|v| v.family == recipe.family && v.id == format!("{}/{id}", recipe.family))
        });
    }
    if crate::extensions::exponential::supports(&recipe.family) {
        return crate::extensions::exponential::variant_id(recipe).and_then(|id| {
            catalog()
                .into_iter()
                .find(|v| v.family == recipe.family && v.id == format!("{}/{id}", recipe.family))
        });
    }
    if recipe.family == "second_order" {
        let r = recipe.parameters.get("r")?.integer_u32()?;
        let s = recipe.parameters.get("s")?.integer_u32()?;
        let id = format!("second_order/roots_{}_{}", r.min(s), r.max(s));
        return catalog().into_iter().find(|v| v.id == id);
    }
    let suffix = recipe
        .parameters
        .get("profile")
        .and_then(Rational::integer_u32)
        .map(|p| format!("profile_{p}"))
        .unwrap_or_else(|| "base".into());
    catalog().into_iter().find(|v| {
        v.family == recipe.family
            && v.id == format!("{}/{suffix}", recipe.family)
            && v.level == level
    })
}

pub fn generate(
    level: u8,
    family: Option<&str>,
    seed: u64,
    history: &[SelectionRecord],
    recent_ids: &[String],
    practice_mode: &str,
    max_candidates: usize,
) -> Result<GeneratedProblem, String> {
    if !(1..=5).contains(&level) {
        return Err("難易度はLv1からLv5です。".into());
    }
    if !["standard", "method_choice", "alternatives"].contains(&practice_mode) {
        return Err("未登録の演習モードです。".into());
    }
    let mut candidates: Vec<_> = catalog()
        .into_iter()
        .filter(|v| v.level == level && family.is_none_or(|f| f == v.family))
        .collect();
    if candidates.is_empty() {
        return Err("選択した型は指定Lvに対応していません。".into());
    }
    let mut branch_counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut pattern_counts: BTreeMap<String, usize> = BTreeMap::new();
    let all = catalog();
    for h in history.iter().rev().take(2000) {
        if let Some(v) = all.iter().find(|v| v.id == h.variant && v.level == level) {
            *branch_counts.entry(&h.variant).or_default() += 1;
            *pattern_counts.entry(v.pattern.clone()).or_default() += 1;
        }
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    // Random tie breaks are deterministic for the same seed and history.
    let ties: BTreeMap<_, _> = candidates
        .iter()
        .map(|v| (v.id.clone(), rng.gen::<u64>()))
        .collect();
    candidates.sort_by_key(|v| {
        (
            *pattern_counts.get(&v.pattern).unwrap_or(&0),
            *branch_counts.get(v.id.as_str()).unwrap_or(&0),
            ties[&v.id],
        )
    });
    let recent: BTreeSet<_> = recent_ids.iter().rev().take(40).collect();
    let seen: BTreeSet<_> = history.iter().rev().take(2000).map(|h| &h.id).collect();
    let mut older_repeat = None;
    let mut last_error = String::new();
    let budget = max_candidates.clamp(1, 2000);
    // Keep a selected branch fixed during coefficient search. If it is exhausted,
    // progress to another underrepresented branch rather than looping forever.
    let mut attempts = 0;
    for v in &candidates {
        for _ in 0..24 {
            if attempts >= budget {
                break;
            }
            attempts += 1;
            let suffix = v.id.split_once('/').map(|(_, s)| s).unwrap_or("base");
            let profile = suffix.strip_prefix("profile_").and_then(|p| p.parse().ok());
            let result =
                generator::sample_recipe(&v.family, level, profile, suffix, seed, &mut rng)
                    .and_then(|r| generator::replay(&r));
            match result {
                Ok(p)
                    if p.classification.level == level
                        && p.classification.numeric_cost <= 8
                        && !recent.contains(&p.id) =>
                {
                    if p.variant.as_ref().map(|actual| &actual.id) != Some(&v.id) {
                        last_error = "細分類の再現結果が選択と一致しません。".into();
                        continue;
                    }
                    if practice_mode == "alternatives"
                        && p.routes.iter().filter(|r| r.complete).count() < 2
                    {
                        last_error = "完結した別解がある候補を探しています。".into();
                        break;
                    }
                    // Reproduction derives exactly the same metadata from the Recipe.
                    // Prefer a question not encountered in the longer history.
                    // Tiny branches can be exhausted; preserve a bounded fallback.
                    if seen.contains(&p.id) {
                        if older_repeat.is_none() {
                            older_repeat = Some(p);
                        }
                        continue;
                    }
                    return Ok(p);
                }
                Ok(_) => last_error = "難易度・計算量・直近の重複条件で候補を除外しました。".into(),
                Err(e) => last_error = e,
            }
        }
    }
    if let Some(p) = older_repeat {
        return Ok(p);
    }
    Err(format!("{attempts}候補以内に条件を満たす問題が見つかりませんでした。型や演習モードを変更できます。{last_error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_advertised_branch_can_generate_at_its_level() {
        for v in catalog() {
            let mut rng = ChaCha8Rng::seed_from_u64(20261017);
            let suffix = v.id.split_once('/').unwrap().1;
            let profile = suffix.strip_prefix("profile_").and_then(|p| p.parse().ok());
            let mut last = String::new();
            let found = (0..64).any(|_| {
                match generator::sample_recipe(
                    &v.family, v.level, profile, suffix, 20261017, &mut rng,
                )
                .and_then(|r| generator::replay(&r))
                {
                    Ok(p) => {
                        last = format!(
                            "actual Lv{}, variant {:?}",
                            p.classification.level,
                            p.variant.as_ref().map(|v| &v.id)
                        );
                        p.classification.level == v.level
                            && p.classification.numeric_cost <= 8
                            && p.variant.as_ref().map(|v| &v.id) == Some(&v.id)
                    }
                    Err(e) => {
                        last = e;
                        false
                    }
                }
            });
            assert!(
                found,
                "unreachable advertised branch {} (Lv{}): {}",
                v.id, v.level, last
            );
        }
    }
}
