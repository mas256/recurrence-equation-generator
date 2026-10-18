//! Reproducible audit: an empty pool, 200 questions per level, updated history.
use recurrence_lab::{
    generator,
    selection::{self, SelectionRecord},
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Instant,
};

fn main() -> Result<(), String> {
    let count = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(200);
    let mut history: Vec<SelectionRecord> = vec![];
    let mut ids = BTreeSet::new();
    let mut results = vec![];
    let mut timings = vec![];
    let mut repeats = 0;
    let mut routes = 0;
    let mut all_branches = BTreeSet::new();
    let mut branches: BTreeMap<String, usize> = BTreeMap::new();
    let mut patterns: BTreeMap<String, usize> = BTreeMap::new();
    let mut methods: BTreeMap<String, usize> = BTreeMap::new();
    // Round-robin levels keep the same global recent window throughout the run.
    for round in 0..count {
        for level in 1..=5 {
            let seed = 0x20261017_u64.wrapping_add((round * 5 + usize::from(level) - 1) as u64);
            let recent = history
                .iter()
                .rev()
                .take(40)
                .map(|h| h.id.clone())
                .collect::<Vec<_>>();
            let start = Instant::now();
            let p = selection::generate(level, None, seed, &history, &recent, "standard", 200)?;
            timings.push(start.elapsed().as_micros());
            assert_eq!(p.classification.level, level);
            assert!(!recent.contains(&p.id));
            let replay = generator::replay(&p.recipe)?;
            assert_eq!(
                serde_json::to_value(&p).unwrap(),
                serde_json::to_value(&replay).unwrap()
            );
            if !ids.insert(p.id.clone()) {
                repeats += 1
            }
            if p.routes.iter().filter(|r| r.complete).count() > 1 {
                routes += 1
            }
            *methods
                .entry(format!("Lv{level}/{}", p.routes[0].method_id))
                .or_default() += 1;
            let v = p.variant.ok_or("Missing variant metadata")?;
            *branches.entry(v.id.clone()).or_default() += 1;
            *patterns
                .entry(format!("Lv{level}/{}", v.pattern))
                .or_default() += 1;
            all_branches.insert(v.id.clone());
            history.push(SelectionRecord {
                id: p.id,
                family: v.family,
                variant: v.id,
            });
            if history.len() > 2000 {
                history.remove(0);
            }
        }
    }
    for level in 1..=5 {
        let eligible = selection::catalog()
            .into_iter()
            .filter(|v| v.level == level)
            .collect::<Vec<_>>();
        let covered = eligible
            .iter()
            .filter(|v| all_branches.contains(&v.id))
            .count();
        results.push(json!({"level":level,"generated":count,"eligible_branches":eligible.len(),"covered_branches":covered,
            "missing_branches":eligible.iter().filter(|v|!all_branches.contains(&v.id)).map(|v|&v.id).collect::<Vec<_>>()}));
    }
    timings.sort();
    println!("{}",serde_json::to_string_pretty(&json!({"rules_version":generator::CURRENT_RULES_VERSION,"seed_start":"0x20261017","level_order":"round_robin","per_level":count,"initial_pool":0,"selection_history_limit":2000,"duplicate_window":40,
        "generated":count*5,"unique":ids.len(),"repeats_outside_recent_window":repeats,"with_alternatives":routes,"levels":results,"pattern_counts":patterns,"primary_method_counts":methods,"branch_counts":branches,
        "elapsed_us":{"median":timings[timings.len()/2],"p95":timings[timings.len()*95/100],"max":timings.last()}})).unwrap());
    Ok(())
}
