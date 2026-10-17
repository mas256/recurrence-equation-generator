//! Bounded exact checks of solution equations; never an all-index proof.
use crate::generator::{DerivationIR, ProblemIR};
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::One;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 64;
const MAX_BITS: u64 = 16384;
const MAX_INDEX: u32 = 256;
const MAX_POWER: u32 = 4096;
const MAX_OPERATIONS: usize = 200_000;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteValidation {
    pub status: String,
    pub checked_terms: u32,
    pub equations_checked: usize,
    pub unresolved_equations: usize,
    pub formal_verification: bool,
    pub notes: Vec<String>,
}
#[derive(Clone, Debug)]
enum Failure {
    Invalid(String),
    Unresolved(String),
}
type Value = Result<Rational, Failure>;
fn combine(left: Value, right: Value, f: impl FnOnce(Rational, Rational) -> Value) -> Value {
    match (left, right) {
        (Err(Failure::Invalid(e)), _) | (_, Err(Failure::Invalid(e))) => Err(Failure::Invalid(e)),
        (Err(e), _) | (_, Err(e)) => Err(e),
        (Ok(a), Ok(b)) => f(a, b),
    }
}
fn bounded(value: Rational) -> Value {
    if value.0.numer().bits() > MAX_BITS || value.0.denom().bits() > MAX_BITS {
        Err(Failure::Unresolved(
            "有理数の大きさが有限検査の上限を超えました。".into(),
        ))
    } else {
        Ok(value)
    }
}
fn shape_nat(value: &NatExpr, depth: usize, count: &mut usize) -> Result<(), String> {
    *count += 1;
    if depth > MAX_DEPTH || *count > MAX_NODES {
        return Err("自然数式が検査の構造上限を超えています。".into());
    }
    match value {
        NatExpr::Add { left, right }
        | NatExpr::Sub { left, right }
        | NatExpr::Mul { left, right } => {
            shape_nat(left, depth + 1, count)?;
            shape_nat(right, depth + 1, count)?;
        }
        NatExpr::Div {
            numerator,
            denominator,
        } => {
            shape_nat(numerator, depth + 1, count)?;
            shape_nat(denominator, depth + 1, count)?;
        }
        NatExpr::Pow { base, .. } => shape_nat(base, depth + 1, count)?,
        NatExpr::GeometricPower { exponent, .. } => shape_nat(exponent, depth + 1, count)?,
        NatExpr::Choose { n, .. } => shape_nat(n, depth + 1, count)?,
        NatExpr::Bound { name } if name.len() > 64 => return Err("束縛変数名が長すぎます。".into()),
        _ => {}
    }
    Ok(())
}
fn shape(value: &Expr, depth: usize, count: &mut usize) -> Result<(), String> {
    *count += 1;
    if depth > MAX_DEPTH || *count > MAX_NODES {
        return Err("解法の式が検査の構造上限を超えています。".into());
    }
    match value {
        Expr::Rational { value } => {
            bounded(value.clone()).map_err(|_| "解法の定数が大きすぎます。")?;
        }
        Expr::NatCast { value } => shape_nat(value, depth + 1, count)?,
        Expr::Term { sequence, index } => {
            if sequence.is_empty() || sequence.len() > 64 {
                return Err("数列名が不正です。".into());
            }
            shape_nat(index, depth + 1, count)?;
        }
        Expr::Add { terms } => {
            for x in terms {
                shape(x, depth + 1, count)?;
            }
        }
        Expr::Mul { factors } => {
            for x in factors {
                shape(x, depth + 1, count)?;
            }
        }
        Expr::Div {
            numerator,
            denominator,
        } => {
            shape(numerator, depth + 1, count)?;
            shape(denominator, depth + 1, count)?;
        }
        Expr::Pow { base, exponent } => {
            shape(base, depth + 1, count)?;
            shape_nat(exponent, depth + 1, count)?;
        }
        Expr::Factorial { index } => shape_nat(index, depth + 1, count)?,
        Expr::Log { base, argument } => {
            shape(base, depth + 1, count)?;
            shape(argument, depth + 1, count)?;
        }
        Expr::Sum {
            variable,
            lower,
            upper,
            body,
        } => {
            if variable.len() > 64 {
                return Err("総和の変数名が長すぎます。".into());
            }
            shape_nat(lower, depth + 1, count)?;
            shape_nat(upper, depth + 1, count)?;
            shape(body, depth + 1, count)?;
        }
    }
    Ok(())
}
fn references(expr: &Expr, result: &mut BTreeSet<String>) {
    match expr {
        Expr::Term { sequence, .. } => {
            result.insert(sequence.clone());
        }
        Expr::Add { terms } => {
            for x in terms {
                references(x, result);
            }
        }
        Expr::Mul { factors } => {
            for x in factors {
                references(x, result);
            }
        }
        Expr::Div {
            numerator,
            denominator,
        } => {
            references(numerator, result);
            references(denominator, result);
        }
        Expr::Pow { base, .. } => references(base, result),
        Expr::Log { base, argument } => {
            references(base, result);
            references(argument, result);
        }
        Expr::Sum { body, .. } => references(body, result),
        _ => {}
    }
}
fn acyclic(definitions: &BTreeMap<String, Expr>) -> Result<(), String> {
    fn visit(
        name: &str,
        defs: &BTreeMap<String, Expr>,
        active: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if done.contains(name) {
            return Ok(());
        }
        if !active.insert(name.into()) {
            return Err(format!("補助数列 {name} の定義が循環しています。"));
        }
        let mut deps = BTreeSet::new();
        if let Some(expr) = defs.get(name) {
            references(expr, &mut deps);
        }
        if name == "S" && !defs.contains_key("S") {
            deps.insert("a".into());
        }
        for dep in deps {
            if defs.contains_key(&dep) || dep == "S" {
                visit(&dep, defs, active, done)?;
            }
        }
        active.remove(name);
        done.insert(name.into());
        Ok(())
    }
    let mut done = BTreeSet::new();
    for name in definitions.keys() {
        visit(name, definitions, &mut BTreeSet::new(), &mut done)?;
    }
    Ok(())
}
struct Context {
    definitions: BTreeMap<String, Expr>,
    cache: BTreeMap<(String, u32), Value>,
    active: BTreeSet<(String, u32)>,
    operations: usize,
}
impl Context {
    fn tick(&mut self) -> Result<(), Failure> {
        self.operations += 1;
        if self.operations > MAX_OPERATIONS {
            Err(Failure::Unresolved(
                "解法検査の計算量上限に達しました。".into(),
            ))
        } else {
            Ok(())
        }
    }
    fn nat(&mut self, expr: &NatExpr, n: u32, bound: Option<(&str, u32)>) -> Result<u32, Failure> {
        self.tick()?;
        let overflow = || Failure::Unresolved("自然数式が計算上限を超えました。".into());
        match expr {
            NatExpr::Index => Ok(n),
            NatExpr::Constant { value } => Ok(*value),
            NatExpr::Bound { name } => bound
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value)
                .ok_or_else(|| Failure::Unresolved(format!("束縛変数 {name} がありません。"))),
            NatExpr::Add { left, right } => self
                .nat(left, n, bound)?
                .checked_add(self.nat(right, n, bound)?)
                .ok_or_else(overflow),
            NatExpr::Sub { left, right } => Ok(self
                .nat(left, n, bound)?
                .saturating_sub(self.nat(right, n, bound)?)),
            NatExpr::Mul { left, right } => self
                .nat(left, n, bound)?
                .checked_mul(self.nat(right, n, bound)?)
                .ok_or_else(overflow),
            NatExpr::Div {
                numerator,
                denominator,
            } => {
                let den = self.nat(denominator, n, bound)?;
                if den == 0 {
                    return Err(Failure::Invalid(
                        "自然数式に零による除算があります。".into(),
                    ));
                }
                Ok(self.nat(numerator, n, bound)? / den)
            }
            NatExpr::Pow { base, exponent } => self
                .nat(base, n, bound)?
                .checked_pow(*exponent)
                .ok_or_else(overflow),
            NatExpr::GeometricPower { base, exponent } => base
                .checked_pow(self.nat(exponent, n, bound)?)
                .ok_or_else(overflow),
            NatExpr::Choose { n: value, k } => {
                let m = self.nat(value, n, bound)?;
                if *k > m {
                    return Ok(0);
                }
                let k = (*k).min(m - k);
                if k > 512 {
                    return Err(Failure::Unresolved(
                        "二項係数の計算量が上限を超えました。".into(),
                    ));
                }
                let mut result = 1u64;
                for i in 1..=k {
                    self.tick()?;
                    result = result
                        .checked_mul(u64::from(m - k + i))
                        .ok_or_else(overflow)?
                        / u64::from(i);
                }
                u32::try_from(result).map_err(|_| overflow())
            }
        }
    }
    fn term(&mut self, name: &str, index: u32) -> Value {
        if index > MAX_INDEX {
            return Err(Failure::Unresolved(
                "数列の添字が有限検査の上限を超えました。".into(),
            ));
        }
        let key = (name.to_string(), index);
        if let Some(value) = self.cache.get(&key) {
            return value.clone();
        }
        if !self.active.insert(key.clone()) {
            return Err(Failure::Invalid(format!("{name} の計算が循環しています。")));
        }
        let result = if let Some(expr) = self.definitions.get(name).cloned() {
            if index == 0 {
                Err(Failure::Unresolved(format!(
                    "{name}_0 の定義域はこの検査では未確定です。"
                )))
            } else {
                self.eval(&expr, index, None)
            }
        } else if name == "S" {
            let mut value = Ok(Rational::integer(0));
            for k in 1..=index {
                let rhs = self.term("a", k);
                value = combine(value, rhs, |a, b| bounded(a.add(&b)));
            }
            value
        } else {
            Err(Failure::Unresolved(format!(
                "記号 {name} の値は束縛されていません。"
            )))
        };
        self.active.remove(&key);
        self.cache.insert(key, result.clone());
        result
    }
    fn eval(&mut self, expr: &Expr, n: u32, bound: Option<(&str, u32)>) -> Value {
        self.tick()?;
        match expr {
            Expr::Rational { value } => bounded(value.clone()),
            Expr::NatCast { value } => Ok(Rational::integer(i64::from(self.nat(value, n, bound)?))),
            Expr::Term { sequence, index } => {
                let k = self.nat(index, n, bound)?;
                self.term(sequence, k)
            }
            Expr::Add { terms } => {
                let mut value = Ok(Rational::integer(0));
                for x in terms {
                    let rhs = self.eval(x, n, bound);
                    value = combine(value, rhs, |a, b| bounded(a.add(&b)));
                }
                value
            }
            Expr::Mul { factors } => {
                let mut value = Ok(Rational::integer(1));
                for x in factors {
                    let rhs = self.eval(x, n, bound);
                    value = combine(value, rhs, |a, b| bounded(a.mul(&b)));
                }
                value
            }
            Expr::Div {
                numerator,
                denominator,
            } => {
                let lhs = self.eval(numerator, n, bound);
                let rhs = self.eval(denominator, n, bound);
                if matches!(&rhs,Ok(value)if value.is_zero()) {
                    return Err(Failure::Invalid(
                        "解法の途中式に零による除算があります。".into(),
                    ));
                }
                combine(lhs, rhs, |a, b| {
                    bounded(a.div(&b).map_err(Failure::Invalid)?)
                })
            }
            Expr::Pow { base, exponent } => {
                let exp = self.nat(exponent, n, bound)?;
                if exp > MAX_POWER {
                    return Err(Failure::Unresolved(
                        "累乗の指数が有限検査の上限を超えました。".into(),
                    ));
                }
                let value = self.eval(base, n, bound)?;
                if value
                    .0
                    .numer()
                    .bits()
                    .max(value.0.denom().bits())
                    .saturating_mul(u64::from(exp))
                    > MAX_BITS
                {
                    return Err(Failure::Unresolved(
                        "累乗結果が有限検査の上限を超えます。".into(),
                    ));
                }
                bounded(value.pow(exp))
            }
            Expr::Factorial { index } => {
                let k = self.nat(index, n, bound)?;
                if k > 512 {
                    return Err(Failure::Unresolved(
                        "階乗が有限検査の上限を超えました。".into(),
                    ));
                }
                bounded(Rational(BigRational::from_integer(
                    (1..=k).fold(BigInt::one(), |acc, v| acc * v),
                )))
            }
            Expr::Log { base, argument } => {
                let base = self.eval(base, n, bound);
                let arg = self.eval(argument, n, bound);
                combine(base, arg, exact_log)
            }
            Expr::Sum {
                variable,
                lower,
                upper,
                body,
            } => {
                let lo = self.nat(lower, n, bound)?;
                let hi = self.nat(upper, n, bound)?;
                if hi >= lo && hi - lo > MAX_INDEX {
                    return Err(Failure::Unresolved(
                        "総和の項数が有限検査の上限を超えました。".into(),
                    ));
                }
                let mut value = Ok(Rational::integer(0));
                for k in lo..=hi {
                    let rhs = self.eval(body, n, Some((variable, k)));
                    value = combine(value, rhs, |a, b| bounded(a.add(&b)));
                }
                value
            }
        }
    }
}
fn exact_log(base: Rational, argument: Rational) -> Value {
    if !base.is_positive() || base.is_one() || !argument.is_positive() {
        return Err(Failure::Invalid(
            "対数の底・真数が定義域条件を満たしません。".into(),
        ));
    }
    let one = Rational::integer(1);
    if argument.is_one() {
        return Ok(Rational::integer(0));
    }
    let mut sign = 1i64;
    let base = if base < one {
        sign = -sign;
        one.div(&base).map_err(Failure::Invalid)?
    } else {
        base
    };
    let target = if argument < one {
        sign = -sign;
        one.div(&argument).map_err(Failure::Invalid)?
    } else {
        argument
    };
    let mut power = one;
    for k in 1..=MAX_POWER {
        power = bounded(power.mul(&base))?;
        if power == target {
            return Ok(Rational::integer(sign * i64::from(k)));
        }
        if power > target {
            break;
        }
    }
    Err(Failure::Unresolved(
        "整数値でない対数は未検証です。実数の対数法則の証明は別途必要です。".into(),
    ))
}
fn definition(eq: &Equation) -> Option<(&str, &Expr)> {
    match &eq.lhs {
        Expr::Term {
            sequence,
            index: NatExpr::Index,
        } => Some((sequence, &eq.rhs)),
        _ => None,
    }
}
/// Finite exact consistency checks against supplied general terms.
/// Independent recurrence/domain verification is also required at the call site.
pub fn validate(problem: &ProblemIR, derivation: &DerivationIR) -> Result<RouteValidation, String> {
    if derivation.auxiliaries.len() > 32 || derivation.steps.len() > 128 {
        return Err("解法の補助数列または手順が多すぎます。".into());
    }
    let mut definitions = BTreeMap::new();
    for eq in std::iter::once(&problem.general_term).chain(&problem.secondary_general_terms) {
        let (name, rhs) =
            definition(eq).ok_or("一般項の左辺は数列の第n項である必要があります。")?;
        if definitions.insert(name.to_string(), rhs.clone()).is_some() {
            return Err(format!("一般項の数列名 {name} が重複しています。"));
        }
    }
    let mut equations: Vec<(&Equation, String, u32)> = Vec::new();
    for eq in &problem.initials {
        equations.push((eq, "元の初期条件".into(), 1));
    }
    equations.push((
        &problem.recurrence,
        "元の漸化式".into(),
        problem.recurrence_start.max(1),
    ));
    for eq in &problem.secondary_recurrences {
        equations.push((eq, "元の連立漸化式".into(), problem.recurrence_start.max(1)));
    }
    let mut auxiliary_names = BTreeSet::new();
    for aux in &derivation.auxiliaries {
        let (name, rhs) = definition(&aux.definition)
            .ok_or("補助数列の定義の左辺は第n項である必要があります。")?;
        if name != aux.name
            || definitions.contains_key(name)
            || !auxiliary_names.insert(name.to_string())
        {
            return Err(format!(
                "補助数列 {} の名前が既存の定義と衝突しています。",
                aux.name
            ));
        }
        // S always means the partial sum of the original sequence. Treat its
        // displayed definition as an assertion, so a false definition cannot
        // silently replace that meaning.
        if name != "S" {
            definitions.insert(name.to_string(), rhs.clone());
        }
        equations.push((&aux.definition, format!("{} の定義", aux.name), 1));
        equations.push((&aux.initial, format!("{} の初期条件", aux.name), 1));
        equations.push((&aux.recurrence, format!("{} の漸化式", aux.name), 1));
    }
    for (index, step) in derivation.steps.iter().enumerate() {
        if let Some(eq) = &step.equation {
            // Only previously unknown n-indexed sequences become definitions.
            // Characteristic t_0/A_0 symbols are never numerically assigned.
            // Existing sequence names are assertions, never overwritten.
            if let Some((name, rhs)) = definition(eq) {
                if name != "S" && !definitions.contains_key(name) {
                    definitions.insert(name.to_string(), rhs.clone());
                }
            }
            equations.push((eq, format!("手順 {}", index + 1), 1));
        }
    }
    for expr in definitions.values() {
        shape(expr, 0, &mut 0)?;
    }
    for (eq, _, _) in &equations {
        shape(&eq.lhs, 0, &mut 0)?;
        shape(&eq.rhs, 0, &mut 0)?;
    }
    acyclic(&definitions)?;
    let checked_terms = if definitions.values().any(|x| x.exponent_degree() >= 3) {
        4
    } else {
        12
    };
    let mut context = Context {
        definitions,
        cache: BTreeMap::new(),
        active: BTreeSet::new(),
        operations: 0,
    };
    let mut result=RouteValidation{
  status:"finite_checked".into(),checked_terms,equations_checked:0,unresolved_equations:0,formal_verification:false,
  notes:vec![format!("n=1..{checked_terms} の有限項の厳密有理数照合です。元の漸化式は指定の開始添字以降で検査します。全添字の証明・変形の同値性・解法の完全性は保証しません。"),"原問題の定義域条件は、呼出側の独立した検証処理でも確認する必要があります。".into()],
 };
    let mut reasons = BTreeSet::new();
    for (eq, label, start) in equations {
        let mut unresolved = start > checked_terms;
        if unresolved {
            reasons.insert("検査範囲内に有効な添字がない式があります。".into());
        }
        for n in start..=checked_terms {
            let lhs = context.eval(&eq.lhs, n, None);
            let rhs = context.eval(&eq.rhs, n, None);
            match combine(lhs, rhs, |a, b| {
                if a == b {
                    Ok(a)
                } else {
                    Err(Failure::Invalid(format!(
                        "左右の値が一致しません ({} ≠ {})。",
                        a, b
                    )))
                }
            }) {
                Ok(_) => {}
                Err(Failure::Invalid(e)) => return Err(format!("{label}、n={n}: {e}")),
                Err(Failure::Unresolved(e)) => {
                    unresolved = true;
                    reasons.insert(e);
                }
            }
        }
        if unresolved {
            result.unresolved_equations += 1;
        } else {
            result.equations_checked += 1;
        }
    }
    if result.unresolved_equations > 0 {
        result.status = "partially_checked".into();
    }
    result.notes.extend(reasons);
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{AuxiliarySequence, DerivationStep, RecurrenceKind};
    fn fixture() -> (ProblemIR, DerivationIR) {
        let problem = ProblemIR {
            kind: RecurrenceKind::FirstOrder,
            index_start: 1,
            recurrence_start: 1,
            sequence_type: "rational".into(),
            recurrence: Equation::new(
                Expr::term("a", 1),
                Expr::add(vec![Expr::term("a", 0), Expr::integer(1)]),
            ),
            initials: vec![Equation::new(
                Expr::Term {
                    sequence: "a".into(),
                    index: NatExpr::constant(1),
                },
                Expr::integer(1),
            )],
            general_term: Equation::new(Expr::term("a", 0), Expr::index()),
            secondary_recurrences: vec![],
            secondary_general_terms: vec![],
            conditions: vec![],
        };
        let aux = AuxiliarySequence {
            name: "u".into(),
            definition: Equation::new(
                Expr::term("u", 0),
                Expr::mul(vec![Expr::integer(2), Expr::term("a", 0)]),
            ),
            initial: Equation::new(
                Expr::Term {
                    sequence: "u".into(),
                    index: NatExpr::constant(1),
                },
                Expr::integer(2),
            ),
            recurrence: Equation::new(
                Expr::term("u", 1),
                Expr::add(vec![Expr::term("u", 0), Expr::integer(2)]),
            ),
        };
        let derivation = DerivationIR {
            method_name: "test".into(),
            hint: "".into(),
            auxiliaries: vec![aux],
            steps: vec![],
            alternatives: vec![],
        };
        (problem, derivation)
    }
    fn step(eq: Equation) -> DerivationStep {
        DerivationStep {
            text: "".into(),
            equation: Some(eq),
            reason: "".into(),
            conditions: vec![],
        }
    }
    #[test]
    fn rejects_corrupt_auxiliary_initial_recurrence_and_printed_step() {
        let (p, d) = fixture();
        assert_eq!(validate(&p, &d).unwrap().status, "finite_checked");
        let mut bad = d.clone();
        bad.auxiliaries[0].initial.rhs = Expr::integer(3);
        assert!(validate(&p, &bad).unwrap_err().contains("初期条件"));
        let mut bad = d.clone();
        bad.auxiliaries[0].recurrence.rhs = Expr::integer(9);
        assert!(validate(&p, &bad).unwrap_err().contains("漸化式"));
        let mut bad = d;
        bad.steps
            .push(step(Equation::new(Expr::term("a", 0), Expr::integer(99))));
        assert!(validate(&p, &bad).unwrap_err().contains("手順"));
    }
    #[test]
    fn rejects_cycles_collisions_and_zero_division() {
        let (p, mut d) = fixture();
        d.steps
            .push(step(Equation::new(Expr::term("v", 0), Expr::term("w", 0))));
        d.steps
            .push(step(Equation::new(Expr::term("w", 0), Expr::term("v", 0))));
        assert!(validate(&p, &d).unwrap_err().contains("循環"));
        let (p, mut d) = fixture();
        d.auxiliaries.push(d.auxiliaries[0].clone());
        assert!(validate(&p, &d).unwrap_err().contains("衝突"));
        let (p, mut d) = fixture();
        d.steps.push(step(Equation::new(
            Expr::integer(0),
            Expr::div(Expr::term("unknown", 0), Expr::integer(0)),
        )));
        assert!(validate(&p, &d).unwrap_err().contains("除算"));
    }
    #[test]
    fn characteristic_symbols_remain_unresolved() {
        let (p, mut d) = fixture();
        d.steps.push(step(Equation::new(
            Expr::integer(0),
            Expr::add(vec![Expr::term("lambda", 0), Expr::integer(2)]),
        )));
        let r = validate(&p, &d).unwrap();
        assert_eq!(r.status, "partially_checked");
        assert_eq!(r.unresolved_equations, 1);
        assert!(!r.formal_verification);
        assert!(r.notes.iter().any(|x| x.contains("lambda")));
    }
    #[test]
    fn checks_dependent_definitions_and_integer_logarithms() {
        let (p, mut d) = fixture();
        d.steps.push(step(Equation::new(
            Expr::term("v", 0),
            Expr::add(vec![Expr::term("u", 0), Expr::integer(1)]),
        )));
        d.steps.push(step(Equation::new(
            Expr::term("v", 1),
            Expr::add(vec![Expr::term("v", 0), Expr::integer(2)]),
        )));
        d.steps.push(step(Equation::new(
            Expr::integer(3),
            Expr::Log {
                base: Box::new(Expr::integer(2)),
                argument: Box::new(Expr::integer(8)),
            },
        )));
        assert_eq!(validate(&p, &d).unwrap().unresolved_equations, 0);
        d.steps.push(step(Equation::new(
            Expr::integer(0),
            Expr::Log {
                base: Box::new(Expr::integer(2)),
                argument: Box::new(Expr::integer(3)),
            },
        )));
        assert_eq!(validate(&p, &d).unwrap().unresolved_equations, 1);
    }
    #[test]
    fn natural_subtraction_matches_existing_semantics() {
        let mut c = Context {
            definitions: BTreeMap::new(),
            cache: BTreeMap::new(),
            active: BTreeSet::new(),
            operations: 0,
        };
        assert_eq!(
            c.nat(&NatExpr::offset(-2), 1, None).unwrap(),
            NatExpr::offset(-2).evaluate(1, None).unwrap()
        );
    }
    #[test]
    fn excessive_exponent_remains_unresolved() {
        let (p, mut d) = fixture();
        d.steps.push(step(Equation::new(
            Expr::integer(0),
            Expr::Pow {
                base: Box::new(Expr::integer(2)),
                exponent: NatExpr::constant(u32::MAX),
            },
        )));
        let r = validate(&p, &d).unwrap();
        assert_eq!(r.status, "partially_checked");
        assert_eq!(r.unresolved_equations, 1);
    }
    #[test]
    fn partial_sum_auxiliary_keeps_the_original_sum_meaning() {
        let (p, mut d) = fixture();
        d.auxiliaries.push(AuxiliarySequence {
            name: "S".into(),
            definition: Equation::new(
                Expr::term("S", 0),
                Expr::Sum {
                    variable: "k".into(),
                    lower: NatExpr::constant(1),
                    upper: NatExpr::n(),
                    body: Box::new(Expr::Term {
                        sequence: "a".into(),
                        index: NatExpr::Bound { name: "k".into() },
                    }),
                },
            ),
            initial: Equation::new(
                Expr::Term {
                    sequence: "S".into(),
                    index: NatExpr::constant(1),
                },
                Expr::integer(1),
            ),
            recurrence: Equation::new(
                Expr::term("S", 1),
                Expr::add(vec![Expr::term("S", 0), Expr::term("a", 1)]),
            ),
        });
        assert_eq!(validate(&p, &d).unwrap().status, "finite_checked");
        d.auxiliaries.last_mut().unwrap().definition.rhs = Expr::integer(99);
        assert!(validate(&p, &d).unwrap_err().contains("S の定義"));
    }
}
