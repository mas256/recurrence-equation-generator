//! Natural exponent profiles and forward recognition of multiplicative relations.
use crate::generator::{
    AuxiliarySequence, DerivationIR, DerivationStep, DomainCondition, FamilyInfo, ProblemIR,
    Recipe, RecipeBlock, RecipeCore, RecurrenceKind,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet};

type Params = BTreeMap<String, Rational>;
fn map(items: &[(&str, Rational)]) -> Params {
    items
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}
fn nat_add(left: NatExpr, right: NatExpr) -> NatExpr {
    NatExpr::Add {
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn nat_mul(left: NatExpr, right: NatExpr) -> NatExpr {
    NatExpr::Mul {
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn choose(index: NatExpr, count: u32) -> NatExpr {
    NatExpr::choose(index, count)
}
fn rat(value: &Rational) -> Expr {
    Expr::rational(value.clone())
}
fn block(id: &str, input: &str, kind: &str, parameters: Params) -> RecipeBlock {
    RecipeBlock {
        id: id.into(),
        input: input.into(),
        kind: kind.into(),
        parameters,
    }
}
pub fn supports(family: &str) -> bool {
    matches!(family, "ratio_power" | "multiplicative_second")
}
pub fn catalog() -> Vec<FamilyInfo> {
    vec![
        FamilyInfo {
            id: "ratio_power".into(),
            label: "指数係数の階比型".into(),
            levels: vec![3, 4],
        },
        FamilyInfo {
            id: "multiplicative_second".into(),
            label: "対数＋二重の階差".into(),
            levels: vec![5],
        },
    ]
}
pub fn sample(family: &str, level: u8, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    if !supports(family) {
        return Err("未登録の指数型です。".into());
    }
    let mut p = map(&[
        ("c", Rational::integer(rng.gen_range(1..=3))),
        ("r", Rational::integer(rng.gen_range(2..=3))),
        ("profile", Rational::integer(rng.gen_range(0..=2))),
        ("profile_version", Rational::integer(1)),
    ]);
    if family == "multiplicative_second" {
        p.insert("d".into(), Rational::integer(rng.gen_range(1..=3)));
        p.insert("e".into(), Rational::integer(rng.gen_range(0..=2)));
    } else {
        p.insert(
            "c".into(),
            Rational::fraction(rng.gen_range(1..=9), [1, 2, 3, 5][rng.gen_range(0..4)]),
        );
    }
    if family == "ratio_power" {
        p.insert(
            "profile".into(),
            Rational::integer(if level == 4 { 2 } else { rng.gen_range(0..=1) }),
        );
    }
    Ok(p)
}
pub fn profile(p: &Params) -> Result<u32, String> {
    if p.get("profile_version") != Some(&Rational::integer(1)) {
        return Err("指数プロファイルの版が一致しません。".into());
    }
    p.get("profile")
        .and_then(Rational::integer_u32)
        .filter(|value| *value <= 2)
        .ok_or("指数プロファイルは0から2の整数です。".into())
}
pub fn validate_parameters(family: &str, p: &Params) -> Result<(), String> {
    let names: &[&str] = match family {
        "ratio_power" => &["c", "r", "profile", "profile_version"],
        "multiplicative_second" => &["c", "d", "e", "r", "profile", "profile_version"],
        _ => return Err("未登録の指数型です。".into()),
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.iter().copied().collect() {
        return Err("指数型Recipeのパラメータ集合が規則と一致しません。".into());
    }
    profile(p)?;
    if !p["r"].integer_u32().is_some_and(|r| (2..=3).contains(&r)) {
        return Err("対数の底は2または3です。".into());
    }
    if family == "ratio_power" {
        if !p["c"].is_positive() || p["c"].0 > Rational::integer(16).0 {
            return Err("初項は正の有理数で16以下です。".into());
        }
    } else {
        for name in ["c", "d"] {
            if !p[name]
                .integer_u32()
                .is_some_and(|value| (1..=3).contains(&value))
            {
                return Err("指数の増分は1から3の自然数です。".into());
            }
        }
        if !p["e"].integer_u32().is_some_and(|value| value <= 2) {
            return Err("初項の指数は0から2の自然数です。".into());
        }
    }
    Ok(())
}
pub fn build_recipe(family: &str, seed: u64, p: Params) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    let profile = profile(&p)?;
    let core = RecipeCore {
        id: "core".into(),
        kind: "constant".into(),
        parameters: map(&[("initial", p["c"].clone())]),
    };
    let mut blocks = Vec::new();
    if family == "ratio_power" {
        blocks.push(block(
            "b1",
            "core",
            "index_scale",
            map(&[
                ("base", p["r"].clone()),
                ("profile", p["profile"].clone()),
                ("profile_version", Rational::integer(1)),
            ]),
        ));
    } else {
        if profile != 0 {
            blocks.push(block(
                "b1",
                "core",
                "index_scale",
                map(&[
                    ("profile", p["profile"].clone()),
                    ("profile_version", Rational::integer(1)),
                ]),
            ));
        }
        for (kind, name) in [
            ("cumulative_sum", "d"),
            ("cumulative_sum", "e"),
            ("power_sequence", "r"),
        ] {
            let id = format!("b{}", blocks.len() + 1);
            let input = blocks
                .last()
                .map(|b: &RecipeBlock| b.id.as_str())
                .unwrap_or("core");
            let parameter = if kind == "power_sequence" {
                "base"
            } else {
                "initial"
            };
            blocks.push(block(
                &id,
                input,
                kind,
                map(&[(parameter, p[name].clone())]),
            ));
        }
    }
    let output = blocks.last().unwrap().id.clone();
    Ok(Recipe {
        schema_version: crate::generator::SCHEMA_VERSION.into(),
        rules_version: crate::generator::EXTENSION_RULES_VERSION.into(),
        rng_version: crate::generator::RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p,
        core,
        blocks,
        output,
        index_start: 1,
    })
}
pub fn ratio_exponents(profile: u32) -> (NatExpr, NatExpr) {
    match profile {
        0 => (choose(NatExpr::n(), 2), NatExpr::n()),
        1 => (
            nat_mul(NatExpr::offset(-1), NatExpr::offset(1)),
            nat_add(
                nat_mul(NatExpr::constant(2), NatExpr::n()),
                NatExpr::constant(1),
            ),
        ),
        2 => (choose(NatExpr::offset(1), 3), choose(NatExpr::offset(1), 2)),
        _ => unreachable!(),
    }
}
pub fn second_exponents(profile: u32, e: u32, d: u32, c: u32) -> (NatExpr, NatExpr) {
    let k = NatExpr::offset(-1);
    let (sum, forcing) = match profile {
        0 => (choose(k.clone(), 2), NatExpr::constant(1)),
        1 => (choose(NatExpr::n(), 3), NatExpr::n()),
        2 => (
            nat_add(
                nat_mul(NatExpr::constant(2), choose(NatExpr::n(), 3)),
                choose(k.clone(), 2),
            ),
            nat_add(
                nat_mul(NatExpr::constant(2), NatExpr::n()),
                NatExpr::constant(1),
            ),
        ),
        _ => unreachable!(),
    };
    (
        nat_add(
            nat_add(NatExpr::constant(e), nat_mul(NatExpr::constant(d), k)),
            nat_mul(NatExpr::constant(c), sum),
        ),
        nat_mul(NatExpr::constant(c), forcing),
    )
}
fn initial(index: u32, value: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: "a".into(),
            index: NatExpr::constant(index),
        },
        rat(&value),
    )
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    validate_parameters(&recipe.family, &recipe.parameters)?;
    let p = &recipe.parameters;
    let profile = profile(p)?;
    let r = rat(&p["r"]);
    let a = Expr::term("a", 0);
    let (kind, recurrence, formula) = if recipe.family == "ratio_power" {
        let (total, increment) = ratio_exponents(profile);
        (
            RecurrenceKind::FirstOrder,
            Equation::new(
                Expr::term("a", 1),
                Expr::mul(vec![Expr::pow(r.clone(), increment), a.clone()]),
            ),
            Expr::mul(vec![rat(&p["c"]), Expr::pow(r.clone(), total)]),
        )
    } else {
        let (total, increment) = second_exponents(
            profile,
            p["e"].integer_u32().unwrap(),
            p["d"].integer_u32().unwrap(),
            p["c"].integer_u32().unwrap(),
        );
        (
            RecurrenceKind::MultiplicativeSecond,
            Equation::new(
                Expr::mul(vec![Expr::term("a", 2), a.clone()]),
                Expr::mul(vec![
                    Expr::pow(r.clone(), increment),
                    Expr::pow(Expr::term("a", 1), NatExpr::constant(2)),
                ]),
            ),
            Expr::pow(r.clone(), total),
        )
    };
    let count = if recipe.family == "ratio_power" { 1 } else { 2 };
    let initials = (1..=count)
        .map(|index| {
            Ok(initial(
                index,
                formula.evaluate(index, &|_, _| Err("一般項に数列参照があります。".into()))?,
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let conditions = vec![DomainCondition {
        kind: "positive".into(),
        expression: a,
        explanation: "初期値と指数係数が正であり、漸化式によりすべての項が正になる。".into(),
    }];
    Ok(ProblemIR {
        kind,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence,
        initials,
        general_term: Equation::new(Expr::term("a", 0), formula),
        conditions,
        secondary_recurrences: vec![],
        secondary_general_terms: vec![],
    })
}

#[derive(Clone, Debug)]
enum Route {
    Ratio {
        base: Rational,
        first: Rational,
        profile: u32,
    },
    Second {
        base: Rational,
        profile: u32,
        e: u32,
        d: u32,
        c: u32,
    },
}
fn literal(expr: &Expr) -> Option<Rational> {
    if let Expr::Rational { value } = expr {
        Some(value.clone())
    } else {
        None
    }
}
fn integer_log(value: &Rational, base: &Rational) -> Option<u32> {
    if !value.is_positive() || base.0 <= Rational::integer(1).0 {
        return None;
    }
    let mut value = value.clone();
    for exponent in 0..=32 {
        if value.is_one() {
            return Some(exponent);
        }
        if value.0 < Rational::integer(1).0 {
            return None;
        }
        value = value.div(base).ok()?;
    }
    None
}
fn detect(problem: &ProblemIR) -> Option<Route> {
    if problem.kind == RecurrenceKind::FirstOrder && problem.recurrence.lhs == Expr::term("a", 1) {
        let Expr::Mul { factors } = &problem.recurrence.rhs else {
            return None;
        };
        if factors.len() != 2 || !factors.contains(&Expr::term("a", 0)) {
            return None;
        }
        let power = factors.iter().find(|x| **x != Expr::term("a", 0))?;
        let Expr::Pow { base, exponent } = power else {
            return None;
        };
        let base = literal(base)?;
        if !base
            .integer_u32()
            .is_some_and(|value| (2..=3).contains(&value))
        {
            return None;
        }
        let first = literal(&problem.initials.first()?.rhs)?;
        if !first.is_positive() {
            return None;
        }
        for profile in 0..=2 {
            if *exponent == ratio_exponents(profile).1 {
                return Some(Route::Ratio {
                    base,
                    first,
                    profile,
                });
            }
        }
    }
    if problem.kind != RecurrenceKind::MultiplicativeSecond {
        return None;
    }
    if problem.recurrence.lhs.canonical()
        != Expr::mul(vec![Expr::term("a", 2), Expr::term("a", 0)]).canonical()
    {
        return None;
    }
    let Expr::Mul { factors } = &problem.recurrence.rhs else {
        return None;
    };
    let square = Expr::pow(Expr::term("a", 1), NatExpr::constant(2));
    if factors.len() != 2 || !factors.contains(&square) {
        return None;
    }
    let Expr::Pow { base, exponent } = factors.iter().find(|x| **x != square)? else {
        return None;
    };
    let base = literal(base)?;
    if !base
        .integer_u32()
        .is_some_and(|value| (2..=3).contains(&value))
    {
        return None;
    }
    let e = integer_log(&literal(&problem.initials.first()?.rhs)?, &base)?;
    let second = integer_log(&literal(&problem.initials.get(1)?.rhs)?, &base)?;
    let d = second.checked_sub(e)?;
    if e > 2 || !(1..=3).contains(&d) {
        return None;
    }
    for profile in 0..=2 {
        for c in 1..=3 {
            if *exponent == second_exponents(profile, e, d, c).1 {
                return Some(Route::Second {
                    base,
                    profile,
                    e,
                    d,
                    c,
                });
            }
        }
    }
    None
}
pub fn classify(problem: &ProblemIR) -> Result<Option<crate::generator::Classification>, String> {
    let Some(route) = detect(problem) else {
        return Ok(None);
    };
    let intermediate = match &route {
        Route::Ratio { base, first, .. } => vec![base.clone(), first.clone()],
        Route::Second {
            base,
            e,
            d,
            c,
            profile,
        } => {
            let mut values = vec![
                base.clone(),
                Rational::integer(*e as i64),
                Rational::integer((e + d) as i64),
                Rational::integer(*d as i64),
                Rational::integer(*c as i64),
            ];
            if *profile == 2 {
                values.push(Rational::integer((2 * c) as i64));
            }
            values
        }
    };
    let result = match route {
        Route::Ratio { profile, .. } => crate::generator::make_classification(
            problem,
            if profile == 2 { 14 } else { 9 },
            "比を掛けて指数を足す",
            "指数係数の階比で指数の和を計算する",
            &[
                "ratio_product",
                "evaluate_product",
                if profile == 2 {
                    "evaluate_quadratic_sum"
                } else {
                    "evaluate_sum"
                },
            ],
            1,
            0,
            0,
        )?,
        Route::Second { profile, .. } => crate::generator::make_classification(
            problem,
            20,
            "対数をとり、階差を2回足し合わせる",
            "対数と二重の階差を組み合わせる",
            &[
                "logarithm",
                "difference",
                "difference_sum",
                "evaluate_sum",
                "difference_sum",
                if profile == 0 {
                    "evaluate_sum"
                } else {
                    "evaluate_quadratic_sum"
                },
            ],
            2,
            1,
            0,
        )?,
    };
    Ok(Some(crate::generator::include_intermediate_values(
        problem,
        result,
        &intermediate,
    )?))
}
fn ds(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn sequence_initial(sequence: &str, value: u32) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: sequence.into(),
            index: NatExpr::constant(1),
        },
        Expr::integer(value as i64),
    )
}
fn nat_expression(value: NatExpr) -> Expr {
    Expr::NatCast { value }
}
pub fn explain(
    problem: &ProblemIR,
    classification: &crate::generator::Classification,
) -> Result<Option<DerivationIR>, String> {
    if !matches!(
        classification.method_name.as_str(),
        "比を掛けて指数を足す" | "対数をとり、階差を2回足し合わせる"
    ) {
        return Ok(None);
    }
    let route = detect(problem).ok_or("指数型の前向き解法を再現できません。")?;
    let mut auxiliaries = Vec::new();
    let (hint, steps) = match route {
        Route::Ratio {
            base,
            first,
            profile,
        } => {
            let (total, increment) = ratio_exponents(profile);
            let normalizer = Expr::pow(rat(&base), total.clone());
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: Equation::new(
                    Expr::term("b", 0),
                    Expr::div(Expr::term("a", 0), normalizer.clone()),
                ),
                initial: Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(&first),
                ),
                recurrence: Equation::new(Expr::term("b", 1), Expr::term("b", 0)),
            });
            let j_increment = replace_nat_index(&increment, &NatExpr::Bound { name: "j".into() });
            let sum = Expr::Sum {
                variable: "j".into(),
                lower: NatExpr::constant(1),
                upper: NatExpr::offset(-1),
                body: Box::new(nat_expression(j_increment)),
            };
            ("倍率を掛け合わせると、同じ底の指数は加算される。指数を1からn−1まで足す。", vec![
                ds("初項と倍率が正なので、すべての項は正である。", None, "正の数の積は正"),
                ds("倍率を初項から順に掛け合わせる。同じ底の積は、指数を足すことでまとめられる。", Some(Equation::new(Expr::term("a", 0), Expr::mul(vec![rat(&first), Expr::pow(rat(&base), total.clone())]))), "指数法則と指数の有限和"),
                ds("倍率の指数の和を計算する。", Some(Equation::new(sum, nat_expression(total))), "等差数列の和、または平方数の和から指数を計算する"),
                ds("この指数を代入すると一般項が得られる。正規化した補助数列b_nは一定になる。", Some(Equation::new(Expr::term("a", 0), Expr::mul(vec![rat(&first), normalizer]))), "有限積の指数を整理する"),
            ])
        }
        Route::Second {
            base,
            profile,
            e,
            d,
            c,
        } => {
            let (total, forcing) = second_exponents(profile, e, d, c);
            let b = Expr::term("b", 0);
            let difference = match profile {
                0 => nat_add(
                    NatExpr::constant(d),
                    nat_mul(NatExpr::constant(c), NatExpr::offset(-1)),
                ),
                1 => nat_add(
                    NatExpr::constant(d),
                    nat_mul(NatExpr::constant(c), choose(NatExpr::n(), 2)),
                ),
                _ => nat_add(
                    NatExpr::constant(d),
                    nat_mul(
                        NatExpr::constant(c),
                        nat_add(
                            nat_mul(NatExpr::constant(2), choose(NatExpr::n(), 2)),
                            NatExpr::offset(-1),
                        ),
                    ),
                ),
            };
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: Equation::new(
                    b.clone(),
                    Expr::Log {
                        base: Box::new(rat(&base)),
                        argument: Box::new(Expr::term("a", 0)),
                    },
                ),
                initial: sequence_initial("b", e),
                recurrence: Equation::new(
                    Expr::sub(
                        Expr::add(vec![Expr::term("b", 2), b.clone()]),
                        Expr::mul(vec![Expr::integer(2), Expr::term("b", 1)]),
                    ),
                    nat_expression(forcing.clone()),
                ),
            });
            auxiliaries.push(AuxiliarySequence {
                name: "c".into(),
                definition: Equation::new(
                    Expr::term("c", 0),
                    Expr::sub(Expr::term("b", 1), b.clone()),
                ),
                initial: sequence_initial("c", d),
                recurrence: Equation::new(
                    Expr::sub(Expr::term("c", 1), Expr::term("c", 0)),
                    nat_expression(forcing.clone()),
                ),
            });
            let definition = auxiliaries[0].definition.clone();
            let recurrence = auxiliaries[0].recurrence.clone();
            let second_initial = Equation::new(
                Expr::Term {
                    sequence: "b".into(),
                    index: NatExpr::constant(2),
                },
                Expr::integer((e + d) as i64),
            );
            ("各項の次数がそろっている。底をそろえて対数をとり、3項間を階差に直す。", vec![
                ds("初期値と指数係数は正である。漸化式により次項も正となるため、すべての項の対数をとれる。", Some(definition), "正の初期値から帰納的に正値性を示す"),
                ds("対数をとると、積は和、2乗は2倍になる。", Some(recurrence), "実数の対数の積と累乗の法則"),
                ds("初期値の対数も計算する。", Some(second_initial), "初期値は底の自然数乗"),
                ds("c_n=b_{n+1}−b_nとおくと、c_{n+1}−c_nが既知になる。これを初期差分から足し合わせる。", Some(Equation::new(Expr::term("c", 0), nat_expression(difference))), "1回目の階差の和"),
                ds("もう一度差を初項から足し合わせると、指数b_nが得られる。", Some(Equation::new(b, nat_expression(total.clone()))), "2回目の階差の和"),
                ds("a_nは底をb_n乗したものなので、一般項は次のとおりである。", Some(Equation::new(Expr::term("a", 0), Expr::pow(rat(&base), total))), "対数の定義から元の数列に戻す"),
            ])
        }
    };
    Ok(Some(DerivationIR {
        method_name: classification.method_name.clone(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    }))
}
fn replace_nat_index(value: &NatExpr, replacement: &NatExpr) -> NatExpr {
    match value {
        NatExpr::Index => replacement.clone(),
        NatExpr::Add { left, right } => nat_add(
            replace_nat_index(left, replacement),
            replace_nat_index(right, replacement),
        ),
        NatExpr::Sub { left, right } => NatExpr::Sub {
            left: Box::new(replace_nat_index(left, replacement)),
            right: Box::new(replace_nat_index(right, replacement)),
        },
        NatExpr::Mul { left, right } => nat_mul(
            replace_nat_index(left, replacement),
            replace_nat_index(right, replacement),
        ),
        NatExpr::Choose { n, k } => choose(replace_nat_index(n, replacement), *k),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe(family: &str, profile: u32) -> Recipe {
        let mut p = map(&[
            ("c", Rational::integer(1)),
            ("r", Rational::integer(2)),
            ("profile", Rational::integer(profile as i64)),
            ("profile_version", Rational::integer(1)),
        ]);
        if family == "multiplicative_second" {
            p.insert("c".into(), Rational::integer(3));
            p.insert("d".into(), Rational::integer(2));
            p.insert("e".into(), Rational::integer(1));
        }
        build_recipe(family, profile as u64, p).unwrap()
    }
    #[test]
    fn all_profiles_have_exact_relations_and_forward_derivations() {
        for family in ["ratio_power", "multiplicative_second"] {
            for profile in 0..=2 {
                let recipe = recipe(family, profile);
                let problem = crate::generator::replay(&recipe).unwrap();
                let ir = &problem.problem;
                for n in 1..=8 {
                    let term = |sequence: &str, index| {
                        assert_eq!(sequence, "a");
                        ir.general_term
                            .rhs
                            .evaluate(index, &|_, _| Err("unexpected term".into()))
                    };
                    assert_eq!(
                        ir.recurrence.lhs.evaluate(n, &term).unwrap(),
                        ir.recurrence.rhs.evaluate(n, &term).unwrap(),
                        "{family}/{profile}, n={n}"
                    );
                }
                let mut with_wrong_answer = ir.clone();
                with_wrong_answer.general_term.rhs = Expr::integer(1);
                let forward = classify(&with_wrong_answer).unwrap().unwrap();
                assert_eq!(forward.method_name, problem.classification.method_name);
                let derivation = explain(&with_wrong_answer, &forward).unwrap().unwrap();
                assert_eq!(
                    derivation
                        .steps
                        .last()
                        .unwrap()
                        .equation
                        .as_ref()
                        .unwrap()
                        .rhs,
                    ir.general_term.rhs
                );
            }
        }
    }
    #[test]
    #[ignore = "requires the prepared Lean 4.19 environment"]
    fn all_power_profiles_are_kernel_checked() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut problems = Vec::new();
        for family in ["ratio_power", "multiplicative_second"] {
            for profile in 0..=2 {
                let base_recipe = recipe(family, profile);
                let amplitudes = if family == "ratio_power" {
                    vec![
                        Rational::integer(1),
                        Rational::fraction(3, 2),
                        Rational::fraction(8, 5),
                    ]
                } else {
                    vec![base_recipe.parameters["c"].clone()]
                };
                for amplitude in amplitudes {
                    let mut parameters = base_recipe.parameters.clone();
                    parameters.insert("c".into(), amplitude);
                    let recipe = build_recipe(family, profile as u64, parameters).unwrap();
                    problems.push(crate::generator::replay(&recipe).unwrap());
                }
            }
        }
        for seed in [11, 91] {
            for level in [3, 4] {
                problems
                    .push(crate::generator::generate(level, Some("ratio_power"), seed).unwrap());
            }
        }
        for (case, problem) in problems.into_iter().enumerate() {
            let proof = crate::proofs::powers::source(&problem).unwrap().unwrap();
            let path = std::env::temp_dir().join(format!(
                "recurrence-power-{}-{case}.lean",
                std::process::id()
            ));
            std::fs::write(&path, proof).unwrap();
            let output = std::process::Command::new("lake")
                .args(["env", "lean"])
                .arg(&path)
                .current_dir(root.join("lean"))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{} case{case}: {}",
                problem.recipe.family,
                String::from_utf8_lossy(&output.stdout)
            );
            std::fs::remove_file(path).unwrap();
        }
    }
}
