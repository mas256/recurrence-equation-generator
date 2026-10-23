//! Finite exact checks shared by the browser engine and the native bank builder.
//!
//! These checks cover a finite set of indices with exact rational arithmetic.
//! They complement, and never replace, the Lean proof of the original problem.
use crate::{
    generator::{self, GeneratedProblem},
    math::{Expr, NatExpr, Rational},
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const CHECKED_TERMS: u32 = 20;

/// Replay the Recipe, then compare the general term with every initial
/// condition, recurrence and domain condition up to `CHECKED_TERMS`.
pub fn exact_checks(problem: &GeneratedProblem) -> Result<Value, String> {
    let replayed = generator::replay(&problem.recipe)?;
    if serde_json::to_value(&replayed).map_err(|e| e.to_string())?
        != serde_json::to_value(problem).map_err(|e| e.to_string())?
    {
        return Err("Recipeと表示する問題・解法の内容が一致しません。".into());
    }
    if let Some(mut check) = crate::extensions::exponential::finite_checks(&problem.problem)? {
        check["status"] = json!("finite_checked");
        check["checked_terms"] = json!(20);
        check["problem_hash"] = json!(problem.id);
        check["recipe_replayed"] = json!(true);
        check["formal_verification"] = json!(false);
        return Ok(check);
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
        "status":"finite_checked", "checked_terms":CHECKED_TERMS,
        "first_index":problem.problem.recurrence_start, "last_index":CHECKED_TERMS,
        "initials_checked":problem.problem.initials.len(), "recurrences_checked":recurrences.len(),
        "conditions_checked":problem.problem.conditions.len(), "recipe_replayed":true,
        "formal_verification":false, "problem_hash":problem.id,
    }))
}
