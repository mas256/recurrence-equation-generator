//! Forward-recognized additive second-order compositions from reference 0.7.
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
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
fn rat(v: &Rational) -> Expr {
    Expr::rational(v.clone())
}
fn block(id: &str, input: &str, kind: &str, parameters: Params) -> RecipeBlock {
    RecipeBlock {
        id: id.into(),
        input: input.into(),
        kind: kind.into(),
        parameters,
    }
}
pub fn catalog() -> Vec<FamilyInfo> {
    [
        ("scaled_second_order", "正規化＋3項間", 4),
        ("difference_scaled", "階差＋階比＋総和", 5),
        ("forced_second", "定数項を含む3項間", 4),
        ("arithmetic_difference", "等差と階差の組合せ", 4),
    ]
    .into_iter()
    .map(|(id, label, level)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels: vec![level],
    })
    .collect()
}
pub fn supports(family: &str) -> bool {
    matches!(
        family,
        "scaled_second_order" | "difference_scaled" | "forced_second" | "arithmetic_difference"
    )
}
pub fn sample(family: &str, _level: u8, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    let c = Rational::integer(rng.gen_range(1..=6));
    let d = Rational::integer(rng.gen_range(1..=6));
    let r = Rational::integer(rng.gen_range(2..=4));
    let mut s = Rational::integer(rng.gen_range(2..=4));
    while r == s {
        s = Rational::integer(rng.gen_range(2..=4));
    }
    let mut p = match family {
        "arithmetic_difference" => map(&[
            ("c", c),
            ("d", d),
            ("s", Rational::integer(rng.gen_range(1..=6))),
        ]),
        "difference_scaled" => map(&[("c", c), ("d", d), ("r", r)]),
        _ => map(&[("c", c), ("d", d), ("r", r), ("s", s)]),
    };
    if matches!(family, "scaled_second_order" | "difference_scaled") {
        p.insert("profile".into(), Rational::integer(rng.gen_range(0..=2)));
        p.insert("profile_version".into(), Rational::integer(1));
    }
    Ok(p)
}
pub fn validate_parameters(family: &str, p: &Params) -> Result<(), String> {
    let names: &[&str] = match family {
        "scaled_second_order" => &["c", "d", "r", "s", "profile", "profile_version"],
        "difference_scaled" => &["c", "d", "r", "profile", "profile_version"],
        "forced_second" => &["c", "d", "r", "s"],
        "arithmetic_difference" => &["c", "d", "s"],
        _ => return Err("未登録の3項間の型です。".into()),
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.iter().copied().collect() {
        return Err("3項間Recipeのパラメータ集合が規則と一致しません。".into());
    }
    for name in ["c", "d"] {
        if !p[name].is_positive() || p[name].0 > Rational::integer(16).0 {
            return Err("初期値の係数は正の有理数で16以下です。".into());
        }
    }
    if family == "arithmetic_difference" {
        if !p["s"].is_positive() || p["s"].0 > Rational::integer(16).0 {
            return Err("初階差は正の有理数で16以下です。".into());
        }
    } else {
        for name in if family == "difference_scaled" {
            vec!["r"]
        } else {
            vec!["r", "s"]
        } {
            if !p[name].integer_u32().is_some_and(|x| (2..=5).contains(&x)) {
                return Err("特性根は2から5の整数です。".into());
            }
        }
        if p.get("s") == p.get("r") {
            return Err("異なる特性根が必要です。".into());
        }
    }
    if matches!(family, "scaled_second_order" | "difference_scaled") {
        profile(p)?;
    }
    Ok(())
}
fn profile(p: &Params) -> Result<u32, String> {
    if p.get("profile_version") != Some(&Rational::integer(1)) {
        return Err("倍率profileの版が一致しません。".into());
    }
    p.get("profile")
        .and_then(Rational::integer_u32)
        .filter(|x| *x <= 2)
        .ok_or("倍率profileは0(n)、1(n+1)、2(2n+1)です。".into())
}
fn scale(profile: u32, offset: i32) -> Expr {
    match profile {
        0 => Expr::offset(offset),
        1 => Expr::offset(offset + 1),
        2 => Expr::add(vec![
            Expr::mul(vec![Expr::integer(2), Expr::offset(offset)]),
            Expr::integer(1),
        ]),
        _ => unreachable!(),
    }
}
pub fn build_recipe(family: &str, seed: u64, p: Params) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    let (core, mut blocks) = match family {
        "arithmetic_difference" => (
            RecipeCore {
                id: "core".into(),
                kind: "constant".into(),
                parameters: map(&[("initial", p["d"].clone())]),
            },
            vec![block(
                "b1",
                "core",
                "second_difference",
                map(&[
                    ("first_difference", p["s"].clone()),
                    ("increment", p["c"].clone()),
                ]),
            )],
        ),
        "difference_scaled" => (
            RecipeCore {
                id: "core".into(),
                kind: "geometric".into(),
                parameters: map(&[("amplitude", p["d"].clone()), ("ratio", p["r"].clone())]),
            },
            vec![
                block(
                    "b1",
                    "core",
                    "index_scale",
                    map(&[
                        ("profile", p["profile"].clone()),
                        ("profile_version", Rational::integer(1)),
                    ]),
                ),
                block(
                    "b2",
                    "b1",
                    "difference_lift",
                    map(&[("initial", p["c"].clone())]),
                ),
            ],
        ),
        _ => {
            let core = RecipeCore {
                id: "core".into(),
                kind: "geometric".into(),
                parameters: map(&[("amplitude", p["d"].clone()), ("ratio", p["r"].clone())]),
            };
            let mut blocks = vec![block(
                "b1",
                "core",
                "linear_combination",
                map(&[("amplitude", p["c"].clone()), ("ratio", p["s"].clone())]),
            )];
            blocks.push(if family == "forced_second" {
                block(
                    "b2",
                    "b1",
                    "add_constant",
                    map(&[("value", p["c"].clone())]),
                )
            } else {
                block(
                    "b2",
                    "b1",
                    "index_scale",
                    map(&[
                        ("profile", p["profile"].clone()),
                        ("profile_version", Rational::integer(1)),
                    ]),
                )
            });
            (core, blocks)
        }
    };
    let output = blocks.last().unwrap().id.clone();
    Ok(Recipe {
        schema_version: crate::generator::SCHEMA_VERSION.into(),
        rules_version: crate::generator::EXTENSION_RULES_VERSION.into(),
        rng_version: crate::generator::RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p,
        core,
        blocks: std::mem::take(&mut blocks),
        output,
        index_start: 1,
    })
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
fn nonzero(expression: Expr, explanation: &str) -> DomainCondition {
    DomainCondition {
        kind: "nonzero".into(),
        expression,
        explanation: explanation.into(),
    }
}
fn geom(amplitude: &Rational, ratio: &Rational) -> Expr {
    Expr::mul(vec![
        rat(amplitude),
        Expr::pow(rat(ratio), NatExpr::offset(-1)),
    ])
}
fn linear(a: Rational, b: Rational) -> Expr {
    Expr::add(vec![Expr::mul(vec![rat(&a), Expr::index()]), rat(&b)])
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    validate_parameters(&recipe.family, &recipe.parameters)?;
    let p = &recipe.parameters;
    let a = Expr::term("a", 0);
    let a1 = Expr::term("a", 1);
    let a2 = Expr::term("a", 2);
    let mut conditions = vec![];
    let (coefficient, q, r, forcing, formula) = match recipe.family.as_str() {
        "arithmetic_difference" => {
            let k = Expr::offset(-1);
            (
                Expr::integer(1),
                Expr::integer(2),
                Expr::integer(-1),
                rat(&p["c"]),
                Expr::add(vec![
                    rat(&p["d"]),
                    Expr::mul(vec![rat(&p["s"]), k.clone()]),
                    Expr::mul(vec![
                        rat(&p["c"].div(&Rational::integer(2))?),
                        k.clone(),
                        Expr::sub(k, Expr::integer(1)),
                    ]),
                ]),
            )
        }
        "forced_second" => {
            let q = p["r"].add(&p["s"]);
            let r = Rational::integer(-1).mul(&p["r"]).mul(&p["s"]);
            let forcing = p["c"].mul(&Rational::integer(1).sub(&q).sub(&r));
            (
                Expr::integer(1),
                rat(&q),
                rat(&r),
                rat(&forcing),
                Expr::add(vec![
                    geom(&p["d"], &p["r"]),
                    geom(&p["c"], &p["s"]),
                    rat(&p["c"]),
                ]),
            )
        }
        "scaled_second_order" => {
            let profile = profile(p)?;
            let g = scale(profile, 0);
            let g1 = scale(profile, 1);
            let g2 = scale(profile, 2);
            let pp = Expr::mul(vec![g.clone(), g1.clone()]);
            conditions.push(nonzero(
                g.clone(),
                "nは1以上なので正規化する倍率は正で零にならない。",
            ));
            conditions.push(nonzero(
                pp.clone(),
                "次々項の係数は正の倍率の積であり、次項を一意に定められる。",
            ));
            (
                pp,
                Expr::mul(vec![rat(&p["r"].add(&p["s"])), g.clone(), g2.clone()]),
                Expr::mul(vec![
                    rat(&Rational::integer(-1).mul(&p["r"]).mul(&p["s"])),
                    g1,
                    g2,
                ]),
                Expr::integer(0),
                Expr::mul(vec![
                    g,
                    Expr::add(vec![geom(&p["d"], &p["r"]), geom(&p["c"], &p["s"])]),
                ]),
            )
        }
        "difference_scaled" => {
            let profile = profile(p)?;
            let g = scale(profile, 0);
            let g1 = scale(profile, 1);
            let (slope, intercept) = if profile == 2 {
                (Rational::integer(2), Rational::integer(1))
            } else {
                (Rational::integer(1), Rational::integer(profile as i64))
            };
            let den = p["r"].sub(&Rational::integer(1));
            let u = p["d"].mul(&slope).div(&den)?;
            let v = p["d"].mul(&intercept).sub(&p["r"].mul(&u)).div(&den)?;
            let h1 = u.add(&v);
            if p["c"] == h1 {
                return Err("階差を戻す定数が零で短い解法に退化します。".into());
            }
            let q_slope = slope.mul(&p["r"].add(&Rational::integer(1)));
            let q_intercept = intercept.add(&p["r"].mul(&slope.add(&intercept)));
            conditions.push(nonzero(
                g.clone(),
                "nは1以上なので階差の正規化と次々項の係数は零にならない。",
            ));
            (
                g,
                linear(q_slope, q_intercept),
                Expr::mul(vec![rat(&Rational::integer(-1).mul(&p["r"])), g1]),
                Expr::integer(0),
                Expr::add(vec![
                    rat(&p["c"].sub(&h1)),
                    Expr::mul(vec![
                        linear(u, v),
                        Expr::pow(rat(&p["r"]), NatExpr::offset(-1)),
                    ]),
                ]),
            )
        }
        _ => return Err("未登録の3項間の型です。".into()),
    };
    let initials = vec![
        initial(
            1,
            formula.evaluate(1, &|_, _| Err("unexpected sequence".into()))?,
        ),
        initial(
            2,
            formula.evaluate(2, &|_, _| Err("unexpected sequence".into()))?,
        ),
    ];
    Ok(ProblemIR {
        kind: RecurrenceKind::SecondOrder,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(
            Expr::mul(vec![coefficient, a2]),
            Expr::add(vec![Expr::mul(vec![q, a1]), Expr::mul(vec![r, a]), forcing]),
        ),
        initials,
        general_term: Equation::new(Expr::term("a", 0), formula),
        conditions,
        secondary_recurrences: vec![],
        secondary_general_terms: vec![],
    })
}

// Polynomial equality is an exact symbolic check on the displayed coefficients.
// It never samples a sequence or uses Recipe parameters to choose a method.
type Poly = Vec<Rational>;
fn trim(mut p: Poly) -> Poly {
    while p.last().is_some_and(Rational::is_zero) {
        p.pop();
    }
    p
}
fn padd(p: &Poly, q: &Poly) -> Poly {
    trim(
        (0..p.len().max(q.len()))
            .map(|i| {
                p.get(i)
                    .cloned()
                    .unwrap_or(Rational::integer(0))
                    .add(&q.get(i).cloned().unwrap_or(Rational::integer(0)))
            })
            .collect(),
    )
}
fn pscale(p: &Poly, r: &Rational) -> Poly {
    trim(p.iter().map(|x| x.mul(r)).collect())
}
fn pmul(p: &Poly, q: &Poly) -> Poly {
    let mut out = vec![Rational::integer(0); p.len() + q.len()];
    for (i, a) in p.iter().enumerate() {
        for (j, b) in q.iter().enumerate() {
            out[i + j] = out[i + j].add(&a.mul(b));
        }
    }
    trim(out)
}
fn npoly(n: &NatExpr) -> Result<Poly, String> {
    Ok(match n {
        NatExpr::Index => vec![Rational::integer(0), Rational::integer(1)],
        NatExpr::Constant { value } => trim(vec![Rational::integer(*value as i64)]),
        NatExpr::Add { left, right } => padd(&npoly(left)?, &npoly(right)?),
        NatExpr::Sub { left, right } => padd(
            &npoly(left)?,
            &pscale(&npoly(right)?, &Rational::integer(-1)),
        ),
        _ => return Err("多項式でない添字です。".into()),
    })
}
fn poly(e: &Expr) -> Result<Poly, String> {
    Ok(match e {
        Expr::Rational { value } => trim(vec![value.clone()]),
        Expr::NatCast { value } => npoly(value)?,
        Expr::Add { terms } => terms
            .iter()
            .try_fold(vec![], |p, e| Ok::<_, String>(padd(&p, &poly(e)?)))?,
        Expr::Mul { factors } => factors
            .iter()
            .try_fold(vec![Rational::integer(1)], |p, e| {
                Ok::<_, String>(pmul(&p, &poly(e)?))
            })?,
        Expr::Div {
            numerator,
            denominator,
        } => {
            let d = poly(denominator)?;
            if d.len() != 1 {
                return Err("定数以外の分母です。".into());
            }
            pscale(&poly(numerator)?, &Rational::integer(1).div(&d[0])?)
        }
        _ => return Err("係数が多項式ではありません。".into()),
    })
}
fn contains(e: &Expr, target: &Expr) -> bool {
    if e == target {
        return true;
    }
    match e {
        Expr::Add { terms } => terms.iter().any(|e| contains(e, target)),
        Expr::Mul { factors } => factors.iter().any(|e| contains(e, target)),
        Expr::Div {
            numerator,
            denominator,
        } => contains(numerator, target) || contains(denominator, target),
        _ => false,
    }
}
fn split(e: &Expr, target: &Expr) -> Result<(Expr, Expr), String> {
    if e == target {
        return Ok((Expr::integer(1), Expr::integer(0)));
    }
    if !contains(e, target) {
        return Ok((Expr::integer(0), e.clone()));
    }
    match e {
        Expr::Add { terms } => {
            let parts = terms
                .iter()
                .map(|e| split(e, target))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                Expr::add(parts.iter().map(|p| p.0.clone()).collect()),
                Expr::add(parts.iter().map(|p| p.1.clone()).collect()),
            ))
        }
        Expr::Mul { factors } => {
            let mut dep = None;
            let mut others = vec![];
            for f in factors {
                if contains(f, target) {
                    if dep.is_some() {
                        return Err("非線形の係数です。".into());
                    }
                    dep = Some(split(f, target)?);
                } else {
                    others.push(f.clone());
                }
            }
            let (a, b) = dep.ok_or("依存する項がありません。")?;
            let factor = Expr::mul(others);
            Ok((
                Expr::mul(vec![factor.clone(), a]),
                Expr::mul(vec![factor, b]),
            ))
        }
        _ => Err("3項間の一次係数を検出できません。".into()),
    }
}
#[derive(Clone)]
pub enum Detected {
    Scaled {
        profile: u32,
        r: Rational,
        s: Rational,
        a: Rational,
        b: Rational,
        first: Rational,
        second: Rational,
    },
    Difference {
        profile: u32,
        r: Rational,
        amplitude: Rational,
        first: Rational,
        u: Rational,
        v: Rational,
    },
    Forced {
        r: Rational,
        s: Rational,
        a: Rational,
        b: Rational,
        shift: Rational,
        first: Rational,
        second: Rational,
    },
    Arithmetic {
        first: Rational,
        difference: Rational,
        increment: Rational,
    },
}
fn constant(p: &Poly) -> Option<Rational> {
    if p.is_empty() {
        Some(Rational::integer(0))
    } else if p.len() == 1 {
        Some(p[0].clone())
    } else {
        None
    }
}
fn proportional(p: &Poly, q: &Poly) -> Option<Rational> {
    let i = q.iter().position(|x| !x.is_zero())?;
    let ratio = p.get(i)?.div(&q[i]).ok()?;
    if *p == pscale(q, &ratio) {
        Some(ratio)
    } else {
        None
    }
}
fn roots(p: &Rational, q: &Rational) -> Option<(Rational, Rational)> {
    for r in 2..=5 {
        for s in r + 1..=5 {
            let rr = Rational::integer(r);
            let ss = Rational::integer(s);
            if rr.add(&ss) == *p && Rational::integer(-1).mul(&rr).mul(&ss) == *q {
                return Some((rr, ss));
            }
        }
    }
    None
}
fn amplitudes(
    first: &Rational,
    second: &Rational,
    r: &Rational,
    s: &Rational,
) -> Result<(Rational, Rational), String> {
    let a = second.sub(&s.mul(first)).div(&r.sub(s))?;
    let b = first.sub(&a);
    Ok((a, b))
}
pub fn detect(problem: &ProblemIR) -> Result<Option<Detected>, String> {
    if problem.kind != RecurrenceKind::SecondOrder {
        return Ok(None);
    }
    let (p, left_rest) = split(&problem.recurrence.lhs, &Expr::term("a", 2))?;
    if left_rest != Expr::integer(0) {
        return Ok(None);
    }
    let (q, rest) = split(&problem.recurrence.rhs, &Expr::term("a", 1))?;
    let (r, f) = split(&rest, &Expr::term("a", 0))?;
    let (pp, qq, rr, ff) = (poly(&p)?, poly(&q)?, poly(&r)?, poly(&f)?);
    let first = problem.initials[0]
        .rhs
        .evaluate(1, &|_, _| Err("初期条件が数値ではありません。".into()))?;
    let second = problem.initials[1]
        .rhs
        .evaluate(1, &|_, _| Err("初期条件が数値ではありません。".into()))?;
    if let (Some(pc), Some(qc), Some(rc), Some(fc)) =
        (constant(&pp), constant(&qq), constant(&rr), constant(&ff))
    {
        if !pc.is_zero() && !fc.is_zero() {
            let q = qc.div(&pc)?;
            let r = rc.div(&pc)?;
            let forcing = fc.div(&pc)?;
            if q == Rational::integer(2) && r == Rational::integer(-1) {
                return Ok(Some(Detected::Arithmetic {
                    first: first.clone(),
                    difference: second.sub(&first),
                    increment: forcing,
                }));
            }
            if let Some((r, s)) = roots(&q, &r) {
                let shift = forcing.div(&Rational::integer(1).sub(&q).sub(&rc.div(&pc)?))?;
                let bf = first.sub(&shift);
                let bs = second.sub(&shift);
                let (a, b) = amplitudes(&bf, &bs, &r, &s)?;
                if !a.is_zero() && !b.is_zero() {
                    return Ok(Some(Detected::Forced {
                        r,
                        s,
                        a,
                        b,
                        shift,
                        first: bf,
                        second: bs,
                    }));
                }
            }
        }
        return Ok(None);
    }
    if !ff.is_empty() {
        return Ok(None);
    }
    // Difference is detected before normalization: the displayed coefficients
    // sum to zero and the first-order difference has a registered scale ratio.
    if pp == padd(&qq, &rr) {
        for profile in 0..=2 {
            let g = poly(&scale(profile, 0))?;
            let g1 = poly(&scale(profile, 1))?;
            if let Some(k) = proportional(&pp, &g) {
                let negative = pscale(&rr, &Rational::integer(-1));
                if let Some(ratio) = proportional(&negative, &pscale(&g1, &k)) {
                    if ratio.integer_u32().is_some_and(|r| (2..=5).contains(&r)) {
                        let gfirst = scale(profile, 0)
                            .evaluate(1, &|_, _| Err("unexpected sequence".into()))?;
                        let amplitude = second.sub(&first).div(&gfirst)?;
                        let (slope, intercept) = if profile == 2 {
                            (Rational::integer(2), Rational::integer(1))
                        } else {
                            (Rational::integer(1), Rational::integer(profile as i64))
                        };
                        let den = ratio.sub(&Rational::integer(1));
                        let u = amplitude.mul(&slope).div(&den)?;
                        let v = amplitude.mul(&intercept).sub(&ratio.mul(&u)).div(&den)?;
                        if !amplitude.is_zero() && first != u.add(&v) {
                            return Ok(Some(Detected::Difference {
                                profile,
                                r: ratio,
                                amplitude,
                                first,
                                u,
                                v,
                            }));
                        }
                    }
                }
            }
        }
    }
    for profile in 0..=2 {
        let g = poly(&scale(profile, 0))?;
        let g1 = poly(&scale(profile, 1))?;
        let g2 = poly(&scale(profile, 2))?;
        if let Some(k) = proportional(&pp, &pmul(&g, &g1)) {
            if let (Some(q), Some(r)) = (
                proportional(&qq, &pscale(&pmul(&g, &g2), &k)),
                proportional(&rr, &pscale(&pmul(&g1, &g2), &k)),
            ) {
                if let Some((r, s)) = roots(&q, &r) {
                    let bfirst = first.div(
                        &scale(profile, 0)
                            .evaluate(1, &|_, _| Err("unexpected sequence".into()))?,
                    )?;
                    let bsecond = second.div(
                        &scale(profile, 0)
                            .evaluate(2, &|_, _| Err("unexpected sequence".into()))?,
                    )?;
                    let (a, b) = amplitudes(&bfirst, &bsecond, &r, &s)?;
                    if !a.is_zero() && !b.is_zero() {
                        return Ok(Some(Detected::Scaled {
                            profile,
                            r,
                            s,
                            a,
                            b,
                            first: bfirst,
                            second: bsecond,
                        }));
                    }
                }
            }
        }
    }
    Ok(None)
}
fn dstep(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn seq_initial(name: &str, value: &Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: name.into(),
            index: NatExpr::constant(1),
        },
        rat(value),
    )
}
fn auxiliary(
    name: &str,
    definition: Expr,
    value: &Rational,
    recurrence: Expr,
) -> AuxiliarySequence {
    AuxiliarySequence {
        name: name.into(),
        definition: Equation::new(Expr::term(name, 0), definition),
        initial: seq_initial(name, value),
        recurrence: Equation::new(Expr::term(name, 1), recurrence),
    }
}
fn roots_steps(
    name: &str,
    r: &Rational,
    s: &Rational,
    a: &Rational,
    b: &Rational,
    first: &Rational,
    second: &Rational,
) -> Vec<DerivationStep> {
    let lambda = Expr::Term {
        sequence: "λ".into(),
        index: NatExpr::constant(1),
    };
    let aa = Expr::Term {
        sequence: "A".into(),
        index: NatExpr::constant(1),
    };
    let bb = Expr::Term {
        sequence: "B".into(),
        index: NatExpr::constant(1),
    };
    vec![
        dstep(
            &format!(
                "特性方程式は (λ−{}) (λ−{})=0 なので、異なる2根は {} と {} である。",
                r, s, r, s
            ),
            Some(Equation::new(
                Expr::mul(vec![
                    Expr::sub(lambda.clone(), rat(r)),
                    Expr::sub(lambda, rat(s)),
                ]),
                Expr::integer(0),
            )),
            "3項間の定数係数から特性方程式を作る",
        ),
        dstep(
            "2つの等比数列の一次結合として一般項を表し、初期値から係数AとBを決める。",
            Some(Equation::new(
                Expr::add(vec![aa.clone(), bb.clone()]),
                rat(first),
            )),
            "第1項の条件",
        ),
        dstep(
            "第2項も代入して、この2本の一次方程式を解く。",
            Some(Equation::new(
                Expr::add(vec![
                    Expr::mul(vec![rat(r), aa]),
                    Expr::mul(vec![rat(s), bb]),
                ]),
                rat(second),
            )),
            "異なる2根なので係数は一意に決まる",
        ),
        dstep(
            &format!(
                "A={}、B={} を代入すると、補助数列の一般項が得られる。",
                a, b
            ),
            Some(Equation::new(
                Expr::term(name, 0),
                Expr::add(vec![geom(a, r), geom(b, s)]),
            )),
            "定数係数の3項間漸化式の一般解",
        ),
    ]
}
pub fn derivation(problem: &ProblemIR, route: &Detected) -> DerivationIR {
    let mut auxiliaries = vec![];
    let (method, hint, mut steps) = match route {
        Detected::Scaled {
            profile,
            r,
            s,
            a,
            b,
            first,
            second,
        } => {
            let g = scale(*profile, 0);
            let normalized = Expr::div(Expr::term("a", 0), g);
            let rec = Expr::add(vec![
                Expr::mul(vec![rat(&r.add(s)), Expr::term("b", 1)]),
                Expr::mul(vec![
                    rat(&Rational::integer(-1).mul(r).mul(s)),
                    Expr::term("b", 0),
                ]),
            ]);
            let definition = Equation::new(Expr::term("b", 0), normalized.clone());
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: definition.clone(),
                initial: seq_initial("b", first),
                recurrence: Equation::new(Expr::term("b", 2), rec.clone()),
            });
            let mut steps = vec![
                dstep(
                    "3つの係数を同じ倍率のn、n+1、n+2で比較し、各項をその倍率で割る。",
                    Some(definition),
                    "倍率はn≥1で正なので除算できる",
                ),
                dstep(
                    "与式に補助数列を代入し共通因子を約分すると、定数係数の3項間漸化式になる。",
                    Some(Equation::new(Expr::term("b", 2), rec)),
                    "初期値も同じ倍率で割る",
                ),
            ];
            steps.extend(roots_steps("b", r, s, a, b, first, second));
            (
                "正規化して特性多項式を使う",
                "各項の係数を、同じ倍率のn・n+1・n+2の値として比較する。",
                steps,
            )
        }
        Detected::Forced {
            r,
            s,
            a,
            b,
            shift,
            first,
            second,
        } => {
            let def = Expr::sub(Expr::term("a", 0), rat(shift));
            let recurrence = Expr::add(vec![
                Expr::mul(vec![rat(&r.add(s)), Expr::term("b", 1)]),
                Expr::mul(vec![
                    rat(&Rational::integer(-1).mul(r).mul(s)),
                    Expr::term("b", 0),
                ]),
            ]);
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: Equation::new(Expr::term("b", 0), def.clone()),
                initial: seq_initial("b", first),
                recurrence: Equation::new(Expr::term("b", 2), recurrence.clone()),
            });
            let mut steps = vec![
                dstep(
                    &format!(
                        "一定値を与式に代入すると x={} を得る。この値を引いて定数項を消す。",
                        shift
                    ),
                    Some(Equation::new(Expr::term("b", 0), def)),
                    "h=F/(1−p−q)を求める",
                ),
                dstep(
                    "置換した補助数列は斉次の3項間漸化式を満たす。初期値も一定値を引いて求める。",
                    Some(Equation::new(Expr::term("b", 2), recurrence)),
                    "定数項が相殺される",
                ),
            ];
            steps.extend(roots_steps("b", r, s, a, b, first, second));
            (
                "定数項を消して3項間を解く",
                "一定の値を引いて定数項を消し、斉次の3項間漸化式に帰着させる。",
                steps,
            )
        }
        Detected::Arithmetic {
            first,
            difference,
            increment,
        } => {
            let def = Expr::sub(Expr::term("a", 1), Expr::term("a", 0));
            let rec = Expr::add(vec![Expr::term("b", 0), rat(increment)]);
            auxiliaries.push(auxiliary("b", def.clone(), difference, rec.clone()));
            let bformula = Expr::add(vec![
                rat(difference),
                Expr::mul(vec![rat(increment), Expr::offset(-1)]),
            ]);
            ("階差を等差数列として解く","隣接する項の差でまとめると、階差数列が等差数列になる。",vec![dstep("与式を隣接する項の差でまとめ、階差数列を定義する。",Some(Equation::new(Expr::term("b",0),def)),"a_{n+2}−a_{n+1}=(a_{n+1}−a_n)+c"),dstep("初階差は第2項から初項を引いた値で、階差数列の公差は一定である。",Some(Equation::new(Expr::term("b",1),rec)),"初期条件と与式"),dstep("したがって階差数列の一般項は等差数列の公式で求められる。",Some(Equation::new(Expr::term("b",0),bformula)),"初階差からn−1回だけ公差を加える"),dstep(&format!("n≥2では初項 {} に第1階差から第n−1階差までを足す。等差数列の和により (n−1)d+c(n−1)(n−2)/2 となる。",first),None,"a_n=a_1+Σ_{k=1}^{n−1}b_k")])
        }
        Detected::Difference {
            profile,
            r,
            amplitude,
            first,
            u,
            v,
        } => {
            let g = scale(*profile, 0);
            let g1 = scale(*profile, 1);
            let def = Expr::sub(Expr::term("a", 1), Expr::term("a", 0));
            let rec = Expr::mul(vec![rat(r), Expr::div(g1, g.clone()), Expr::term("b", 0)]);
            let bfirst = amplitude.mul(
                &g.evaluate(1, &|_, _| Err("unexpected sequence".into()))
                    .expect("scale"),
            );
            auxiliaries.push(auxiliary("b", def.clone(), &bfirst, rec.clone()));
            auxiliaries.push(auxiliary(
                "c",
                Expr::div(Expr::term("b", 0), g.clone()),
                amplitude,
                Expr::mul(vec![rat(r), Expr::term("c", 0)]),
            ));
            let h = linear(u.clone(), v.clone());
            let hnext = Expr::add(vec![Expr::mul(vec![rat(u), Expr::offset(1)]), rat(v)]);
            ("階差を正規化し、和の相殺で戻す","3つの係数の和が0であることを使い、隣接する項の差を新しい数列にする。",vec![dstep("次々項・次項・現在の項の係数を符号込みで足すと零なので、階差でまとめられる。",Some(Equation::new(Expr::term("b",0),def)),"P=Q+Rを係数から確認"),dstep("階差数列は添字を含む階比の漸化式を満たす。",Some(Equation::new(Expr::term("b",1),rec)),"隣接する2項の差を整理"),dstep("階差を倍率で割ると等比数列になる。",Some(Equation::new(Expr::term("c",0),Expr::div(Expr::term("b",0),g.clone()))),"n≥1で倍率は正"),dstep("等比数列の一般項から元の階差を求める。",Some(Equation::new(Expr::term("b",0),Expr::mul(vec![g.clone(),geom(amplitude,r)]))),"c_n=c_1 r^{n−1}"),dstep("一次式H(n)を選び、階差を隣接する2つの式の差として表す。",Some(Equation::new(Expr::sub(Expr::mul(vec![rat(r),hnext]),h),Expr::mul(vec![rat(amplitude),g]))),"係数を比較してrH(n+1)−H(n)=d g(n)を満たすHを求める"),dstep(&format!("b_k=H(k+1)r^k−H(k)r^(k−1) をk=1からn−1まで足すと中間項が相殺される。初項 {} を加え、a_n=a_1+H(n)r^(n−1)−H(1) となる。",first),None,"有限和の相殺を全添字で用いる")])
        }
    };
    steps.push(dstep(
        "補助数列を元の数列に戻すと一般項を得る。n=1、n=2でも2つの初期条件と一致する。",
        Some(problem.general_term.clone()),
        "置換を戻し、初期条件とすべての添字での漸化式を確認",
    ));
    DerivationIR {
        method_name: method.into(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    }
}

pub fn classify(problem: &ProblemIR) -> Result<Option<crate::generator::Classification>, String> {
    let Some(route) = detect(problem)? else {
        return Ok(None);
    };
    let intermediate = match &route {
        Detected::Scaled {
            r,
            s,
            a,
            b,
            first,
            second,
            ..
        } => vec![
            r.add(s),
            Rational::integer(-1).mul(r).mul(s),
            r.clone(),
            s.clone(),
            a.clone(),
            b.clone(),
            first.clone(),
            second.clone(),
        ],
        Detected::Difference {
            profile,
            r,
            amplitude,
            first,
            u,
            v,
        } => vec![
            first.clone(),
            first.add(
                &amplitude
                    .mul(&scale(*profile, 0).evaluate(1, &|_, _| Err("unexpected term".into()))?),
            ),
            amplitude.clone(),
            r.clone(),
            first.sub(&u.add(v)),
            u.clone(),
            v.clone(),
        ],
        Detected::Forced { shift, .. } => vec![shift.clone()],
        Detected::Arithmetic {
            first,
            difference,
            increment,
        } => vec![first.clone(), first.add(difference), increment.clone()],
    };
    let (score, method, reason, operations, discovery, degree) = match &route {
        Detected::Scaled { .. } => (
            14,
            "正規化して特性多項式を使う",
            "正規化して3項間を解く",
            vec!["index_scale", "characteristic_distinct"],
            2,
            2,
        ),
        Detected::Difference { .. } => (
            20,
            "階差を正規化し、和の相殺で戻す",
            "階差・階比・多項式と等比の和を組み合わせる",
            vec!["difference", "index_scale", "geometric", "geometric_sum"],
            3,
            1,
        ),
        Detected::Forced { .. } => (
            14,
            "定数項を消して3項間を解く",
            "定数項を消して3項間漸化式を解く",
            vec!["shift", "characteristic_distinct"],
            2,
            0,
        ),
        Detected::Arithmetic { .. } => (
            14,
            "階差を等差数列として解く",
            "階差を等差数列として解き、和で戻す",
            vec!["difference", "difference_sum", "evaluate_sum"],
            1,
            0,
        ),
    };
    let classification = crate::generator::make_classification(
        problem,
        score,
        method,
        reason,
        &operations,
        discovery,
        0,
        degree,
    )?;
    Ok(Some(crate::generator::include_intermediate_values(
        problem,
        classification,
        &intermediate,
    )?))
}
pub fn explain(
    problem: &ProblemIR,
    classification: &crate::generator::Classification,
) -> Result<Option<DerivationIR>, String> {
    if !matches!(
        classification.method_name.as_str(),
        "正規化して特性多項式を使う"
            | "階差を正規化し、和の相殺で戻す"
            | "定数項を消して3項間を解く"
            | "階差を等差数列として解く"
    ) {
        return Ok(None);
    }
    Ok(detect(problem)?.map(|route| derivation(problem, &route)))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(family: &str, profile: u32) -> Recipe {
        let mut p = match family {
            "arithmetic_difference" => map(&[
                ("c", Rational::integer(2)),
                ("d", Rational::integer(1)),
                ("s", Rational::integer(3)),
            ]),
            "difference_scaled" => map(&[
                ("c", Rational::integer(2)),
                ("d", Rational::integer(1)),
                ("r", Rational::integer(2)),
            ]),
            _ => map(&[
                ("c", Rational::integer(2)),
                ("d", Rational::integer(1)),
                ("r", Rational::integer(2)),
                ("s", Rational::integer(3)),
            ]),
        };
        if matches!(family, "difference_scaled" | "scaled_second_order") {
            p.insert("profile".into(), Rational::integer(profile as i64));
            p.insert("profile_version".into(), Rational::integer(1));
        }
        build_recipe(family, 17, p).unwrap()
    }
    #[test]
    fn reference_second_routes_replay_and_classify_without_the_answer() {
        for family in catalog() {
            for profile in if family.id.ends_with("scaled") || family.id == "scaled_second_order" {
                0..=2
            } else {
                0..=0
            } {
                let generated = crate::generator::replay(&fixture(&family.id, profile)).unwrap();
                assert_eq!(generated.classification.level, family.levels[0]);
                assert_eq!(
                    generated.classification.score,
                    if family.id == "difference_scaled" {
                        20
                    } else {
                        14
                    }
                );
                let mut hidden = generated.problem.clone();
                hidden.general_term.rhs = Expr::integer(999);
                let classified = classify(&hidden).unwrap().unwrap();
                assert_eq!(classified.method_name, generated.classification.method_name);
                assert!(!generated.derivation.auxiliaries.is_empty());
            }
        }
    }
    #[test]
    fn reference_odd_scale_and_second_initials_are_exact() {
        let scaled = compile(&fixture("scaled_second_order", 2)).unwrap();
        assert_eq!(scaled.initials[0].rhs, Expr::integer(9));
        assert_eq!(scaled.initials[1].rhs, Expr::integer(40));
        let diff = compile(&fixture("difference_scaled", 2)).unwrap();
        assert_eq!(diff.initials[0].rhs, Expr::integer(2));
        assert_eq!(diff.initials[1].rhs, Expr::integer(5));
    }
    #[test]
    fn changed_recurrence_cannot_select_the_recipe_method() {
        let mut p = compile(&fixture("scaled_second_order", 0)).unwrap();
        p.recurrence.rhs = Expr::integer(7);
        assert!(detect(&p).unwrap().is_none());
    }
}
