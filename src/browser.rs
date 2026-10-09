//! Browser entry points use the same exact IR, classifier and registered recipes.
//!
//! The checks here cover a finite set of indices. They do not run Lean in the
//! browser and must not be presented as a fresh universal proof.
use crate::{
    export,
    generator::{self, GeneratedProblem, Recipe},
    math::{Expr, NatExpr, Rational},
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

const CHECKED_TERMS: u32 = 20;
const CANDIDATE_LIMIT: usize = 200;
const MAX_JSON_BYTES: usize = 256 * 1024;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerateRequest {
    level: u8,
    #[serde(default)]
    family: Option<String>,
    seed: Value,
    #[serde(default)]
    recent_ids: Vec<String>,
}
fn parse_json(text: &str) -> Result<Value, String> {
    if text.len() > MAX_JSON_BYTES {
        return Err("JSONファイルは256 KiB以下にしてください。".into());
    }
    serde_json::from_str(text).map_err(|e| format!("JSONを読み込めません: {e}"))
}
fn seed(value: &Value) -> Result<u64, String> {
    match value {
        Value::String(text)
            if !text.is_empty() && text.len() <= 20 && text.bytes().all(|c| c.is_ascii_digit()) =>
        {
            text.parse::<u64>().map_err(|_| {
                "seedは0〜18446744073709551615の整数を文字列で指定してください。".into()
            })
        }
        Value::Number(number) => {
            let value = number.as_u64().or_else(|| {
                number
                    .as_f64()
                    .filter(|n| {
                        n.is_finite()
                            && *n >= 0.0
                            && n.fract() == 0.0
                            && *n <= MAX_SAFE_JS_INTEGER as f64
                    })
                    .map(|n| n as u64)
            });
            value.filter(|n| *n <= MAX_SAFE_JS_INTEGER).ok_or_else(|| "数値のseedはJavaScriptの安全な整数範囲で指定してください。大きいseedは文字列で指定できます。".into())
        }
        _ => Err("seedは整数の文字列またはJavaScriptの安全な整数を指定してください。".into()),
    }
}
fn normalize_recipe_seed(value: &mut Value) -> Result<(), String> {
    // Native Recipe files are handed to Rust as the original JSON text, so their
    // numeric u64 seeds have not passed through JavaScript's floating point parser.
    let raw = value.get("seed").ok_or("Recipeにseedがありません。")?;
    let parsed = raw.as_u64().map(Ok).unwrap_or_else(|| seed(raw))?;
    value
        .as_object_mut()
        .ok_or("RecipeはJSONオブジェクトを指定してください。")?
        .insert("seed".into(), json!(parsed));
    Ok(())
}
fn encode(problem: &GeneratedProblem, check: Value) -> Result<String, String> {
    let mut value = serde_json::to_value(problem).map_err(|e| e.to_string())?;
    value["recipe"]["seed"] = problem.recipe.seed.to_string().into();
    value
        .as_object_mut()
        .ok_or("問題をJSONに変換できません。")?
        .insert("browser_check".into(), check);
    serde_json::to_string(&value).map_err(|e| e.to_string())
}
fn exact_checks(problem: &GeneratedProblem) -> Result<Value, String> {
    let replayed = generator::replay(&problem.recipe)?;
    if serde_json::to_value(&replayed).map_err(|e| e.to_string())?
        != serde_json::to_value(problem).map_err(|e| e.to_string())?
    {
        return Err("Recipeと表示する問題・解法の内容が一致しません。".into());
    }
    let mut formulas = BTreeMap::new();
    for equation in std::iter::once(&problem.problem.general_term)
        .chain(&problem.problem.secondary_general_terms)
    {
        match &equation.lhs {
            Expr::Term {
                sequence,
                index: NatExpr::Index,
            } if !formulas.contains_key(sequence.as_str()) => {
                formulas.insert(sequence.as_str(), &equation.rhs);
            }
            _ => return Err("一般項の数列名または添字が登録規則と一致しません。".into()),
        }
    }
    // Recurrences can refer to n+1 and n+2, and sums can include those indices.
    let mut terms = BTreeMap::new();
    let closed = |_: &str, _: u32| Err("一般項には未解決の数列が残っています。".into());
    for (&sequence, formula) in &formulas {
        for n in 1..=CHECKED_TERMS + 2 {
            terms.insert((sequence, n), formula.evaluate(n, &closed)?);
        }
    }
    let term = |sequence: &str, n: u32| -> Result<Rational, String> {
        terms
            .get(&(sequence, n))
            .cloned()
            .ok_or_else(|| "登録範囲外の数列または添字です。".into())
    };
    for initial in &problem.problem.initials {
        if initial.lhs.evaluate(1, &term)? != initial.rhs.evaluate(1, &term)? {
            return Err("初期条件と一般項が一致しません。".into());
        }
    }
    let recurrences: Vec<_> = std::iter::once(&problem.problem.recurrence)
        .chain(&problem.problem.secondary_recurrences)
        .collect();
    for n in problem.problem.recurrence_start..=CHECKED_TERMS {
        for equation in &recurrences {
            if equation.lhs.evaluate(n, &term)? != equation.rhs.evaluate(n, &term)? {
                return Err(format!("n={n}の漸化式と一般項が一致しません。"));
            }
        }
        for condition in &problem.problem.conditions {
            let value = condition.expression.evaluate(n, &term)?;
            let satisfied = match condition.kind.as_str() {
                "nonzero" => !value.is_zero(),
                "positive" => value.is_positive(),
                _ => return Err("未登録の定義域条件です。".into()),
            };
            if !satisfied {
                return Err(format!("n={n}の定義域条件を満たしていません。"));
            }
        }
    }
    Ok(json!({
        "status":"browser_checked", "engine":"rust_wasm", "checked_terms":CHECKED_TERMS,
        "first_index":problem.problem.recurrence_start, "last_index":CHECKED_TERMS,
        "initials_checked":problem.problem.initials.len(), "recurrences_checked":recurrences.len(),
        "conditions_checked":problem.problem.conditions.len(), "recipe_replayed":true,
        "formal_verification":false, "lean_executed":false, "problem_hash":problem.id,
    }))
}

/// Catalog JSON is independent of the native Lean environment.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn browser_catalog() -> Result<String, String> {
    serde_json::to_string(&json!({
        "ready":true, "deployment_mode":"browser", "engine":"rust_wasm",
        "application_version":env!("CARGO_PKG_VERSION"), "families":generator::family_catalog(),
        "limits":{"candidate_limit":CANDIDATE_LIMIT}, "checked_terms":CHECKED_TERMS,
    }))
    .map_err(|e| e.to_string())
}

/// Accept a decimal u64 seed string, or a number in JavaScript's safe range.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn browser_generate(request_json: &str) -> Result<String, String> {
    let request: GenerateRequest =
        serde_json::from_value(parse_json(request_json)?).map_err(|e| e.to_string())?;
    let seed = seed(&request.seed)?;
    let family = request.family.as_deref().filter(|name| *name != "mixed");
    if !(1..=5).contains(&request.level) {
        return Err("難易度はLv1からLv5です。".into());
    }
    if family.is_some_and(|id| {
        !generator::family_catalog()
            .iter()
            .any(|info| info.id == id && info.levels.contains(&request.level))
    }) {
        return Err("選択した型は指定Lvに対応していません。".into());
    }
    if request.recent_ids.len() > 40
        || request.recent_ids.iter().any(|id| {
            id.len() != 64
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        })
    {
        return Err("recent_idsには直近40件以内の問題IDを指定してください。".into());
    }
    let recent: BTreeSet<_> = request.recent_ids.iter().map(String::as_str).collect();
    let mut last_error = "品質条件を満たす問題を生成できませんでした。".to_owned();
    for attempt in 0..CANDIDATE_LIMIT {
        match generator::generate_limited(
            request.level,
            family,
            seed.wrapping_add(attempt as u64),
            1,
        ) {
            Ok(problem)
                if problem.classification.level == request.level
                    && problem.classification.numeric_cost <= 8
                    && !recent.contains(problem.id.as_str()) =>
            {
                let check = exact_checks(&problem)?;
                return encode(&problem, check);
            }
            Ok(_) => last_error = "直近の問題と重複したため、別の候補を探しました。".into(),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "{CANDIDATE_LIMIT}件以内に条件を満たす問題が見つかりませんでした。{last_error}"
    ))
}

/// Canonical Recipe validation runs before finite exact arithmetic checks.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn browser_replay(recipe_json: &str) -> Result<String, String> {
    let mut value = parse_json(recipe_json)?;
    normalize_recipe_seed(&mut value)?;
    let recipe: Recipe = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let problem = generator::replay(&recipe)?;
    let check = exact_checks(&problem)?;
    encode(&problem, check)
}

/// Regenerate all display fields before exporting; supplied text cannot alter TeX.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn browser_tex(problem_json: &str, show_solution: bool) -> Result<String, String> {
    let mut value = parse_json(problem_json)?;
    value
        .as_object_mut()
        .ok_or("問題はJSONオブジェクトを指定してください。")?
        .remove("browser_check");
    normalize_recipe_seed(
        value
            .get_mut("recipe")
            .ok_or("問題にRecipeがありません。")?,
    )?;
    let problem: GeneratedProblem = serde_json::from_value(value).map_err(|e| e.to_string())?;
    exact_checks(&problem)?;
    Ok(export::tex(&problem, show_solution))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_family_is_checked_replayable_and_exportable() {
        assert_eq!(generator::family_catalog().len(), 28);
        for family in generator::family_catalog() {
            let generated = browser_generate(
                &json!({"level":family.levels[0],"family":family.id,"seed":"18446744073709551615"})
                    .to_string(),
            )
            .unwrap();
            let parsed: Value = serde_json::from_str(&generated).unwrap();
            assert!(parsed["recipe"]["seed"].is_string());
            assert_eq!(parsed["browser_check"]["checked_terms"], 20);
            assert_eq!(parsed["browser_check"]["formal_verification"], false);
            let replayed = browser_replay(&parsed["recipe"].to_string()).unwrap();
            assert_eq!(generated, replayed);
            let tex = browser_tex(&generated, true).unwrap();
            assert!(tex.contains(parsed["id"].as_str().unwrap()));
        }
    }
    #[test]
    fn unsafe_seeds_tampering_and_duplicates_are_handled() {
        for seed in [
            json!(9_007_199_254_740_992u64),
            json!(-1),
            json!(1.5),
            json!("18446744073709551616"),
            json!("abc"),
        ] {
            assert!(browser_generate(&json!({"level":1,"seed":seed}).to_string()).is_err());
        }
        let generated =
            browser_generate(r#"{"level":1,"family":"arithmetic","seed":"1"}"#).unwrap();
        let mut parsed: Value = serde_json::from_str(&generated).unwrap();
        let other = browser_generate(
            &json!({"level":1,"family":"arithmetic","seed":"1","recent_ids":[parsed["id"]]})
                .to_string(),
        )
        .unwrap();
        assert_ne!(
            serde_json::from_str::<Value>(&other).unwrap()["id"],
            parsed["id"]
        );
        parsed["answer_latex"] = "modified".into();
        assert!(browser_tex(&parsed.to_string(), true).is_err());
        let mut recipe = parsed["recipe"].clone();
        recipe["output"] = "modified".into();
        assert!(browser_replay(&recipe.to_string()).is_err());
    }
    #[test]
    fn native_numeric_u64_recipe_keeps_every_seed_digit() {
        let original = generator::generate(1, Some("arithmetic"), u64::MAX).unwrap();
        let raw_native_recipe = serde_json::to_string(&original.recipe).unwrap();
        let restored: Value =
            serde_json::from_str(&browser_replay(&raw_native_recipe).unwrap()).unwrap();
        assert_eq!(restored["recipe"]["seed"], u64::MAX.to_string());
        assert_eq!(restored["id"], original.id);
    }
    #[test]
    fn legacy_weighted_recipe_above_generation_budget_still_replays() {
        let mut recipe = generator::generate(5, Some("weighted_sum"), 42)
            .unwrap()
            .recipe;
        for c in [Rational::integer(16), Rational::fraction(25, 3)] {
            for r in 2..=5 {
                for s in 2..=5 {
                    if r == s {
                        continue;
                    }
                    let r = Rational::integer(r);
                    let s = Rational::integer(s);
                    let denominator = r.sub(&s);
                    let a = c
                        .mul(&r.sub(&Rational::integer(1)))
                        .div(&denominator)
                        .unwrap();
                    let b = c
                        .mul(&Rational::integer(1).sub(&s))
                        .div(&denominator)
                        .unwrap();
                    recipe.parameters.insert("c".into(), c.clone());
                    recipe.parameters.insert("r".into(), r.clone());
                    recipe.parameters.insert("s".into(), s.clone());
                    recipe.core.parameters.insert("amplitude".into(), a);
                    recipe.core.parameters.insert("ratio".into(), r);
                    recipe.blocks[0].parameters.insert("amplitude".into(), b);
                    recipe.blocks[0].parameters.insert("ratio".into(), s);
                    if let Ok(problem) = generator::replay(&recipe) {
                        if problem.classification.numeric_cost > 8 {
                            let restored: Value = serde_json::from_str(
                                &browser_replay(&serde_json::to_string(&recipe).unwrap()).unwrap(),
                            )
                            .unwrap();
                            assert_eq!(restored["id"], problem.id);
                            return;
                        }
                    }
                }
            }
        }
        panic!("expected a legacy weighted recipe above the new generation budget");
    }
    #[test]
    fn changed_problem_ir_is_rejected() {
        let original = generator::generate(4, Some("reciprocal_affine"), 64).unwrap();
        let mut changed = original.clone();
        changed.problem.initials[0].rhs = Expr::integer(999);
        assert!(exact_checks(&changed).is_err());
        changed = original.clone();
        changed.problem.recurrence.rhs = Expr::integer(999);
        assert!(exact_checks(&changed).is_err());
        changed = original;
        changed.problem.conditions[0].expression = Expr::integer(0);
        assert!(exact_checks(&changed).is_err());
    }
}
