//! Exponential forcing and positive power recurrences, recognized from statements.
use crate::generator::{
    AuxiliarySequence, Classification, DerivationIR, DerivationStep, DomainCondition, FamilyInfo,
    ProblemIR, Recipe, RecipeBlock, RecipeCore, RecurrenceKind,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_bigint::BigInt;
use num_traits::Signed;
#[cfg(any(feature = "browser", test))]
use num_traits::{ToPrimitive, Zero};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
#[cfg(any(feature = "browser", test))]
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
type Params = BTreeMap<String, Rational>;
fn params(items: &[(&str, Rational)]) -> Params {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
fn rat(r: &Rational) -> Expr {
    Expr::rational(r.clone())
}
fn pow(r: &Rational, n: NatExpr) -> Expr {
    Expr::pow(rat(r), n)
}
fn literal(e: &Expr) -> Option<Rational> {
    if let Expr::Rational { value } = e {
        Some(value.clone())
    } else {
        None
    }
}
fn ini(r: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: "a".into(),
            index: NatExpr::constant(1),
        },
        rat(&r),
    )
}
fn bounded(r: &Rational, num: i64, den: i64) -> bool {
    r.0.numer().abs() <= BigInt::from(num) && r.0.denom() <= &BigInt::from(den)
}
pub fn supports(f: &str) -> bool {
    matches!(
        f,
        "cases_exponential_forcing" | "cases_logarithmic" | "cases_shifted_logarithmic"
    )
}
pub fn catalog() -> Vec<FamilyInfo> {
    vec![
        FamilyInfo {
            id: "cases_exponential_forcing".into(),
            label: "指数付加項・倍率一致と退化".into(),
            levels: vec![1, 2, 3],
        },
        FamilyInfo {
            id: "cases_logarithmic".into(),
            label: "対数で一次型に帰着".into(),
            levels: vec![1, 3],
        },
        FamilyInfo {
            id: "cases_shifted_logarithmic".into(),
            label: "平行移動して対数をとる".into(),
            levels: vec![1, 2, 4],
        },
    ]
}
pub struct VariantInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub level: u8,
}
pub fn variants(f: &str) -> Vec<VariantInfo> {
    let entries: &[(&str, &str, u8)] = match f {
        "cases_exponential_forcing" => &[
            ("zero_forcing", "付加項ゼロ", 1),
            ("arithmetic", "倍率がともに1", 1),
            ("constant_tail", "第2項以降が定数", 1),
            ("zero_multiplier", "零の倍率", 2),
            ("resonant", "倍率一致", 3),
            ("nonresonant", "倍率不一致", 3),
        ],
        "cases_logarithmic" => &[
            ("zero_power", "指数0", 1),
            ("unit_power", "指数1", 1),
            ("fixed_initial", "初期値が定数解", 1),
            ("power_two", "指数2・対数", 3),
            ("power_three", "指数3・対数", 3),
        ],
        "cases_shifted_logarithmic" => &[
            ("zero_power", "指数0", 1),
            ("fixed_initial", "初期値が定数解", 1),
            ("unit_power", "指数1・平行移動", 2),
            ("power_two", "指数2・平行移動と対数", 4),
            ("power_three", "指数3・平行移動と対数", 4),
        ],
        _ => &[],
    };
    entries
        .iter()
        .map(|&(id, label, level)| VariantInfo { id, label, level })
        .collect()
}
pub fn sample_variant(f: &str, v: &str, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    if !variants(f).iter().any(|x| x.id == v) {
        return Err("未登録の指数・対数バリエーションです。".into());
    }
    if f == "cases_exponential_forcing" {
        let a = Rational::integer(rng.gen_range(1..=5));
        let c = Rational::integer(rng.gen_range(1..=3));
        let (p, r, c) = match v {
            "zero_forcing" => (
                Rational::integer([-3, -2, -1, 1, 2, 3][rng.gen_range(0..6)]),
                Rational::integer(2),
                Rational::integer(0),
            ),
            "arithmetic" => (Rational::integer(1), Rational::integer(1), c),
            "constant_tail" => (Rational::integer(0), Rational::integer(1), c),
            "zero_multiplier" => {
                if rng.gen_bool(0.5) {
                    (Rational::integer(0), Rational::integer(2), c)
                } else {
                    (Rational::integer(2), Rational::integer(0), c)
                }
            }
            "resonant" => {
                let p = [
                    Rational::integer(-2),
                    Rational::integer(2),
                    Rational::fraction(1, 2),
                ][rng.gen_range(0..3)]
                .clone();
                (p.clone(), p, c)
            }
            _ => {
                let p = Rational::integer([-2, -1, 1, 2][rng.gen_range(0..4)]);
                let r = Rational::integer(if rng.gen_bool(0.5) { -3 } else { 3 });
                (p, r, c)
            }
        };
        let a = if p != r && c == a.mul(&r.sub(&p)) {
            a.add(&Rational::integer(1))
        } else {
            a
        };
        Ok(params(&[("p", p), ("r", r), ("c", c), ("a", a)]))
    } else {
        let base = [
            Rational::integer(2),
            Rational::integer(3),
            Rational::fraction(1, 2),
            Rational::fraction(1, 3),
        ][rng.gen_range(0..4)]
        .clone();
        let q = match v {
            "zero_power" => 0,
            "unit_power" => 1,
            "power_two" => 2,
            "power_three" => 3,
            _ => rng.gen_range(2..=3),
        };
        let (d, e) = if v == "fixed_initial" {
            (0, 0)
        } else {
            (rng.gen_range(1..=2), rng.gen_range(0..=2))
        };
        let h = if f == "cases_shifted_logarithmic" {
            [-3, -2, -1, 1, 2, 3][rng.gen_range(0..6)]
        } else {
            0
        };
        Ok(params(&[
            ("base", base),
            ("q", Rational::integer(q)),
            ("d", Rational::integer(d)),
            ("e", Rational::integer(e)),
            ("h", Rational::integer(h)),
        ]))
    }
}
pub fn sample(f: &str, level: u8, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    let list = variants(f)
        .into_iter()
        .filter(|v| v.level == level)
        .collect::<Vec<_>>();
    if list.is_empty() {
        return Err("この型は指定されたLvに対応していません。".into());
    }
    sample_variant(f, list[rng.gen_range(0..list.len())].id, rng)
}
pub fn variant_id(recipe: &Recipe) -> Option<String> {
    let p = &recipe.parameters;
    let id = if recipe.family == "cases_exponential_forcing" {
        if p.get("c")?.is_zero() {
            "zero_forcing"
        } else if p.get("p")?.is_one() && p.get("r")?.is_one() {
            "arithmetic"
        } else if p.get("p")?.is_zero() && p.get("r")?.is_one() {
            "constant_tail"
        } else if p.get("p")?.is_zero() || p.get("r")?.is_zero() {
            "zero_multiplier"
        } else if p.get("p")? == p.get("r")? {
            "resonant"
        } else {
            "nonresonant"
        }
    } else if matches!(
        recipe.family.as_str(),
        "cases_logarithmic" | "cases_shifted_logarithmic"
    ) {
        if p.get("q")?.is_zero() {
            "zero_power"
        } else if p.get("d")?.is_zero() && p.get("e")?.is_zero() {
            "fixed_initial"
        } else {
            match p.get("q")?.integer_u32()? {
                1 => "unit_power",
                2 => "power_two",
                3 => "power_three",
                _ => return None,
            }
        }
    } else {
        return None;
    };
    Some(id.into())
}
pub fn validate_parameters(f: &str, p: &Params) -> Result<(), String> {
    let keys: &[&str] = match f {
        "cases_exponential_forcing" => &["p", "r", "c", "a"],
        "cases_logarithmic" | "cases_shifted_logarithmic" => &["base", "q", "d", "e", "h"],
        _ => return Err("未登録の指数・対数型です。".into()),
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != keys.iter().copied().collect() {
        return Err("指数・対数型のパラメータ集合が一致しません。".into());
    }
    if f == "cases_exponential_forcing" {
        if p.values().any(|v| !bounded(v, 12, 4)) {
            return Err("指数付加項は分子絶対値12以下・分母4以下です。".into());
        }
    } else {
        if ![
            Rational::integer(2),
            Rational::integer(3),
            Rational::fraction(1, 2),
            Rational::fraction(1, 3),
        ]
        .contains(&p["base"])
        {
            return Err("対数型の底は2、3、1/2、1/3です。".into());
        }
        if !p["q"].integer_u32().is_some_and(|v| v <= 3)
            || ["d", "e"]
                .iter()
                .any(|&k| !p[k].integer_u32().is_some_and(|v| v <= 2))
        {
            return Err("q=0..3、初期指数・付加指数=0..2です。".into());
        }
        if !bounded(&p["h"], 3, 1)
            || (f == "cases_logarithmic" && !p["h"].is_zero())
            || (f == "cases_shifted_logarithmic" && p["h"].is_zero())
        {
            return Err("平行移動の値が範囲外です。".into());
        }
    }
    Ok(())
}
pub fn build_recipe(f: &str, seed: u64, p: Params) -> Result<Recipe, String> {
    validate_parameters(f, &p)?;
    Ok(Recipe {
        schema_version: crate::generator::SCHEMA_VERSION.into(),
        rules_version: crate::generator::CURRENT_RULES_VERSION.into(),
        rng_version: crate::generator::RNG_VERSION.into(),
        family: f.into(),
        seed,
        core: RecipeCore {
            id: "core".into(),
            kind: if f == "cases_exponential_forcing" {
                "exponential_forcing"
            } else {
                "positive_power"
            }
            .into(),
            parameters: p.clone(),
        },
        blocks: vec![RecipeBlock {
            id: "b1".into(),
            input: "core".into(),
            kind: "case_parameters_v1".into(),
            parameters: p.clone(),
        }],
        output: "b1".into(),
        index_start: 1,
        parameters: p,
    })
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    Forcing {
        p: Rational,
        r: Rational,
        c: Rational,
        first: Rational,
    },
    Logarithmic {
        base: Rational,
        q: u32,
        d: u32,
        e: u32,
        shift: Rational,
    },
}
pub fn logarithmic_exponent(q: u32, d: u32, e: u32) -> NatExpr {
    let k = NatExpr::offset(-1);
    if q == 0 {
        return NatExpr::constant(0);
    }
    if q == 1 {
        return NatExpr::add(NatExpr::constant(e), NatExpr::mul(NatExpr::constant(d), k));
    }
    let x = NatExpr::GeometricPower {
        base: q,
        exponent: Box::new(k),
    };
    NatExpr::add(
        NatExpr::mul(NatExpr::constant(e), x.clone()),
        NatExpr::mul(
            NatExpr::constant(d),
            NatExpr::Div {
                numerator: Box::new(NatExpr::Sub {
                    left: Box::new(x),
                    right: Box::new(NatExpr::constant(1)),
                }),
                denominator: Box::new(NatExpr::constant(q - 1)),
            },
        ),
    )
}
pub fn formula(route: &Route) -> Expr {
    match route {
        Route::Forcing { p, r, c, first } => {
            let k = NatExpr::offset(-1);
            let homogeneous = Expr::mul(vec![rat(first), pow(p, k.clone())]);
            let particular = if p == r {
                Expr::mul(vec![rat(c), Expr::offset(-1), pow(p, NatExpr::offset(-2))])
            } else {
                Expr::mul(vec![
                    rat(&c.div(&p.sub(r)).expect("distinct roots")),
                    Expr::sub(pow(p, k.clone()), pow(r, k)),
                ])
            };
            Expr::add(vec![homogeneous, particular])
        }
        Route::Logarithmic {
            base,
            q,
            d,
            e,
            shift,
        } => {
            let inner = if *d == 0 && *e == 0 {
                Expr::integer(1)
            } else if *q == 0 {
                let constant = base.pow(*d);
                Expr::add(vec![
                    rat(&constant),
                    Expr::mul(vec![
                        rat(&base.pow(*e).sub(&constant)),
                        pow(&Rational::integer(0), NatExpr::offset(-1)),
                    ]),
                ])
            } else {
                pow(base, logarithmic_exponent(*q, *d, *e))
            };
            Expr::add(vec![rat(shift), inner])
        }
    }
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    validate_parameters(&recipe.family, &recipe.parameters)?;
    let p = &recipe.parameters;
    let (route, rhs, conditions, first) = if recipe.family == "cases_exponential_forcing" {
        (
            Route::Forcing {
                p: p["p"].clone(),
                r: p["r"].clone(),
                c: p["c"].clone(),
                first: p["a"].clone(),
            },
            Expr::add(vec![
                Expr::mul(vec![rat(&p["p"]), Expr::term("a", 0)]),
                Expr::mul(vec![rat(&p["c"]), pow(&p["r"], NatExpr::offset(-1))]),
            ]),
            vec![],
            p["a"].clone(),
        )
    } else {
        let q = p["q"].integer_u32().unwrap();
        let d = p["d"].integer_u32().unwrap();
        let e = p["e"].integer_u32().unwrap();
        let shifted = Expr::sub(Expr::term("a", 0), rat(&p["h"]));
        (
            Route::Logarithmic {
                base: p["base"].clone(),
                q,
                d,
                e,
                shift: p["h"].clone(),
            },
            Expr::add(vec![
                rat(&p["h"]),
                Expr::mul(vec![
                    rat(&p["base"].pow(d)),
                    Expr::pow(shifted.clone(), NatExpr::constant(q)),
                ]),
            ]),
            vec![DomainCondition {
                kind: "positive".into(),
                expression: shifted,
                explanation:
                    "初期値の移動後とCは正なので帰納的にa_n−h>0。対数の底は正かつ1でない。".into(),
            }],
            p["h"].add(&p["base"].pow(e)),
        )
    };
    Ok(ProblemIR {
        kind: RecurrenceKind::FirstOrder,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(Expr::term("a", 1), rhs),
        initials: vec![ini(first)],
        general_term: Equation::new(Expr::term("a", 0), formula(&route)),
        conditions,
        secondary_recurrences: vec![],
        secondary_general_terms: vec![],
    })
}
fn scalar_term(e: &Expr, target: &Expr) -> Option<Rational> {
    if e == target {
        return Some(Rational::integer(1));
    }
    if let Expr::Mul { factors } = e {
        let mut c = Rational::integer(1);
        let mut found = false;
        for factor in factors {
            if factor == target && !found {
                found = true;
            } else {
                c = c.mul(&literal(factor)?);
            }
        }
        if found {
            return Some(c);
        }
    }
    None
}
fn add_terms(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::Add { terms } => terms.iter().collect(),
        _ => vec![e],
    }
}
fn first_value(p: &ProblemIR) -> Option<Rational> {
    if p.initials.len() != 1 || p.initials[0].lhs != ini(Rational::integer(0)).lhs {
        return None;
    }
    literal(&p.initials[0].rhs)
}
fn integer_log(value: &Rational, base: &Rational) -> Option<u32> {
    (0..=2).find(|&e| base.pow(e) == *value)
}
pub fn detect(problem: &ProblemIR) -> Option<Route> {
    if problem.kind != RecurrenceKind::FirstOrder
        || problem.recurrence.lhs != Expr::term("a", 1)
        || !problem.secondary_recurrences.is_empty()
        || problem.index_start != 1
        || problem.recurrence_start != 1
    {
        return None;
    }
    let first = first_value(problem)?;
    let terms = add_terms(&problem.recurrence.rhs);
    for h in -3..=3 {
        let shift = Rational::integer(h);
        let shifted = Expr::sub(Expr::term("a", 0), rat(&shift));
        for q in 0..=3 {
            let target = Expr::pow(shifted.clone(), NatExpr::constant(q));
            let mut coefficient = None;
            let mut constant = Rational::integer(0);
            let mut valid = true;
            for term in &terms {
                if let Some(c) = scalar_term(term, &target) {
                    if coefficient.is_some() {
                        valid = false;
                        break;
                    }
                    coefficient = Some(c);
                } else if let Some(c) = literal(term) {
                    constant = constant.add(&c);
                } else {
                    valid = false;
                    break;
                }
            }
            if !valid || constant != shift {
                continue;
            }
            let Some(coefficient) = coefficient else {
                continue;
            };
            for base in [
                Rational::integer(2),
                Rational::integer(3),
                Rational::fraction(1, 2),
                Rational::fraction(1, 3),
            ] {
                if let (Some(d), Some(e)) = (
                    integer_log(&coefficient, &base),
                    integer_log(&first.sub(&shift), &base),
                ) {
                    return Some(Route::Logarithmic {
                        base,
                        q,
                        d,
                        e,
                        shift,
                    });
                }
            }
        }
    }
    let mut p = Rational::integer(0);
    let mut forcing = None;
    for term in terms {
        if let Some(v) = scalar_term(term, &Expr::term("a", 0)) {
            p = p.add(&v);
            continue;
        }
        let factors = match term {
            Expr::Mul { factors } => factors.iter().collect::<Vec<_>>(),
            _ => vec![term],
        };
        let mut c = Rational::integer(1);
        let mut r = None;
        for factor in factors {
            if let Expr::Pow { base, exponent } = factor {
                if *exponent != NatExpr::offset(-1) || r.is_some() {
                    return None;
                }
                r = Some(literal(base)?);
            } else {
                c = c.mul(&literal(factor)?);
            }
        }
        if let Some(r) = r {
            if forcing.is_some() {
                return None;
            }
            forcing = Some((r, c));
        } else if !c.is_zero() {
            return None;
        }
    }
    let (r, c) = forcing.unwrap_or((Rational::integer(1), Rational::integer(0)));
    if [&p, &r, &c, &first].iter().any(|v| !bounded(v, 12, 4)) {
        return None;
    }
    Some(Route::Forcing { p, r, c, first })
}
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    let Some(route) = detect(problem) else {
        return Ok(None);
    };
    let (score, method, reason, ops, discovery, domain) = match &route {
        Route::Forcing { p, r, c, first } => {
            if c.is_zero() {
                (
                    2,
                    "付加項ゼロの等比数列",
                    "付加項が消えるので等比数列",
                    vec!["geometric"],
                    0,
                    0,
                )
            } else if p.is_one() && r.is_one() {
                (
                    2,
                    "倍率一致1の等差数列",
                    "倍率がともに1なので等差数列",
                    vec!["constant", "difference_sum"],
                    0,
                    0,
                )
            } else if p.is_zero() && r.is_one() {
                (
                    1,
                    "第2項以降が一定",
                    "右辺にa_nがなく第2項以降は定数",
                    vec!["constant"],
                    0,
                    0,
                )
            } else if p != r && *c == first.mul(&r.sub(p)) {
                (
                    2,
                    "初期条件で一方の解が消える等比数列",
                    "初項と付加項の対応により公比rの等比数列",
                    vec!["geometric"],
                    0,
                    0,
                )
            } else if p.is_zero() || r.is_zero() {
                (
                    4,
                    "零の倍率を分けて直接求める",
                    "初項と最初の付加項を分ける",
                    vec!["geometric"],
                    1,
                    0,
                )
            } else if p == r {
                (
                    10,
                    "指数付加項の倍率一致",
                    "倍率で割ると一定の階差になる",
                    vec!["index_scale", "difference_sum"],
                    1,
                    0,
                )
            } else {
                (
                    9,
                    "指数形の特解を引く",
                    "同じ指数形の特解を引いて等比にする",
                    vec!["particular_solution", "geometric"],
                    1,
                    0,
                )
            }
        }
        Route::Logarithmic { q, d, e, shift, .. } => {
            if *q == 0 || (*d == 0 && *e == 0) {
                (
                    1,
                    "対数を使わず定数を判定",
                    "移動後が定数または第2項以降が一定",
                    vec!["constant"],
                    0,
                    0,
                )
            } else if *q == 1 && shift.is_zero() {
                (
                    2,
                    "指数1の等比数列",
                    "指数が1なので通常の等比数列",
                    vec!["geometric"],
                    0,
                    0,
                )
            } else if *q == 1 {
                (
                    5,
                    "平行移動して等比数列にする",
                    "移動後が等比数列",
                    vec!["shift", "geometric"],
                    1,
                    0,
                )
            } else if shift.is_zero() {
                (
                    11,
                    "対数で一次漸化式にする",
                    "正値を確認して対数をとり一次型を解く",
                    vec!["logarithm", "fixed_point", "geometric"],
                    1,
                    1,
                )
            } else {
                (
                    16,
                    "平行移動して対数をとる",
                    "正の移動後の数列に対数をとる",
                    vec!["shift", "logarithm", "fixed_point", "geometric"],
                    2,
                    1,
                )
            }
        }
    };
    Ok(Some(crate::generator::make_classification(
        problem, score, method, reason, &ops, discovery, domain, 0,
    )?))
}
fn step(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn solution(_problem: &ProblemIR, route: &Route, alternate: bool) -> Result<DerivationIR, String> {
    let mut auxiliaries = vec![];
    let mut steps = vec![];
    let hint;
    let method;
    match route {
        Route::Forcing { p, r, c, first } => {
            if c.is_zero() {
                hint = "付加項が零になるか確認する。";
                method = "付加項ゼロの等比数列";
                steps.push(step(
                    "付加項が零なので初項と公比から一般項を得る。",
                    None,
                    "等比数列",
                ));
            } else if p != r && *c == first.mul(&r.sub(p)) {
                hint = "初項から公比rの等比数列になっていないか確認する。";
                method = "初期条件で一方の解が消える等比数列";
                steps.push(step("c=a_1(r−p)なので、a_n=a_1r^(n−1)を代入すると元の漸化式を満たす。初期条件から解は一意である。",None,"元の式に代入する帰納法"));
            } else if p.is_one() && r.is_one() {
                hint = "指数係数がともに1であることに注目する。";
                method = "倍率一致1の等差数列";
                steps.push(step(
                    "与式はa_(n+1)=a_n+cなので等差数列の一般項を用いる。",
                    None,
                    "倍率が1の場合",
                ));
            } else if p.is_zero() || r.is_zero() {
                hint = "初項と第2項以降を分ける。";
                method = "零の倍率を分けて直接求める";
                let explicit = if p.is_zero() && r.is_zero() {
                    format!("a_1={first}、a_2={c}、n≥3ではa_n=0。")
                } else if p.is_zero() && r.is_one() {
                    format!("a_1={first}。右辺は一定なので、n≥2ではa_n={c}。")
                } else if p.is_zero() {
                    format!("a_1={first}。n≥2では、元の式を一つずらしてa_n={c}·({r})^(n−2)。")
                } else {
                    format!(
                        "a_1={first}、a_2={}。n≥2ではa_n=a_2·({p})^(n−2)。",
                        p.mul(first).add(c)
                    )
                };
                steps.push(step(&explicit, None, "初期条件と元の漸化式"));
                steps.push(step(
                    "下の一つの式では0^0=1（空積）とし、初項の例外も表す。",
                    None,
                    "場合分けした結果の統一表記",
                ));
            } else if alternate {
                hint = "隣接する2式から指数付加項を消去する。";
                method = "指数付加項を消して3項間にする";
                let eq = Equation::new(
                    Expr::term("a", 2),
                    Expr::add(vec![
                        Expr::mul(vec![rat(&p.add(r)), Expr::term("a", 1)]),
                        Expr::mul(vec![
                            rat(&Rational::integer(-1).mul(p).mul(r)),
                            Expr::term("a", 0),
                        ]),
                    ]),
                );
                steps.push(step(
                    "添字を一つ進めた式から元の式のr倍を引く。",
                    Some(eq),
                    "隣接2式の差",
                ));
                steps.push(step(
                    &format!(
                        "特性多項式は(t−{p})(t−{r})。{}",
                        if p == r {
                            "重根なので(A+B(n−1))p^(n−1)を用いる。"
                        } else {
                            "異なる2根の等比数列の一次結合を用いる。"
                        }
                    ),
                    None,
                    "特性方程式",
                ));
                steps.push(step(
                    &format!(
                        "元の式からa_2={}。a_1={}と合わせて定数を定める。",
                        p.mul(first).add(c),
                        first
                    ),
                    None,
                    "初期条件",
                ));
            } else if p == r {
                hint = "共通の倍率の(n−1)乗で割る。";
                method = "指数付加項の倍率一致";
                let definition = Equation::new(
                    Expr::term("b", 0),
                    Expr::div(Expr::term("a", 0), pow(p, NatExpr::offset(-1))),
                );
                let recurrence = Equation::new(
                    Expr::term("b", 1),
                    Expr::add(vec![Expr::term("b", 0), rat(&c.div(p)?)]),
                );
                let initial = Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(first),
                );
                auxiliaries.push(AuxiliarySequence {
                    name: "b".into(),
                    definition: definition.clone(),
                    initial,
                    recurrence: recurrence.clone(),
                });
                steps.push(step(
                    "p≠0なので補助数列を定義できる。",
                    Some(definition),
                    "非零の倍率",
                ));
                steps.push(step(
                    "元の式をp^nで割ると補助数列は等差数列となる。",
                    Some(recurrence),
                    "p=rの場合",
                ));
            } else {
                hint = "特解Kr^(n−1)を探す。";
                method = "指数形の特解を引く";
                let k = c.div(&r.sub(p))?;
                let definition = Equation::new(
                    Expr::term("b", 0),
                    Expr::sub(
                        Expr::term("a", 0),
                        Expr::mul(vec![rat(&k), pow(r, NatExpr::offset(-1))]),
                    ),
                );
                let recurrence = Equation::new(
                    Expr::term("b", 1),
                    Expr::mul(vec![rat(p), Expr::term("b", 0)]),
                );
                let initial = Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(&first.sub(&k)),
                );
                auxiliaries.push(AuxiliarySequence {
                    name: "b".into(),
                    definition: definition.clone(),
                    initial,
                    recurrence: recurrence.clone(),
                });
                steps.push(step(
                    &format!("特解Kr^(n−1)を代入するとK(r−p)=c。r≠pよりK={k}。"),
                    None,
                    "係数比較。倍率一致ではこの除算を使わない",
                ));
                steps.push(step(
                    "特解を引いた補助数列を定義する。",
                    Some(definition),
                    "特解を引く",
                ));
                steps.push(step(
                    "補助数列は公比pの等比数列になる。",
                    Some(recurrence),
                    "代入して整理",
                ));
            }
        }
        Route::Logarithmic {
            base,
            q,
            d,
            e,
            shift,
        } => {
            if *q <= 1 || (*d == 0 && *e == 0) {
                hint = "対数の前に指数0・1と定数解を確認する。";
                method = "対数を使わず退化を処理する";
                if *q == 0 {
                    steps.push(step(
                        &format!(
                            "指数が0なので、a_1={}、n≥2ではa_n={}。初項と第2項以降を分ける。",
                            shift.add(&base.pow(*e)),
                            shift.add(&base.pow(*d))
                        ),
                        None,
                        "元の漸化式へ直接代入",
                    ));
                } else if *d == 0 && *e == 0 {
                    steps.push(step(&format!("C=1、a_1−h=1。1の自然数乗は1なので、帰納的にすべての項がa_n={}となる。", shift.add(&Rational::integer(1))), None, "定数解と初期条件"));
                } else {
                    let definition = Equation::new(
                        Expr::term("b", 0),
                        Expr::sub(Expr::term("a", 0), rat(shift)),
                    );
                    let recurrence = Equation::new(
                        Expr::term("b", 1),
                        Expr::mul(vec![rat(&base.pow(*d)), Expr::term("b", 0)]),
                    );
                    let initial = Equation::new(
                        Expr::Term {
                            sequence: "b".into(),
                            index: NatExpr::constant(1),
                        },
                        rat(&base.pow(*e)),
                    );
                    auxiliaries.push(AuxiliarySequence {
                        name: "b".into(),
                        definition: definition.clone(),
                        initial,
                        recurrence: recurrence.clone(),
                    });
                    steps.push(step(
                        "指数が1なので、平行移動後の数列をそのまま使える。",
                        Some(definition),
                        "対数を使わない正規化",
                    ));
                    steps.push(step(
                        "平行移動後は等比数列となる。",
                        Some(recurrence),
                        "元の式への代入",
                    ));
                    steps.push(step(
                        "初項と公比から補助数列を求める。",
                        Some(Equation::new(
                            Expr::term("b", 0),
                            pow(base, logarithmic_exponent(*q, *d, *e)),
                        )),
                        "等比数列の一般項",
                    ));
                }
            } else if alternate {
                hint = "繰り返し代入して指数の等比級数を調べる。";
                method = "累乗を反復し指数の和を求める";
                steps.push(step(
                    "x_n=a_n−hとおく。x_1>0かつC>0なので帰納的にx_n>0。",
                    None,
                    "正値の帰納法",
                ));
                steps.push(step(&format!("x_(n+1)=C x_n^{q}を反復するとx_n=x_1^({q}^(n−1)) C^(1+{q}+⋯+{q}^(n−2))。n=1では空和0。"),None,"繰り返し代入と自然数乗の法則"));
                steps.push(step(&format!("C={base}^{d}、x_1={base}^{e}を代入し、有限等比級数を({q}^(n−1)−1)/({q}−1)でまとめる。"),None,"q≥2でq−1≠0"));
            } else {
                hint = "a_n−hの正値を確認して対数をとる。";
                method = if shift.is_zero() {
                    "対数で一次漸化式にする"
                } else {
                    "平行移動して対数をとる"
                };
                let definition = Equation::new(
                    Expr::term("b", 0),
                    Expr::Log {
                        base: Box::new(rat(base)),
                        argument: Box::new(Expr::sub(Expr::term("a", 0), rat(shift))),
                    },
                );
                let recurrence = Equation::new(
                    Expr::term("b", 1),
                    Expr::add(vec![
                        Expr::mul(vec![Expr::integer(*q as i64), Expr::term("b", 0)]),
                        Expr::integer(*d as i64),
                    ]),
                );
                let initial = Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    Expr::integer(*e as i64),
                );
                auxiliaries.push(AuxiliarySequence {
                    name: "b".into(),
                    definition: definition.clone(),
                    initial,
                    recurrence: recurrence.clone(),
                });
                steps.push(step(&format!("a_1−h={base}^{e}>0、C={base}^{d}>0より帰納的にa_n−h>0。底{base}は正かつ1でない。"),None,"実対数の定義域"));
                steps.push(step(
                    "正値を確認して対数の補助数列を定義する。",
                    Some(definition),
                    "底と真数の条件",
                ));
                steps.push(step(
                    "積と累乗の対数法則で一次漸化式となる。",
                    Some(recurrence),
                    "正の真数に対する対数法則",
                ));
                steps.push(step(&format!("不動点は−{d}/({q}−1)。これを引いて等比数列を解くとb_n={e}·{q}^(n−1)+{d}({q}^(n−1)−1)/({q}−1)。"),None,"q≥2の一次型"));
            }
        }
    }
    steps.push(step(
        "補助数列を戻し初期条件を満たす一般項を得る。",
        Some(Equation::new(Expr::term("a", 0), formula(route))),
        "逆変換と初期条件",
    ));
    Ok(DerivationIR {
        method_name: method.into(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    })
}
pub fn explain(p: &ProblemIR, c: &Classification) -> Result<Option<DerivationIR>, String> {
    let Some(own) = classify(p)? else {
        return Ok(None);
    };
    if own.method_name != c.method_name {
        return Ok(None);
    }
    let Some(r) = detect(p) else {
        return Ok(None);
    };
    let mut derivation = solution(p, &r, false)?;
    derivation.method_name = c.method_name.clone();
    Ok(Some(derivation))
}
pub fn alternative_derivations(p: &ProblemIR) -> Result<Vec<DerivationIR>, String> {
    let Some(r) = detect(p) else {
        return Ok(vec![]);
    };
    let useful = match &r {
        Route::Forcing { p, r, c, first } => {
            !p.is_zero()
                && !r.is_zero()
                && !c.is_zero()
                && !(p.is_one() && r.is_one())
                && !(p != r && *c == first.mul(&r.sub(p)))
        }
        Route::Logarithmic { q, d, e, .. } => *q >= 2 && (*d > 0 || *e > 0),
    };
    if useful {
        Ok(vec![solution(p, &r, true)?])
    } else {
        Ok(vec![])
    }
}
// Evaluate exponents in BigInt, never materializing double-exponential terms.
#[cfg(any(feature = "browser", test))]
fn nat_big(expr: &NatExpr, n: u32) -> Result<BigInt, String> {
    Ok(match expr {
        NatExpr::Index => n.into(),
        NatExpr::Constant { value } => (*value).into(),
        NatExpr::Add { left, right } => nat_big(left, n)? + nat_big(right, n)?,
        NatExpr::Sub { left, right } => {
            (nat_big(left, n)? - nat_big(right, n)?).max(BigInt::zero())
        }
        NatExpr::Mul { left, right } => nat_big(left, n)? * nat_big(right, n)?,
        NatExpr::Div {
            numerator,
            denominator,
        } => {
            let d = nat_big(denominator, n)?;
            if d.is_zero() {
                return Err("指数の分母が零です。".into());
            }
            nat_big(numerator, n)? / d
        }
        NatExpr::Pow { base, exponent } => {
            if *exponent > 32 {
                return Err("検査対象の指数が範囲外です。".into());
            }
            nat_big(base, n)?.pow(*exponent)
        }
        NatExpr::GeometricPower { base, exponent } => {
            let e = nat_big(exponent, n)?
                .to_u32()
                .filter(|&v| v <= 64)
                .ok_or("指数検査の上限を超えています。")?;
            BigInt::from(*base).pow(e)
        }
        _ => return Err("専用指数検査で扱わない自然数式です。".into()),
    })
}
#[cfg(any(feature = "browser", test))]
fn mod_pow(mut b: u64, mut e: u64, m: u64) -> u64 {
    let mut v = 1;
    while e > 0 {
        if e & 1 == 1 {
            v = v * b % m;
        }
        b = b * b % m;
        e >>= 1;
    }
    v
}
#[cfg(any(feature = "browser", test))]
fn rational_mod(v: &Rational, p: u64) -> Result<u64, String> {
    fn residue(n: &BigInt, p: u64) -> u64 {
        ((n % BigInt::from(p) + BigInt::from(p)) % BigInt::from(p))
            .to_u64()
            .unwrap()
    }
    let numerator = residue(v.0.numer(), p);
    let denominator = residue(v.0.denom(), p);
    if denominator == 0 {
        return Err("剰余検査の素数で分母が零です。".into());
    }
    Ok(numerator * mod_pow(denominator, p - 2, p) % p)
}
#[cfg(any(feature = "browser", test))]
fn exponent_mod(e: &BigInt, p: u64) -> u64 {
    (e % BigInt::from(p - 1)).to_u64().unwrap()
}
/// Exact recurrence checks for n=1..5; exponent identities and independent
/// recurrence iteration modulo three primes through n=20. Not a universal proof.
#[cfg(any(feature = "browser", test))]
pub fn finite_checks(problem: &ProblemIR) -> Result<Option<Value>, String> {
    let Some(Route::Logarithmic {
        base,
        q,
        d,
        e,
        shift,
    }) = detect(problem)
    else {
        return Ok(None);
    };
    if problem.general_term.lhs != Expr::term("a", 0) {
        return Err("一般項の左辺がa_nではありません。".into());
    }
    let route = Route::Logarithmic {
        base: base.clone(),
        q,
        d,
        e,
        shift: shift.clone(),
    };
    // Canonical answer reconstruction is required: agreeing finitely many terms
    // cannot approve an unrelated formula which diverges at a later index.
    if problem.general_term.rhs.canonical() != formula(&route).canonical() {
        return Err("原式と初期条件から復元した一般項に一致しません。".into());
    }
    if problem.conditions.len() != 1
        || problem.conditions[0].kind != "positive"
        || problem.conditions[0].expression.canonical()
            != Expr::sub(Expr::term("a", 0), rat(&shift)).canonical()
    {
        return Err("真数の正値条件が不足または変更されています。".into());
    }
    let term = |sequence: &str, n: u32| -> Result<Rational, String> {
        if sequence != "a" {
            return Err("未登録の数列です。".into());
        }
        problem
            .general_term
            .rhs
            .evaluate(n, &|_, _| Err("一般項は閉じた式が必要です。".into()))
    };
    if term("a", 1)? != first_value(problem).ok_or("初期条件がありません。")? {
        return Err("初期条件が一致しません。".into());
    }
    for n in 1..=5 {
        if problem.recurrence.lhs.evaluate(n, &term)?
            != problem.recurrence.rhs.evaluate(n, &term)?
        {
            return Err(format!("n={n}で漸化式が一致しません。"));
        }
        if !term("a", n)?.sub(&shift).is_positive() {
            return Err("真数が正ではありません。".into());
        }
    }
    let exponent = if q == 0 {
        None
    } else {
        Some(logarithmic_exponent(q, d, e))
    };
    let primes = [1_000_003u64, 1_000_033, 1_000_037];
    for p in primes {
        let b = rational_mod(&base, p)?;
        let c = mod_pow(b, d as u64, p);
        let h = rational_mod(&shift, p)?;
        let mut current = (h + mod_pow(b, e as u64, p)) % p;
        for n in 1..=20 {
            let this_exp = match &exponent {
                Some(x) => nat_big(x, n)?,
                None => BigInt::from(if n == 1 { e } else { d }),
            };
            let next_exp = match &exponent {
                Some(x) => nat_big(x, n + 1)?,
                None => BigInt::from(d),
            };
            if next_exp != BigInt::from(q) * &this_exp + BigInt::from(d) {
                return Err("指数の漸化式が一致しません。".into());
            }
            let direct = (h + mod_pow(b, exponent_mod(&this_exp, p), p)) % p;
            if current != direct {
                return Err("原式を反復した剰余と一般項の剰余が一致しません。".into());
            }
            let x = (current + p - h) % p;
            current = (h + c * mod_pow(x, q as u64, p)) % p;
            let next = (h + mod_pow(b, exponent_mod(&next_exp, p), p)) % p;
            if current != next {
                return Err("一般項の次項の剰余が一致しません。".into());
            }
        }
    }
    Ok(Some(
        json!({"status":"finite_checks_passed","formal_verification":false,"method":"bounded_exact_exponent_and_modular_checks","checked_through_n":20,"exact_recurrence_through_n":5,"exact_terms_through_index":6,"exponent_identity_through_n":20,"modular_recurrence_through_n":20,"modular_primes":primes,"formula_reconstructed_from_statement":true,"message":"n=1..5の漸化式を厳密照合。n=1..20の指数恒等式と3素数での元の漸化式を検査。全添字の形式証明ではありません。"}),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn forcing(p: i64, r: i64, c: i64, a: i64) -> ProblemIR {
        compile(
            &build_recipe(
                "cases_exponential_forcing",
                0,
                params(&[
                    ("p", Rational::integer(p)),
                    ("r", Rational::integer(r)),
                    ("c", Rational::integer(c)),
                    ("a", Rational::integer(a)),
                ]),
            )
            .unwrap(),
        )
        .unwrap()
    }
    fn logarithmic(q: u32, d: u32, e: u32, h: i64, b: Rational) -> ProblemIR {
        compile(
            &build_recipe(
                if h == 0 {
                    "cases_logarithmic"
                } else {
                    "cases_shifted_logarithmic"
                },
                0,
                params(&[
                    ("base", b),
                    ("q", Rational::integer(q as i64)),
                    ("d", Rational::integer(d as i64)),
                    ("e", Rational::integer(e as i64)),
                    ("h", Rational::integer(h)),
                ]),
            )
            .unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn signed_zero_and_repeated_multipliers_satisfy_recurrence() {
        for p in -3..=3 {
            for r in -3..=3 {
                for c in [-2, 0, 2] {
                    let problem = forcing(p, r, c, 3);
                    let term = |_: &str, n: u32| {
                        problem
                            .general_term
                            .rhs
                            .evaluate(n, &|_, _| Err("closed".into()))
                    };
                    assert_eq!(term("a", 1).unwrap(), Rational::integer(3));
                    for n in 1..=20 {
                        assert_eq!(
                            problem.recurrence.lhs.evaluate(n, &term).unwrap(),
                            problem.recurrence.rhs.evaluate(n, &term).unwrap(),
                            "p={p},r={r},c={c},n={n}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn huge_log_exponents_are_checked_without_materializing_terms() {
        for q in 0..=3 {
            for d in 0..=2 {
                for e in 0..=2 {
                    for h in [-2, 0, 2] {
                        for b in [
                            Rational::integer(2),
                            Rational::integer(3),
                            Rational::fraction(1, 3),
                        ] {
                            let p = logarithmic(q, d, e, h, b);
                            let check = finite_checks(&p).unwrap().unwrap();
                            assert_eq!(check["formal_verification"], false);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn forward_detection_ignores_answer_and_tampering_is_rejected() {
        let mut p = logarithmic(3, 1, 2, -2, Rational::integer(3));
        let c = classify(&p).unwrap().unwrap();
        p.general_term.rhs = Expr::integer(999);
        assert_eq!(classify(&p).unwrap().unwrap().method_name, c.method_name);
        assert!(finite_checks(&p).is_err());
        let mut p = logarithmic(2, 1, 1, 1, Rational::integer(2));
        p.conditions.clear();
        assert!(finite_checks(&p).is_err());
    }
    #[test]
    fn exceptional_coefficients_use_easier_methods() {
        assert_eq!(classify(&forcing(1, 1, 2, 3)).unwrap().unwrap().level, 1);
        assert_eq!(classify(&forcing(0, 1, 2, 3)).unwrap().unwrap().level, 1);
        assert_eq!(classify(&forcing(0, 2, 2, 3)).unwrap().unwrap().level, 2);
        assert_eq!(classify(&forcing(2, 2, 2, 3)).unwrap().unwrap().level, 3);
        assert_eq!(
            classify(&logarithmic(1, 1, 1, 0, Rational::integer(2)))
                .unwrap()
                .unwrap()
                .level,
            1
        );
        assert_eq!(
            classify(&logarithmic(1, 1, 1, 2, Rational::integer(2)))
                .unwrap()
                .unwrap()
                .level,
            2
        );
    }
    #[test]
    fn variants_replay_and_have_the_advertised_level() {
        use rand::SeedableRng;
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for f in catalog() {
            for v in variants(&f.id) {
                for seed in 0..20 {
                    let recipe =
                        build_recipe(&f.id, seed, sample_variant(&f.id, v.id, &mut rng).unwrap())
                            .unwrap();
                    assert_eq!(variant_id(&recipe).as_deref(), Some(v.id));
                    let p = compile(&recipe).unwrap();
                    assert_eq!(
                        classify(&p).unwrap().unwrap().level,
                        v.level,
                        "{} {}",
                        f.id,
                        v.id
                    );
                }
            }
        }
    }
    #[test]
    fn unsupported_parameters_and_domains_are_rejected() {
        let mut p = logarithmic(2, 1, 1, 0, Rational::integer(2));
        p.initials[0].rhs = Expr::integer(-2);
        assert!(detect(&p).is_none());
        let p = params(&[
            ("base", Rational::integer(2)),
            ("q", Rational::integer(-1)),
            ("d", Rational::integer(1)),
            ("e", Rational::integer(1)),
            ("h", Rational::integer(0)),
        ]);
        assert!(build_recipe("cases_logarithmic", 0, p).is_err());
    }
}
