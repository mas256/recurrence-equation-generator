//! A fixed problem bank generated ahead of time for a static web page.
//!
//! Every accepted problem passes the finite exact checks and a fresh Lean proof
//! of the original problem. `check` replays a published bank without Lean and
//! rejects any record that differs from what the Recipe regenerates.
use crate::{
    finite_check,
    generator::{self, GeneratedProblem, Recipe},
    math::Equation,
    selection::{self, SelectionRecord},
    verifier::{self, VerificationError, Verifier},
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{atomic::AtomicBool, Arc, Mutex},
    time::{Duration, Instant},
};

pub const SCHEMA_VERSION: &str = "bank-1";
pub const LEVELS: std::ops::RangeInclusive<u8> = 1..=5;
const CANDIDATE_LIMIT: usize = 200;
/// Short IDs are used in shared links; collisions are rejected when building.
const PUBLIC_ID_LENGTH: usize = 12;

pub struct Options {
    pub seed: u64,
    pub per_level: usize,
    pub jobs: usize,
    pub lean_timeout: Duration,
    pub max_lean_failures: usize,
}

fn digest(bytes: impl AsRef<[u8]>) -> String {
    hex::encode(Sha256::digest(bytes.as_ref()))
}

fn latex_list<'a>(equations: impl IntoIterator<Item = &'a Equation>) -> Vec<String> {
    equations.into_iter().map(Equation::latex).collect()
}

/// `system`: two sequences, `positive`: a positive sequence, otherwise `single`.
fn shape(problem: &GeneratedProblem) -> &'static str {
    if !problem.problem.secondary_recurrences.is_empty() {
        "system"
    } else if problem
        .problem
        .conditions
        .iter()
        .any(|c| c.kind == "positive")
    {
        "positive"
    } else {
        "single"
    }
}

fn instruction(shape: &str) -> &'static str {
    match shape {
        "system" => "次の条件で定められる2つの数列について、一般項をそれぞれ求めよ。",
        "positive" => "次の条件を満たす正の数列について、一般項を求めよ。",
        _ => "次の条件で定められる数列について、一般項を求めよ。",
    }
}

/// Explanations keep prose and equations separate; the page renders `\[...\]`.
fn steps(solution: &[generator::SolutionStep]) -> Vec<String> {
    solution
        .iter()
        .map(|step| match &step.latex {
            Some(latex) => format!("{}\n\\[{}\\]", step.text, latex),
            None => step.text.clone(),
        })
        .collect()
}

/// The page record contains only content regenerated from the Recipe, so
/// `check` can compare it exactly. Verification fields are added separately.
pub fn page_record(problem: &GeneratedProblem, finite: &Value) -> Result<Value, String> {
    let variant = problem
        .variant
        .as_ref()
        .ok_or("出題分類の情報がありません。")?;
    let mut routes: Vec<Value> = problem
        .routes
        .iter()
        .filter(|r| r.complete)
        .map(|r| {
            json!({
                "title": r.method_name, "method_id": r.method_id, "hint": r.hint,
                "steps": steps(&r.solution), "level": r.level, "score": r.score,
                "total_score": r.classification.total_score, "parts": r.classification.parts,
                "numeric_cost": r.classification.numeric_cost, "reason": r.classification.reason,
                "checked_terms": r.validation.checked_terms,
                "equations_checked": r.validation.equations_checked,
            })
        })
        .collect();
    if routes.is_empty() {
        routes.push(json!({
            "title": problem.derivation.method_name, "method_id": "primary", "hint": problem.hint,
            "steps": steps(&problem.solution), "level": problem.classification.level,
            "score": problem.classification.score,
            "total_score": problem.classification.total_score,
            "parts": problem.classification.parts,
            "numeric_cost": problem.classification.numeric_cost,
            "reason": problem.classification.reason,
            "checked_terms": finite["checked_terms"], "equations_checked": Value::Null,
        }));
    }
    let mut recipe = serde_json::to_value(&problem.recipe).map_err(|e| e.to_string())?;
    // Keep every u64 digit for JavaScript, as the browser engine does.
    recipe["seed"] = problem.recipe.seed.to_string().into();
    Ok(json!({
        "id": &problem.id[..PUBLIC_ID_LENGTH],
        "problem_hash": problem.id,
        "level": problem.classification.level,
        "family": problem.recipe.family,
        "family_label": problem.family_label,
        "variant": {"id": variant.id, "label": variant.label, "pattern": variant.pattern},
        "statement": {
            "shape": shape(problem),
            "instruction": instruction(shape(problem)),
            "recurrences_tex": latex_list(
                std::iter::once(&problem.problem.recurrence)
                    .chain(&problem.problem.secondary_recurrences)),
            "initials_tex": latex_list(&problem.problem.initials),
            "range_tex": format!("n\\ge{}", problem.problem.index_start),
        },
        "answer_tex": latex_list(
            std::iter::once(&problem.problem.general_term)
                .chain(&problem.problem.secondary_general_terms)).join(",\\quad "),
        "routes": routes,
        "classification": {
            "level": problem.classification.level, "score": problem.classification.score,
            "total_score": problem.classification.total_score,
            "parts": problem.classification.parts,
            "numeric_cost": problem.classification.numeric_cost,
            "reason": problem.classification.reason,
            "rules_version": problem.classification.rules_version,
            "metrics": problem.classification.metrics,
        },
        "recipe": recipe,
        "finite_check": finite,
    }))
}

fn lean_record(source: &str, record: &verifier::VerificationRecord) -> Value {
    json!({
        "status": "lean-verified", "lean_version": record.lean_version,
        "proof_sha256": record.proof_hash, "environment_sha256": record.environment_hash,
        "checked_theorems": record.checked_theorems, "axioms": record.axioms,
        "elapsed_ms": record.elapsed_ms, "source": source,
    })
}

struct Candidate {
    problem: GeneratedProblem,
    finite: Value,
}

/// Choose candidates per level with the engine's curriculum balancing,
/// skipping repeats so every problem in the bank is distinct.
fn choose(
    level: u8,
    count: usize,
    rng: &mut ChaCha8Rng,
    history: &mut Vec<SelectionRecord>,
    taken: &mut BTreeSet<String>,
) -> Result<Vec<Candidate>, String> {
    let mut chosen = vec![];
    let mut attempts = 0;
    while chosen.len() < count {
        attempts += 1;
        if attempts > count * 50 + 200 {
            return Err(format!(
                "Lv{level}で重複しない問題を{count}問そろえられませんでした。"
            ));
        }
        let recent: Vec<_> = history
            .iter()
            .rev()
            .take(40)
            .map(|h| h.id.clone())
            .collect();
        let problem = match selection::generate(
            level,
            None,
            rng.next_u64(),
            history,
            &recent,
            "standard",
            CANDIDATE_LIMIT,
        ) {
            Ok(problem) => problem,
            Err(_) => continue,
        };
        if problem.classification.level != level
            || !taken.insert(problem.id[..PUBLIC_ID_LENGTH].to_string())
        {
            continue;
        }
        let finite = finite_check::exact_checks(&problem)?;
        history.push(SelectionRecord {
            id: problem.id.clone(),
            family: problem.recipe.family.clone(),
            variant: problem
                .variant
                .as_ref()
                .map(|v| v.id.clone())
                .unwrap_or_default(),
        });
        chosen.push(Candidate { problem, finite });
    }
    Ok(chosen)
}

/// Prove candidates in parallel. Each Lean run uses its own module.
fn prove(
    verifier: &Verifier,
    candidates: Vec<Candidate>,
    options: &Options,
) -> Vec<(Candidate, Result<Value, String>)> {
    let queue = Mutex::new(candidates.into_iter().enumerate().collect::<Vec<_>>());
    let results = Mutex::new(vec![]);
    std::thread::scope(|scope| {
        for _ in 0..options.jobs.max(1) {
            scope.spawn(|| loop {
                let Some((index, candidate)) = queue.lock().unwrap().pop() else {
                    return;
                };
                let outcome = verifier::proof_source(&candidate.problem).and_then(|source| {
                    verifier
                        .verify(
                            &candidate.problem,
                            Arc::new(AtomicBool::new(false)),
                            options.lean_timeout,
                        )
                        .map(|record| lean_record(&source, &record))
                        .map_err(|e: VerificationError| e.to_string())
                });
                eprintln!(
                    "Lv{} {}: {}",
                    candidate.problem.classification.level,
                    &candidate.problem.id[..PUBLIC_ID_LENGTH],
                    if outcome.is_ok() {
                        "Lean OK"
                    } else {
                        "Lean failed"
                    }
                );
                results.lock().unwrap().push((index, candidate, outcome));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(index, ..)| *index);
    results.into_iter().map(|(_, c, r)| (c, r)).collect()
}

pub fn build(verifier: &Verifier, options: &Options) -> Result<(Value, Value), String> {
    if options.per_level == 0 {
        return Err("各Lvの問題数は1以上にしてください。".into());
    }
    let environment = verifier.status();
    if !environment.ready {
        return Err(environment.message);
    }
    let started = Instant::now();
    let mut rng = ChaCha8Rng::seed_from_u64(options.seed);
    let mut history = vec![];
    let mut taken = BTreeSet::new();
    let mut problems = vec![];
    let mut lean_failures = vec![];
    for level in LEVELS {
        let mut accepted = vec![];
        while accepted.len() < options.per_level {
            let batch = choose(
                level,
                options.per_level - accepted.len(),
                &mut rng,
                &mut history,
                &mut taken,
            )?;
            for (candidate, outcome) in prove(verifier, batch, options) {
                match outcome {
                    Ok(lean) => {
                        let mut record = page_record(&candidate.problem, &candidate.finite)?;
                        record["lean"] = lean;
                        accepted.push(record);
                    }
                    Err(reason) => {
                        lean_failures.push(json!({
                            "problem_hash": candidate.problem.id, "level": level,
                            "family": candidate.problem.recipe.family,
                            "reason": reason.chars().take(2000).collect::<String>(),
                        }));
                        if lean_failures.len() > options.max_lean_failures {
                            return Err(format!(
                                "Lean検証の失敗が{}件を超えました。最後の失敗: {reason}",
                                options.max_lean_failures
                            ));
                        }
                    }
                }
            }
        }
        problems.extend(accepted);
    }
    let bank = json!({
        "schema_version": SCHEMA_VERSION,
        "engine_version": env!("CARGO_PKG_VERSION"),
        "selection_policy_version": selection::VERSION,
        "rules_version": generator::CURRENT_RULES_VERSION,
        "seed": options.seed.to_string(),
        "families": generator::family_catalog().into_iter()
            .map(|f| (f.id, f.label)).collect::<BTreeMap<_, _>>(),
        "problems": problems,
    });
    let bytes = serde_json::to_vec_pretty(&bank).map_err(|e| e.to_string())?;
    let counts: BTreeMap<String, usize> = LEVELS
        .map(|level| {
            let count = problems_at(&bank, level);
            (level.to_string(), count)
        })
        .collect();
    let manifest = json!({
        "schema_version": SCHEMA_VERSION,
        "status": "lean-verified",
        "engine_version": env!("CARGO_PKG_VERSION"),
        "seed": options.seed.to_string(),
        "count": problems_at(&bank, 0),
        "counts": counts,
        "lean_version": environment.version,
        "finite_checked_terms": finite_check::CHECKED_TERMS,
        "lean_rejected": lean_failures,
        "elapsed_seconds": started.elapsed().as_secs(),
        "bank_sha256": digest(&bytes),
    });
    Ok((bank, manifest))
}

fn problems_at(bank: &Value, level: u8) -> usize {
    bank["problems"]
        .as_array()
        .map(|problems| {
            problems
                .iter()
                .filter(|p| level == 0 || p["level"] == level)
                .count()
        })
        .unwrap_or(0)
}

/// Check a published bank without Lean: each Recipe must regenerate the same
/// record, pass the finite checks and produce the proof that Lean accepted.
pub fn check(bank_bytes: &[u8], manifest: &Value) -> Result<String, String> {
    if manifest["bank_sha256"] != digest(bank_bytes) {
        return Err("problems.jsonのSHA-256がmanifest.jsonと一致しません。".into());
    }
    let bank: Value = serde_json::from_slice(bank_bytes).map_err(|e| e.to_string())?;
    if bank["schema_version"] != SCHEMA_VERSION
        || manifest["schema_version"] != SCHEMA_VERSION
        || manifest["status"] != "lean-verified"
    {
        return Err("問題バンクの形式が一致しません。".into());
    }
    if bank["engine_version"] != env!("CARGO_PKG_VERSION") {
        return Err(format!(
            "問題バンクはエンジン{}で生成されています。現在のエンジン{}で再生成してください。",
            bank["engine_version"],
            env!("CARGO_PKG_VERSION")
        ));
    }
    let problems = bank["problems"]
        .as_array()
        .ok_or("problemsがありません。")?;
    let mut ids = BTreeSet::new();
    for record in problems {
        let id = record["id"].as_str().unwrap_or_default();
        if !ids.insert(id.to_string()) {
            return Err(format!("問題ID {id} が重複しています。"));
        }
        let recipe: Recipe =
            serde_json::from_value(record["recipe"].clone()).map_err(|e| format!("{id}: {e}"))?;
        let problem = generator::replay(&recipe).map_err(|e| format!("{id}: {e}"))?;
        let finite = finite_check::exact_checks(&problem).map_err(|e| format!("{id}: {e}"))?;
        let mut expected = page_record(&problem, &finite)?;
        let lean = &record["lean"];
        let source = verifier::proof_source(&problem)?;
        if lean["status"] != "lean-verified"
            || lean["source"] != source
            || lean["proof_sha256"] != digest(source.as_bytes())
            || lean["axioms"].as_array().is_none_or(|axioms| {
                axioms.iter().any(|a| {
                    !["propext", "Classical.choice", "Quot.sound"]
                        .contains(&a.as_str().unwrap_or(""))
                })
            })
        {
            return Err(format!(
                "{id}: Lean証明の記録が再生成した証明と一致しません。"
            ));
        }
        expected["lean"] = lean.clone();
        if expected != *record {
            return Err(format!(
                "{id}: 問題・解法がRecipeから再生成した内容と一致しません。"
            ));
        }
    }
    for level in LEVELS {
        if manifest["counts"][level.to_string()] != problems_at(&bank, level) {
            return Err(format!("Lv{level}の問題数がmanifest.jsonと一致しません。"));
        }
    }
    Ok(format!(
        "Checked {} bank problems: Recipe replay, finite exact checks and Lean proof sources",
        problems.len()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble a bank as `build` does, with a Lean record taken from the
    /// proof source; the actual Lean runs are covered by the ignored tests.
    fn sample_bank() -> (Vec<u8>, Value) {
        let mut rng = ChaCha8Rng::seed_from_u64(20261023);
        let mut history = vec![];
        let mut taken = BTreeSet::new();
        let mut problems = vec![];
        for level in LEVELS {
            for candidate in choose(level, 4, &mut rng, &mut history, &mut taken).unwrap() {
                let source = verifier::proof_source(&candidate.problem).unwrap();
                let mut record = page_record(&candidate.problem, &candidate.finite).unwrap();
                record["lean"] = json!({
                    "status": "lean-verified", "lean_version": "4.19.0",
                    "proof_sha256": digest(source.as_bytes()), "environment_sha256": "test",
                    "checked_theorems": [], "axioms": ["propext"], "elapsed_ms": 0,
                    "source": source,
                });
                problems.push(record);
            }
        }
        let bank = json!({"schema_version": SCHEMA_VERSION,
            "engine_version": env!("CARGO_PKG_VERSION"), "problems": problems});
        let bytes = serde_json::to_vec(&bank).unwrap();
        let manifest = json!({"schema_version": SCHEMA_VERSION, "status": "lean-verified",
            "counts": {"1": 4, "2": 4, "3": 4, "4": 4, "5": 4}, "bank_sha256": digest(&bytes)});
        (bytes, manifest)
    }

    #[test]
    fn bank_is_distinct_balanced_and_replayable() {
        let (bytes, manifest) = sample_bank();
        check(&bytes, &manifest).unwrap();
        let bank: Value = serde_json::from_slice(&bytes).unwrap();
        for p in bank["problems"].as_array().unwrap() {
            assert_eq!(p["id"].as_str().unwrap().len(), PUBLIC_ID_LENGTH);
            assert!(!p["routes"].as_array().unwrap().is_empty());
            assert!(!p["statement"]["recurrences_tex"]
                .as_array()
                .unwrap()
                .is_empty());
            assert_eq!(p["finite_check"]["recipe_replayed"], true);
        }
    }

    #[test]
    fn tampered_bank_is_rejected() {
        let (bytes, manifest) = sample_bank();
        let mut bank: Value = serde_json::from_slice(&bytes).unwrap();
        let rehash = |bank: &Value| {
            let bytes = serde_json::to_vec(bank).unwrap();
            let mut manifest = manifest.clone();
            manifest["bank_sha256"] = digest(&bytes).into();
            (bytes, manifest)
        };
        let original = bank.clone();
        bank["problems"][0]["answer_tex"] = "a_{n}=0".into();
        let (b, m) = rehash(&bank);
        assert!(check(&b, &m).is_err());
        bank = original.clone();
        bank["problems"][1]["lean"]["source"] = "theorem x : True := trivial".into();
        let (b, m) = rehash(&bank);
        assert!(check(&b, &m).is_err());
        bank = original.clone();
        bank["problems"][2]["lean"]["axioms"] = json!(["sorryAx"]);
        let (b, m) = rehash(&bank);
        assert!(check(&b, &m).is_err());
        let mut m = manifest.clone();
        m["bank_sha256"] = "0".into();
        assert!(check(&bytes, &m).is_err());
    }
}
