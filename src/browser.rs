//! Browser entry points use the same exact IR, classifier and registered recipes.
//!
//! The checks here cover a finite set of indices. They do not run Lean in the
//! browser and must not be presented as a fresh universal proof.
use crate::{
    export,
    generator::{self, GeneratedProblem, Recipe},
};
use serde::Deserialize;
use serde_json::{json, Value};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::finite_check::CHECKED_TERMS;
const CANDIDATE_LIMIT: usize = 200;
const MAX_JSON_BYTES: usize = 512 * 1024;
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
    #[serde(default)]
    selection_history: Vec<crate::selection::SelectionRecord>,
    #[serde(default = "standard_mode")]
    practice_mode: String,
}
fn standard_mode() -> String {
    "standard".into()
}
fn parse_json(text: &str) -> Result<Value, String> {
    if text.len() > MAX_JSON_BYTES {
        return Err("JSONファイルは512 KiB以下にしてください。".into());
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
    let mut check = crate::finite_check::exact_checks(problem)?;
    check["status"] = json!("browser_checked");
    check["engine"] = json!("rust_wasm");
    check["lean_executed"] = json!(false);
    Ok(check)
}

/// Catalog JSON is independent of the native Lean environment.
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub fn browser_catalog() -> Result<String, String> {
    serde_json::to_string(&json!({
        "ready":true, "deployment_mode":"browser", "engine":"rust_wasm",
        "application_version":env!("CARGO_PKG_VERSION"), "families":generator::family_catalog(),
        "methods":crate::learning::methods(), "coverage":crate::coverage::catalog(),
        "variants":crate::selection::catalog(), "selection_policy_version":crate::selection::VERSION, "assessment_version":crate::learning::ASSESSMENT_VERSION,
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
    if request.selection_history.len() > 2000 {
        return Err("選択履歴は直近2,000件までです。".into());
    }
    let problem = crate::selection::generate(
        request.level,
        family,
        seed,
        &request.selection_history,
        &request.recent_ids,
        &request.practice_mode,
        CANDIDATE_LIMIT,
    )?;
    let check = exact_checks(&problem)?;
    encode(&problem, check)
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
    use crate::math::{Expr, Rational};
    #[test]
    fn every_family_is_checked_replayable_and_exportable() {
        assert!(generator::family_catalog().len() > 28);
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
        let original = generator::generate(3, Some("reciprocal_affine"), 64).unwrap();
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
