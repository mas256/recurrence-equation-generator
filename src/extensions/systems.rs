//! Two rational sequences, separated by an exact, forward-detected linear combination.
use crate::generator::{
    AuxiliarySequence, Classification, DerivationIR, DerivationStep, DomainCondition, FamilyInfo,
    ProblemIR, Recipe, RecipeBlock, RecipeCore, RecurrenceKind, EXTENSION_RULES_VERSION,
    RNG_VERSION, SCHEMA_VERSION,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet};

type Params = BTreeMap<String, Rational>;
type Poly = BTreeMap<u32, Rational>;

pub fn supports(family: &str) -> bool {
    matches!(
        family,
        "coupled_symmetric" | "coupled_weighted" | "coupled_scaled" | "coupled_forced"
    )
}
pub fn catalog() -> Vec<FamilyInfo> {
    [
        ("coupled_symmetric", "対称な連立", vec![3]),
        ("coupled_weighted", "一次結合で分離する連立", vec![3]),
        ("coupled_scaled", "正規化する連立", vec![4]),
        ("coupled_forced", "付加項のある連立", vec![3, 5]),
    ]
    .into_iter()
    .map(|(id, label, levels)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels,
    })
    .collect()
}
fn params(items: &[(&str, Rational)]) -> Params {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
pub fn sample(family: &str, level: u8, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    if !supports(family) {
        return Err("未登録の連立型です。".into());
    }
    let r = rng.gen_range(2..=5);
    let mut s = rng.gen_range(2..=5);
    while s == r {
        s = rng.gen_range(2..=5);
    }
    let c = if family == "coupled_weighted" {
        rng.gen_range(2..=3)
    } else {
        rng.gen_range(1..=6)
    };
    let profile = match family {
        "coupled_scaled" => rng.gen_range(0..=2),
        "coupled_forced" => i64::from(level == 5),
        _ => 0,
    };
    Ok(params(&[
        ("r", Rational::integer(r)),
        ("s", Rational::integer(s)),
        ("c", Rational::integer(c)),
        (
            "d",
            Rational::fraction(rng.gen_range(1..=9), rng.gen_range(1..=3)),
        ),
        ("profile", Rational::integer(profile)),
    ]))
}
pub fn validate_parameters(family: &str, p: &Params) -> Result<(), String> {
    if !supports(family) {
        return Err("未登録の連立型です。".into());
    }
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>()
        != ["r", "s", "c", "d", "profile"].into_iter().collect()
    {
        return Err("連立型のパラメータ集合が登録規則と一致しません。".into());
    }
    for k in ["r", "s"] {
        if !p[k].integer_u32().is_some_and(|x| (2..=5).contains(&x)) {
            return Err("連立の固有値は2から5の整数です。".into());
        }
    }
    if p["r"] == p["s"] {
        return Err("連立の2つの固有値は異なる必要があります。".into());
    }
    for k in ["c", "d"] {
        if !p[k].is_positive() || p[k].0 > Rational::integer(16).0 {
            return Err("連立の振幅は正の有理数で16以下です。".into());
        }
    }
    if family == "coupled_weighted" && !p["c"].integer_u32().is_some_and(|x| (2..=3).contains(&x)) {
        return Err("一次結合の重みは2または3です。".into());
    }
    let max_profile = match family {
        "coupled_scaled" => 2,
        "coupled_forced" => 1,
        _ => 0,
    };
    if !p["profile"].integer_u32().is_some_and(|x| x <= max_profile) {
        return Err("連立型のプロファイルが登録範囲外です。".into());
    }
    Ok(())
}
pub fn build_recipe(family: &str, seed: u64, p: Params) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    let mut blocks = Vec::new();
    if family == "coupled_forced" {
        blocks.push(RecipeBlock {
            id: "b1".into(),
            input: "core".into(),
            kind: "index_add".into(),
            parameters: params(&[
                ("amplitude", p["c"].clone()),
                ("profile", p["profile"].clone()),
            ]),
        });
    }
    let id = format!("b{}", blocks.len() + 1);
    let input = blocks
        .last()
        .map(|x: &RecipeBlock| x.id.clone())
        .unwrap_or_else(|| "core".into());
    blocks.push(RecipeBlock {
        id,
        input,
        kind: "pair_mix".into(),
        parameters: params(&[
            ("ratio", p["s"].clone()),
            ("amplitude", p["c"].clone()),
            (
                "weight",
                if family == "coupled_weighted" {
                    p["c"].clone()
                } else {
                    Rational::integer(1)
                },
            ),
        ]),
    });
    if family == "coupled_scaled" {
        let input = blocks.last().unwrap().id.clone();
        blocks.push(RecipeBlock {
            id: format!("b{}", blocks.len() + 1),
            input,
            kind: "index_scale".into(),
            parameters: params(&[("profile", p["profile"].clone())]),
        });
    }
    Ok(Recipe {
        schema_version: SCHEMA_VERSION.into(),
        rules_version: EXTENSION_RULES_VERSION.into(),
        rng_version: RNG_VERSION.into(),
        family: family.into(),
        seed,
        core: RecipeCore {
            id: "core".into(),
            kind: "geometric".into(),
            parameters: params(&[("ratio", p["r"].clone()), ("amplitude", p["d"].clone())]),
        },
        output: blocks.last().unwrap().id.clone(),
        blocks,
        parameters: p,
        index_start: 1,
    })
}
fn scale(profile: u32, next: bool) -> Expr {
    let n = Expr::offset(i32::from(next));
    match profile {
        0 => n,
        1 => Expr::add(vec![n, Expr::integer(1)]),
        _ => Expr::add(vec![Expr::mul(vec![Expr::integer(2), n]), Expr::integer(1)]),
    }
}
fn initial(sequence: &str, value: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: sequence.into(),
            index: NatExpr::constant(1),
        },
        Expr::rational(value),
    )
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    let p = &recipe.parameters;
    validate_parameters(&recipe.family, p)?;
    let r = &p["r"];
    let s = &p["s"];
    let c = &p["c"];
    let d = &p["d"];
    let t = if recipe.family == "coupled_weighted" {
        c.clone()
    } else {
        Rational::integer(1)
    };
    let profile = p["profile"].integer_u32().unwrap();
    let scaled = recipe.family == "coupled_scaled";
    let g = if scaled {
        scale(profile, false)
    } else {
        Expr::integer(1)
    };
    let next = if scaled {
        scale(profile, true)
    } else {
        Expr::integer(1)
    };
    let h = if recipe.family != "coupled_forced" {
        Expr::integer(0)
    } else if profile == 0 {
        Expr::rational(c.clone())
    } else {
        Expr::add(vec![
            Expr::mul(vec![
                Expr::rational(Rational::integer(-1).mul(c)),
                Expr::index(),
            ]),
            Expr::rational(
                Rational::integer(-1)
                    .mul(c)
                    .div(&r.sub(&Rational::integer(1)))?,
            ),
        ])
    };
    let forcing = if recipe.family != "coupled_forced" {
        Expr::integer(0)
    } else if profile == 0 {
        Expr::rational(Rational::integer(1).sub(r).mul(c))
    } else {
        Expr::mul(vec![
            Expr::rational(r.sub(&Rational::integer(1)).mul(c)),
            Expr::index(),
        ])
    };
    let u = Expr::add(vec![
        Expr::mul(vec![
            Expr::rational(d.clone()),
            Expr::pow(Expr::rational(r.clone()), NatExpr::offset(-1)),
        ]),
        h,
    ]);
    let v = Expr::mul(vec![
        Expr::rational(c.clone()),
        Expr::pow(Expr::rational(s.clone()), NatExpr::offset(-1)),
    ]);
    let af = Expr::mul(vec![
        g.clone(),
        Expr::rational(Rational::fraction(1, 2)),
        Expr::add(vec![u.clone(), v.clone()]),
    ]);
    let bf = Expr::mul(vec![
        g.clone(),
        Expr::rational(Rational::fraction(1, 2).div(&t)?),
        Expr::sub(u, v),
    ]);
    let diagonal = r.add(s).div(&Rational::integer(2))?;
    let off_a = t.mul(&r.sub(s)).div(&Rational::integer(2))?;
    let off_b = r.sub(s).div(&Rational::integer(2).mul(&t))?;
    let build_rhs = |a_coeff: Rational, b_coeff: Rational, force_coeff: Rational| {
        Expr::add(vec![
            Expr::mul(vec![
                next.clone(),
                Expr::add(vec![
                    Expr::mul(vec![Expr::rational(a_coeff), Expr::term("a", 0)]),
                    Expr::mul(vec![Expr::rational(b_coeff), Expr::term("b", 0)]),
                ]),
            ]),
            Expr::mul(vec![
                g.clone(),
                next.clone(),
                Expr::rational(force_coeff),
                forcing.clone(),
            ]),
        ])
    };
    let no_terms = |_: &str, _: u32| Err("一般項に数列の項が残っています。".into());
    let a1 = af.evaluate(1, &no_terms)?;
    let b1 = bf.evaluate(1, &no_terms)?;
    let mut conditions = Vec::new();
    if scaled {
        conditions.push(DomainCondition {
            kind: "nonzero".into(),
            expression: g.clone(),
            explanation: "n≥1なので、正規化に用いる倍率は正であり零にならない。".into(),
        });
    }
    Ok(ProblemIR {
        kind: RecurrenceKind::System,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(
            Expr::mul(vec![g.clone(), Expr::term("a", 1)]),
            build_rhs(diagonal.clone(), off_a, Rational::fraction(1, 2)),
        ),
        secondary_recurrences: vec![Equation::new(
            Expr::mul(vec![g.clone(), Expr::term("b", 1)]),
            build_rhs(off_b, diagonal, Rational::fraction(1, 2).div(&t)?),
        )],
        initials: vec![initial("a", a1), initial("b", b1)],
        general_term: Equation::new(Expr::term("a", 0), af),
        secondary_general_terms: vec![Equation::new(Expr::term("b", 0), bf)],
        conditions,
    })
}

fn rational(x: i64) -> Rational {
    Rational::integer(x)
}
fn poly_add(mut a: Poly, b: Poly) -> Poly {
    for (k, v) in b {
        let value = a.get(&k).cloned().unwrap_or_else(|| rational(0)).add(&v);
        if value.is_zero() {
            a.remove(&k);
        } else {
            a.insert(k, value);
        }
    }
    a
}
fn poly_mul(a: &Poly, b: &Poly) -> Poly {
    let mut out = Poly::new();
    for (ka, va) in a {
        for (kb, vb) in b {
            let k = ka + kb;
            let v = out
                .get(&k)
                .cloned()
                .unwrap_or_else(|| rational(0))
                .add(&va.mul(vb));
            if v.is_zero() {
                out.remove(&k);
            } else {
                out.insert(k, v);
            }
        }
    }
    out
}
fn nat_poly(n: &NatExpr) -> Result<Poly, String> {
    Ok(match n {
        NatExpr::Index => [(1, rational(1))].into_iter().collect(),
        NatExpr::Constant { value } => {
            if *value == 0 {
                Poly::new()
            } else {
                [(0, rational(i64::from(*value)))].into_iter().collect()
            }
        }
        NatExpr::Add { left, right } => poly_add(nat_poly(left)?, nat_poly(right)?),
        NatExpr::Sub { left, right }
            if **left == NatExpr::Index && **right == NatExpr::constant(1) =>
        {
            [(1, rational(1)), (0, rational(-1))].into_iter().collect()
        }
        _ => return Err("連立係数に未登録の自然数式があります。".into()),
    })
}
fn polynomial(e: &Expr) -> Result<Poly, String> {
    Ok(match e {
        Expr::Rational { value } => {
            if value.is_zero() {
                Poly::new()
            } else {
                [(0, value.clone())].into_iter().collect()
            }
        }
        Expr::NatCast { value } => nat_poly(value)?,
        Expr::Add { terms } => terms.iter().try_fold(Poly::new(), |a, b| {
            Ok::<_, String>(poly_add(a, polynomial(b)?))
        })?,
        Expr::Mul { factors } => factors
            .iter()
            .try_fold([(0, rational(1))].into_iter().collect(), |a, b| {
                Ok::<_, String>(poly_mul(&a, &polynomial(b)?))
            })?,
        Expr::Div {
            numerator,
            denominator,
        } => {
            let d = polynomial(denominator)?;
            if d.len() != 1 || !d.contains_key(&0) {
                return Err("連立係数の除数が定数ではありません。".into());
            }
            polynomial(numerator)?
                .into_iter()
                .map(|(k, v)| Ok((k, v.div(&d[&0])?)))
                .collect::<Result<_, String>>()?
        }
        _ => return Err("連立係数が多項式ではありません。".into()),
    })
}
fn poly_scale(p: &Poly, r: &Rational) -> Poly {
    p.iter()
        .filter_map(|(k, v)| {
            let v = v.mul(r);
            (!v.is_zero()).then_some((*k, v))
        })
        .collect()
}
fn proportional(a: &Poly, b: &Poly) -> Result<Rational, String> {
    if a.is_empty() {
        return Ok(rational(0));
    }
    let (k, v) = b.last_key_value().ok_or("倍率の多項式が零です。")?;
    let r = a
        .get(k)
        .ok_or("連立係数の次数が倍率と一致しません。")?
        .div(v)?;
    if poly_scale(b, &r) != *a {
        return Err("連立係数から定数行列を検出できません。".into());
    }
    Ok(r)
}
fn poly_div(mut a: Poly, b: &Poly) -> Result<Poly, String> {
    let mut out = Poly::new();
    let (degree, leading) = b.last_key_value().ok_or("倍率が零です。")?;
    while let Some((&d, c)) = a.last_key_value() {
        if d < *degree {
            break;
        }
        let offset = d - degree;
        let r = c.div(leading)?;
        out.insert(offset, r.clone());
        let subtract = b
            .iter()
            .map(|(k, v)| (k + offset, rational(-1).mul(&r).mul(v)))
            .collect();
        a = poly_add(a, subtract);
    }
    if !a.is_empty() {
        return Err("連立の付加項が登録プロファイルに一致しません。".into());
    }
    Ok(out)
}
fn poly_expr(p: &Poly) -> Expr {
    Expr::add(
        p.iter()
            .rev()
            .map(|(degree, c)| {
                if *degree == 0 {
                    Expr::rational(c.clone())
                } else {
                    Expr::mul(vec![
                        Expr::rational(c.clone()),
                        if *degree == 1 {
                            Expr::index()
                        } else {
                            Expr::pow(Expr::index(), NatExpr::constant(*degree))
                        },
                    ])
                }
            })
            .collect(),
    )
}
fn contains(e: &Expr, target: &Expr) -> bool {
    e == target
        || match e {
            Expr::Add { terms } => terms.iter().any(|e| contains(e, target)),
            Expr::Mul { factors } => factors.iter().any(|e| contains(e, target)),
            Expr::Div {
                numerator,
                denominator,
            } => contains(numerator, target) || contains(denominator, target),
            Expr::Pow { base, .. } => contains(base, target),
            _ => false,
        }
}
fn linear(e: &Expr, target: &Expr) -> Result<(Expr, Expr), String> {
    if e == target {
        return Ok((Expr::integer(1), Expr::integer(0)));
    }
    if !contains(e, target) {
        return Ok((Expr::integer(0), e.clone()));
    }
    match e {
        Expr::Add { terms } => {
            let pairs = terms
                .iter()
                .map(|e| linear(e, target))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                Expr::add(pairs.iter().map(|p| p.0.clone()).collect()),
                Expr::add(pairs.into_iter().map(|p| p.1).collect()),
            ))
        }
        Expr::Mul { factors } => {
            let mut a = Expr::integer(1);
            let mut b = Expr::integer(1);
            let mut dependent = false;
            for f in factors {
                let (c, d) = linear(f, target)?;
                if c != Expr::integer(0) {
                    if dependent {
                        return Err("連立式に非線形の項があります。".into());
                    }
                    dependent = true;
                    a = Expr::mul(vec![a, c]);
                    b = Expr::mul(vec![b, d]);
                } else {
                    a = Expr::mul(vec![a, d.clone()]);
                    b = Expr::mul(vec![b, d]);
                }
            }
            Ok((a, b))
        }
        Expr::Div {
            numerator,
            denominator,
        } if !contains(denominator, target) => {
            let (a, b) = linear(numerator, target)?;
            Ok((
                Expr::div(a, *denominator.clone()),
                Expr::div(b, *denominator.clone()),
            ))
        }
        _ => Err("連立の一次係数を抽出できません。".into()),
    }
}
#[derive(Clone)]
struct Modes {
    scale: Expr,
    r: Rational,
    s: Rational,
    t: Rational,
    forcing: Poly,
    u1: Rational,
    v1: Rational,
}
fn detect(problem: &ProblemIR) -> Result<Modes, String> {
    if problem.kind != RecurrenceKind::System || problem.secondary_recurrences.len() != 1 {
        return Err("2数列の連立漸化式ではありません。".into());
    }
    let (a_scale, rest) = linear(&problem.recurrence.lhs, &Expr::term("a", 1))?;
    let (b_scale, brest) = linear(&problem.secondary_recurrences[0].lhs, &Expr::term("b", 1))?;
    if rest != Expr::integer(0)
        || brest != Expr::integer(0)
        || polynomial(&a_scale)? != polynomial(&b_scale)?
    {
        return Err("連立の左辺の共通倍率を検出できません。".into());
    }
    let g = polynomial(&a_scale)?;
    let (scale, next) = if g == polynomial(&Expr::integer(1))? {
        (Expr::integer(1), Expr::integer(1))
    } else {
        (0..=2)
            .map(|profile| (self::scale(profile, false), self::scale(profile, true)))
            .find(|(s, _)| polynomial(s).is_ok_and(|p| p == g))
            .ok_or("連立の倍率が登録プロファイルにありません。")?
    };
    let next_poly = polynomial(&next)?;
    let (a_coeff, a_rest) = linear(&problem.recurrence.rhs, &Expr::term("a", 0))?;
    let (b_coeff, a_force) = linear(&a_rest, &Expr::term("b", 0))?;
    let (z_coeff, b_rest) = linear(&problem.secondary_recurrences[0].rhs, &Expr::term("a", 0))?;
    let (w_coeff, b_force) = linear(&b_rest, &Expr::term("b", 0))?;
    let p = proportional(&polynomial(&a_coeff)?, &next_poly)?;
    let q = proportional(&polynomial(&b_coeff)?, &next_poly)?;
    let z = proportional(&polynomial(&z_coeff)?, &next_poly)?;
    let w = proportional(&polynomial(&w_coeff)?, &next_poly)?;
    if p != w {
        return Err("連立行列の対角成分が一致しません。".into());
    }
    let t = (1..=3)
        .map(rational)
        .find(|t| z.mul(t).mul(t) == q)
        .ok_or("連立を分離する一次結合を検出できません。")?;
    let r = p.add(&q.div(&t)?);
    let s = p.sub(&q.div(&t)?);
    if r == s || r.is_zero() || s.is_zero() || r.is_one() {
        return Err("連立のモードが退化しています。".into());
    }
    let forcing = poly_div(
        poly_scale(&polynomial(&a_force)?, &rational(2)),
        &poly_mul(&g, &next_poly),
    )?;
    if forcing.last_key_value().is_some_and(|(d, _)| *d > 1) {
        return Err("2次以上の連立付加項は未登録です。".into());
    }
    if polynomial(&b_force)? != poly_scale(&polynomial(&a_force)?, &rational(1).div(&t)?) {
        return Err("2つの付加項が一次結合の分離条件に一致しません。".into());
    }
    let eval = |e: &Expr| e.evaluate(1, &|_, _| Err("初期条件が定数ではありません。".into()));
    let find_initial = |sequence: &str| {
        problem
            .initials
            .iter()
            .find(|eq| {
                eq.lhs
                    == Expr::Term {
                        sequence: sequence.into(),
                        index: NatExpr::constant(1),
                    }
            })
            .ok_or("連立の初期条件が不足しています。".to_string())
            .and_then(|eq| eval(&eq.rhs))
    };
    let first = eval(&scale)?;
    let a1 = find_initial("a")?.div(&first)?;
    let b1 = find_initial("b")?.div(&first)?;
    Ok(Modes {
        scale,
        r,
        s,
        t: t.clone(),
        forcing,
        u1: a1.add(&t.mul(&b1)),
        v1: a1.sub(&t.mul(&b1)),
    })
}
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    if problem.kind != RecurrenceKind::System {
        return Ok(None);
    }
    let m = detect(problem)?;
    let degree = m.forcing.last_key_value().map(|(d, _)| *d).unwrap_or(0);
    let normalized = m.scale != Expr::integer(1);
    let forced = !m.forcing.is_empty();
    let (score, reason) = if degree > 0 {
        (20, "連立の分離と一次式の特解を組み合わせる")
    } else if normalized {
        (14, "正規化してから連立を分離する")
    } else {
        (9, "連立を和・差や一次結合で分離する")
    };
    let mut operations = Vec::new();
    if normalized {
        operations.push("index_scale");
    }
    operations.push("split_pair");
    if degree > 0 {
        operations.push("polynomial_shift");
    } else if forced {
        operations.extend(["fixed_point", "shift"]);
    }
    operations.extend(["geometric", "recover_pair"]);
    let classification = crate::generator::make_classification(
        problem,
        score,
        "一次結合で連立を分離する",
        reason,
        &operations,
        1 + u8::from(normalized) + u8::from(m.t != rational(1)) + u8::from(forced),
        u8::from(normalized),
        if normalized { 1 } else { degree as u8 },
    )?;
    let two = rational(2);
    let p = m.r.add(&m.s).div(&two)?;
    let q = m.t.mul(&m.r.sub(&m.s)).div(&two)?;
    let z = m.r.sub(&m.s).div(&two.mul(&m.t))?;
    let a1 = m.u1.add(&m.v1).div(&two)?;
    let b1 = m.u1.sub(&m.v1).div(&two.mul(&m.t))?;
    // Match the reference outer paired-modes route: normalized matrix and
    // initials are measured, rather than all nested scalar substitutions.
    crate::generator::include_intermediate_values(
        problem,
        classification,
        &[p.clone(), q, z, p, a1, b1, m.r, m.s],
    )
    .map(Some)
}
fn step(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn at_first(sequence: &str) -> Expr {
    Expr::Term {
        sequence: sequence.into(),
        index: NatExpr::constant(1),
    }
}
pub fn explain(
    problem: &ProblemIR,
    classification: &Classification,
) -> Result<Option<DerivationIR>, String> {
    if classification.method_name != "一次結合で連立を分離する" {
        return Ok(None);
    }
    let m = detect(problem)?;
    let u = Expr::term("u", 0);
    let v = Expr::term("v", 0);
    let scaled = m.scale != Expr::integer(1);
    let def_u = Equation::new(
        u.clone(),
        Expr::div(
            Expr::add(vec![
                Expr::term("a", 0),
                Expr::mul(vec![Expr::rational(m.t.clone()), Expr::term("b", 0)]),
            ]),
            m.scale.clone(),
        ),
    );
    let def_v = Equation::new(
        v.clone(),
        Expr::div(
            Expr::sub(
                Expr::term("a", 0),
                Expr::mul(vec![Expr::rational(m.t.clone()), Expr::term("b", 0)]),
            ),
            m.scale.clone(),
        ),
    );
    let rec_u = Equation::new(
        Expr::term("u", 1),
        Expr::add(vec![
            Expr::mul(vec![Expr::rational(m.r.clone()), u.clone()]),
            poly_expr(&m.forcing),
        ]),
    );
    let rec_v = Equation::new(
        Expr::term("v", 1),
        Expr::mul(vec![Expr::rational(m.s.clone()), v.clone()]),
    );
    let init_u = Equation::new(at_first("u"), Expr::rational(m.u1.clone()));
    let init_v = Equation::new(at_first("v"), Expr::rational(m.v1.clone()));
    let mut steps = Vec::new();
    if scaled {
        steps.push(step(
            "nは1以上なので、共通倍率は正である。2つの数列を同じ倍率で割って正規化する。",
            None,
            "正規化の分母が零にならない",
        ));
    }
    steps.push(step(&format!("行列の対角成分は等しく、非対角成分の比は重み{}の2乗である。和の一次結合を次のようにおく。",m.t),Some(def_u.clone()),"2つの式を重みを掛けて足す"));
    steps.push(step(
        "差の一次結合も定義する。この2つは独立で、元の2数列を復元できる。",
        Some(def_v.clone()),
        "2つの式を重みを掛けて引く",
    ));
    steps.push(step(
        "与式を代入して整理すると、和の一次結合は次の1数列の漸化式を満たす。",
        Some(rec_u.clone()),
        "共通倍率を相殺して係数をまとめる",
    ));
    steps.push(step(
        "差の一次結合では付加項が相殺され、等比数列となる。",
        Some(rec_v.clone()),
        "分離した第2の漸化式",
    ));
    steps.push(step(
        "2つの初期条件を代入する。和の初項は次の値である。",
        Some(init_u.clone()),
        "a₁とb₁を同時に用いる",
    ));
    steps.push(step(
        "差の初項は次の値である。",
        Some(init_v.clone()),
        "a₁とb₁を同時に用いる",
    ));
    let particular = if m.forcing.is_empty() {
        Expr::integer(0)
    } else {
        let linear = m.forcing.get(&1).cloned().unwrap_or_else(|| rational(0));
        let constant = m.forcing.get(&0).cloned().unwrap_or_else(|| rational(0));
        let alpha = linear.div(&rational(1).sub(&m.r))?;
        let beta = constant.sub(&alpha).div(&rational(1).sub(&m.r))?;
        Expr::add(vec![
            Expr::mul(vec![Expr::rational(alpha), Expr::index()]),
            Expr::rational(beta),
        ])
    };
    if !m.forcing.is_empty() {
        steps.push(step(
            "付加項と同じ次数の特解を考える。係数を比較すると、次の特解が得られる。",
            Some(Equation::new(Expr::term("h", 0), particular.clone())),
            "h_(n+1)−r h_nが付加項に等しくなるように係数を決める",
        ));
        steps.push(step(
            "u_nからこの特解を引くと、付加項が消え、公比が一定の等比数列となる。",
            Some(Equation::new(
                Expr::term("w", 0),
                Expr::sub(u.clone(), particular.clone()),
            )),
            "特解による定数移動",
        ));
    }
    let hp = particular.evaluate(1, &|_, _| Err("特解に未知数があります。".into()))?;
    let u_formula = Expr::add(vec![
        Expr::mul(vec![
            Expr::rational(m.u1.sub(&hp)),
            Expr::pow(Expr::rational(m.r.clone()), NatExpr::offset(-1)),
        ]),
        particular,
    ]);
    let v_formula = Expr::mul(vec![
        Expr::rational(m.v1.clone()),
        Expr::pow(Expr::rational(m.s.clone()), NatExpr::offset(-1)),
    ]);
    steps.push(step(
        "等比数列の一般項と初期条件から、和の一次結合を求める。",
        Some(Equation::new(u.clone(), u_formula)),
        "初項からn−1回、公比を掛ける",
    ));
    steps.push(step(
        "差の一次結合の一般項も求める。",
        Some(Equation::new(v.clone(), v_formula)),
        "等比数列の一般項",
    ));
    steps.push(step(
        "定義した2つの式を足し、共通倍率を掛け戻すと、第1の数列を復元できる。",
        Some(Equation::new(
            Expr::term("a", 0),
            Expr::mul(vec![
                m.scale.clone(),
                Expr::rational(Rational::fraction(1, 2)),
                Expr::add(vec![u.clone(), v.clone()]),
            ]),
        )),
        "一次結合の逆変換",
    ));
    steps.push(step(
        "定義した2つの式の差をとり、重みで割ると、第2の数列を復元できる。",
        Some(Equation::new(
            Expr::term("b", 0),
            Expr::mul(vec![
                m.scale,
                Expr::rational(Rational::fraction(1, 2).div(&m.t)?),
                Expr::sub(u, v),
            ]),
        )),
        "重みは非零",
    ));
    steps.push(step(
        "求めた2つの一般項を代入すると、第1の数列の答えは次のとおりである。",
        Some(problem.general_term.clone()),
        "分離した2モードを代入",
    ));
    steps.push(step(
        "第2の数列の答えは次のとおりである。",
        Some(problem.secondary_general_terms[0].clone()),
        "分離した2モードを代入",
    ));
    steps.push(step("これらの一般項は両方の初期条件と両方の漸化式をすべての添字で満たす。初期条件から2数列の次の項が順に定まるため、解は一意である。",None,"Leanで連立全体の初期条件・漸化式・一意性を検証"));
    Ok(Some(DerivationIR {
        method_name: classification.method_name.clone(),
        hint: format!(
            "a_n+{}b_nとa_n−{}b_nを考え{}、2つの漸化式を分離する。",
            m.t,
            m.t,
            if scaled {
                "、共通倍率で割り"
            } else {
                ""
            }
        ),
        auxiliaries: vec![
            AuxiliarySequence {
                name: "u".into(),
                definition: def_u,
                initial: init_u,
                recurrence: rec_u,
            },
            AuxiliarySequence {
                name: "v".into(),
                definition: def_v,
                initial: init_v,
                recurrence: rec_v,
            },
        ],
        steps,
        alternatives: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example(family: &str, profile: i64) -> ProblemIR {
        let p = params(&[
            ("r", rational(2)),
            ("s", rational(3)),
            ("c", rational(2)),
            ("d", rational(4)),
            ("profile", rational(profile)),
        ]);
        compile(&build_recipe(family, 42, p).unwrap()).unwrap()
    }
    #[test]
    fn both_sequences_satisfy_both_initials_and_every_registered_profile() {
        for (family, profiles) in [
            ("coupled_symmetric", vec![0]),
            ("coupled_weighted", vec![0]),
            ("coupled_scaled", vec![0, 1, 2]),
            ("coupled_forced", vec![0, 1]),
        ] {
            for profile in profiles {
                let p = example(family, profile);
                let term = |sequence: &str, index: u32| match sequence {
                    "a" => p
                        .general_term
                        .rhs
                        .evaluate(index, &|_, _| Err("unexpected sequence dependency".into())),
                    "b" => p.secondary_general_terms[0]
                        .rhs
                        .evaluate(index, &|_, _| Err("unexpected sequence dependency".into())),
                    _ => Err("unknown sequence".into()),
                };
                for eq in &p.initials {
                    assert_eq!(
                        eq.lhs.evaluate(1, &term).unwrap(),
                        eq.rhs.evaluate(1, &term).unwrap(),
                        "{family}/{profile}"
                    );
                }
                for n in 1..=12 {
                    for eq in std::iter::once(&p.recurrence).chain(&p.secondary_recurrences) {
                        assert_eq!(
                            eq.lhs.evaluate(n, &term).unwrap(),
                            eq.rhs.evaluate(n, &term).unwrap(),
                            "{family}/{profile}, n={n}"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn forward_detection_ignores_answers_and_detects_broken_matrix() {
        let mut p = example("coupled_forced", 1);
        let m = detect(&p).unwrap();
        assert_eq!(m.forcing.keys().copied().collect::<Vec<_>>(), vec![1]);
        p.general_term.rhs = Expr::integer(0);
        p.secondary_general_terms[0].rhs = Expr::integer(0);
        assert_eq!(detect(&p).unwrap().r, m.r);
        p.secondary_recurrences[0].rhs = Expr::integer(0);
        assert!(detect(&p).is_err());
    }

    #[test]
    fn normalized_initials_count_toward_the_reference_numeric_budget() {
        // The pinned reference measures cost 9, although the visible constants
        // alone cost 3. Omitting the normalized initials accepted this problem.
        let parameters = params(&[
            ("r", rational(2)),
            ("s", rational(5)),
            ("c", rational(4)),
            ("d", Rational::fraction(1, 3)),
            ("profile", rational(2)),
        ]);
        let mut p = compile(&build_recipe("coupled_scaled", 7, parameters).unwrap()).unwrap();
        assert!(classify(&p).is_err());
        p.general_term.rhs = Expr::integer(0);
        p.secondary_general_terms[0].rhs = Expr::integer(0);
        assert!(classify(&p).is_err());
    }
    #[test]
    fn odd_profile_has_reference_multiplier_two_n_plus_one() {
        let p = example("coupled_scaled", 2);
        assert_eq!(
            detect(&p)
                .unwrap()
                .scale
                .evaluate(1, &|_, _| Err("term".into()))
                .unwrap(),
            rational(3)
        );
    }
}
