//! First-order reference families. Detection reads the statement, never the recipe.
use crate::generator::*;
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_traits::ToPrimitive;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet};

pub fn catalog() -> Vec<FamilyInfo> {
    [
        ("affine", "不動点を引く2項間", vec![2]),
        ("shifted_scaled", "階比と定数の移動（12倍率）", vec![3, 4]),
        ("scaled_affine", "階比と不動点（9倍率）", vec![4]),
        ("reciprocal_scaled", "逆数と階比（9倍率）", vec![5]),
        ("polynomial_forcing", "一次式の付加項", vec![3]),
        ("reciprocal_forcing", "逆数と一次式の付加項", vec![5]),
        (
            "polynomial_difference",
            "多項式の階差（6多項式）",
            vec![1, 2],
        ),
        ("mobius", "不動点の差を逆数にする1次分数", vec![4]),
    ]
    .into_iter()
    .map(|(id, label, levels)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels,
    })
    .collect()
}

pub fn supports(family: &str) -> bool {
    catalog().iter().any(|f| f.id == family)
}
fn params(items: &[(&str, Rational)]) -> BTreeMap<String, Rational> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
pub fn sample(
    family: &str,
    level: u8,
    rng: &mut ChaCha8Rng,
) -> Result<BTreeMap<String, Rational>, String> {
    let c = Rational::integer(rng.gen_range(1..=3));
    let d = Rational::integer(rng.gen_range((2 * c.integer_u32().unwrap() + 1).max(4)..=9) as i64);
    let r = Rational::integer(rng.gen_range(2..=4));
    let profiles: &[i64] = if family == "shifted_scaled" && level == 3 {
        &[0, 2, 8]
    } else if family == "shifted_scaled" {
        &[1, 3, 4, 5, 6, 7, 9, 10, 11]
    } else {
        &[0, 1, 5, 6, 7, 8, 9, 10, 11]
    };
    let profile = Rational::integer(profiles[rng.gen_range(0..profiles.len())]);
    Ok(match family {
        "affine" | "polynomial_forcing" | "reciprocal_forcing" => {
            params(&[("c", c), ("d", d), ("r", r)])
        }
        "shifted_scaled" => params(&[("c", c), ("d", d), ("profile", profile)]),
        "scaled_affine" | "reciprocal_scaled" => {
            params(&[("c", c), ("d", d), ("r", r), ("profile", profile)])
        }
        "polynomial_difference" => params(&[
            ("c", c),
            ("d", d),
            (
                "profile",
                Rational::integer(if level == 1 {
                    [0, 1, 5][rng.gen_range(0..3)]
                } else {
                    [2, 3, 4][rng.gen_range(0..3)]
                }),
            ),
        ]),
        "mobius" => params(&[
            ("c", c),
            ("d", d),
            ("r", r),
            ("h", Rational::integer(rng.gen_range(2..=3))),
        ]),
        _ => return Err("未登録の1次漸化式です。".into()),
    })
}
pub fn validate_parameters(family: &str, p: &BTreeMap<String, Rational>) -> Result<(), String> {
    let names: &[&str] = match family {
        "affine" | "polynomial_forcing" | "reciprocal_forcing" => &["c", "d", "r"],
        "shifted_scaled" => &["c", "d", "profile"],
        "scaled_affine" | "reciprocal_scaled" => &["c", "d", "r", "profile"],
        "polynomial_difference" => &["c", "d", "profile"],
        "mobius" => &["c", "d", "r", "h"],
        _ => return Err("未登録の1次漸化式です。".into()),
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.iter().copied().collect() {
        return Err("パラメータ集合が登録規則と一致しません。".into());
    }
    for (name, value) in p {
        if name == "profile" {
            if value.integer_u32().is_none_or(|x| {
                x >= if family == "polynomial_difference" {
                    6
                } else {
                    12
                }
            }) {
                return Err("倍率プロファイルが範囲外です。".into());
            }
        } else if !value.is_positive() || value.0 > Rational::integer(16).0 {
            return Err("係数は正の有理数で16以下です。".into());
        }
    }
    if let Some(r) = p.get("r") {
        if r.integer_u32().is_none_or(|x| !(2..=5).contains(&x)) {
            return Err("公比は2から5の整数です。".into());
        }
    }
    if matches!(family, "scaled_affine" | "reciprocal_scaled")
        && matches!(p["profile"].integer_u32(), Some(2..=4))
    {
        return Err("この倍率の付加項は係数次数の上限を超えます。".into());
    }
    if matches!(family, "polynomial_forcing" | "reciprocal_forcing") {
        let first = p["d"]
            .sub(&p["c"])
            .sub(&p["c"].div(&p["r"].sub(&Rational::integer(1)))?);
        if !first.is_positive() {
            return Err("一次付加項型の初項が正ではありません。".into());
        }
    }
    if family == "mobius" {
        let force = Rational::integer(1).sub(&p["r"]).mul(&p["c"]);
        let m0 = Rational::integer(1).add(&p["h"].mul(&force));
        let m1 = p["h"]
            .mul(&p["r"].sub(&Rational::integer(1)))
            .sub(&p["h"].pow(2).mul(&force));
        if m0.is_zero() || m1.is_zero() {
            return Err("1次分数の分子が退化します。".into());
        }
    }
    Ok(())
}
pub fn build_recipe(
    family: &str,
    seed: u64,
    p: BTreeMap<String, Rational>,
) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    let core_kind = if matches!(family, "shifted_scaled" | "polynomial_difference") {
        "constant"
    } else if matches!(family, "polynomial_forcing" | "reciprocal_forcing") {
        "geometric"
    } else {
        "affine_fixed_point"
    };
    let core_params = if core_kind == "constant" {
        params(&[("initial", p["d"].clone())])
    } else if core_kind == "geometric" {
        params(&[("amplitude", p["d"].clone()), ("ratio", p["r"].clone())])
    } else {
        params(&[
            ("amplitude", p["d"].clone()),
            ("ratio", p["r"].clone()),
            ("fixed_point", p["c"].clone()),
            (
                "constant_term",
                Rational::integer(1).sub(&p["r"]).mul(&p["c"]),
            ),
        ])
    };
    let mut blocks = Vec::new();
    let mut append = |kind: &str, parameters: BTreeMap<String, Rational>| {
        let id = format!("b{}", blocks.len() + 1);
        let input = blocks
            .last()
            .map(|b: &RecipeBlock| b.id.clone())
            .unwrap_or_else(|| "core".into());
        blocks.push(RecipeBlock {
            id,
            input,
            kind: kind.into(),
            parameters,
        });
    };
    if matches!(
        family,
        "shifted_scaled" | "scaled_affine" | "reciprocal_scaled"
    ) {
        append("index_scale", params(&[("profile", p["profile"].clone())]));
    }
    if family == "shifted_scaled" {
        append(
            "add_constant",
            params(&[("value", Rational::integer(-1).mul(&p["c"]))]),
        );
    }
    if matches!(family, "polynomial_forcing" | "reciprocal_forcing") {
        append(
            "index_add",
            params(&[
                ("slope", Rational::integer(-1).mul(&p["c"])),
                (
                    "intercept",
                    Rational::integer(-1)
                        .mul(&p["c"])
                        .div(&p["r"].sub(&Rational::integer(1)))?,
                ),
            ]),
        );
    }
    if family == "polynomial_difference" {
        append(
            "difference_polynomial",
            params(&[
                ("coefficient", p["c"].clone()),
                ("profile", p["profile"].clone()),
            ]),
        );
    }
    if matches!(family, "reciprocal_scaled" | "reciprocal_forcing") {
        append("reciprocal", BTreeMap::new());
    }
    if family == "mobius" {
        append("mobius_encode", params(&[("shift", p["h"].clone())]));
    }
    let output = blocks
        .last()
        .map(|b| b.id.clone())
        .unwrap_or_else(|| "core".into());
    Ok(Recipe {
        schema_version: SCHEMA_VERSION.into(),
        rules_version: EXTENSION_RULES_VERSION.into(),
        rng_version: RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p,
        core: RecipeCore {
            id: "core".into(),
            kind: core_kind.into(),
            parameters: core_params,
        },
        blocks,
        output,
        index_start: 1,
    })
}

/// Stable codes: n,n+1,n²,n(n+1),n(n+2),2n+1, then their reciprocals.
pub fn scale(profile: u32, offset: i32) -> Expr {
    let n = Expr::offset(offset);
    let direct = match profile % 6 {
        0 => n,
        1 => Expr::add(vec![n, Expr::integer(1)]),
        2 => Expr::pow(n, NatExpr::constant(2)),
        3 => Expr::mul(vec![n.clone(), Expr::add(vec![n, Expr::integer(1)])]),
        4 => Expr::mul(vec![n.clone(), Expr::add(vec![n, Expr::integer(2)])]),
        _ => Expr::add(vec![Expr::mul(vec![Expr::integer(2), n]), Expr::integer(1)]),
    };
    if profile >= 6 {
        Expr::div(Expr::integer(1), direct)
    } else {
        direct
    }
}
fn nz(expression: Expr, text: &str) -> DomainCondition {
    DomainCondition {
        kind: "nonzero".into(),
        expression,
        explanation: text.into(),
    }
}
fn initial(value: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: "a".into(),
            index: NatExpr::constant(1),
        },
        Expr::rational(value),
    )
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    let p = &recipe.parameters;
    let c = Expr::rational(p["c"].clone());
    let d = Expr::rational(p["d"].clone());
    let a = Expr::term("a", 0);
    let mut conditions = Vec::new();
    let (rhs, formula) = match recipe.family.as_str() {
        "polynomial_difference" => {
            let profile = p["profile"].integer_u32().unwrap();
            let nm1 = Expr::offset(-1);
            let n = Expr::index();
            let prefix = match profile {
                0 => Expr::mul(vec![Expr::rational(Rational::fraction(1, 2)), nm1, n]),
                1 => Expr::mul(vec![
                    Expr::rational(Rational::fraction(1, 2)),
                    nm1,
                    Expr::offset(2),
                ]),
                2 => Expr::mul(vec![
                    Expr::rational(Rational::fraction(1, 6)),
                    nm1,
                    n.clone(),
                    Expr::sub(Expr::mul(vec![Expr::integer(2), n]), Expr::integer(1)),
                ]),
                3 => Expr::mul(vec![
                    Expr::rational(Rational::fraction(1, 3)),
                    nm1,
                    n,
                    Expr::offset(1),
                ]),
                4 => Expr::mul(vec![
                    Expr::rational(Rational::fraction(1, 6)),
                    nm1,
                    n.clone(),
                    Expr::add(vec![Expr::mul(vec![Expr::integer(2), n]), Expr::integer(5)]),
                ]),
                _ => Expr::mul(vec![nm1, Expr::offset(1)]),
            };
            (
                Expr::add(vec![a, Expr::mul(vec![c.clone(), scale(profile, 0)])]),
                Expr::add(vec![d, Expr::mul(vec![c, prefix])]),
            )
        }
        "shifted_scaled" => {
            let profile = p["profile"].integer_u32().unwrap();
            let g = scale(profile, 0);
            let gn = scale(profile, 1);
            conditions.push(nz(
                scale(profile % 6, 0),
                "nは1以上なので、倍率の多項式は正であり零にならない。",
            ));
            let ratio = if profile >= 6 {
                Expr::div(scale(profile % 6, 0), scale(profile % 6, 1))
            } else {
                Expr::div(gn, g.clone())
            };
            (
                Expr::add(vec![
                    Expr::mul(vec![ratio.clone(), a]),
                    Expr::mul(vec![c.clone(), Expr::sub(ratio, Expr::integer(1))]),
                ]),
                Expr::sub(Expr::mul(vec![d, g]), c),
            )
        }
        "polynomial_forcing" | "reciprocal_forcing" => {
            let r = Expr::rational(p["r"].clone());
            let forcing = Expr::mul(vec![
                Expr::rational(p["c"].mul(&p["r"].sub(&Rational::integer(1)))),
                Expr::index(),
            ]);
            let base = Expr::sub(
                Expr::sub(
                    Expr::mul(vec![d, Expr::pow(r.clone(), NatExpr::offset(-1))]),
                    Expr::mul(vec![c, Expr::index()]),
                ),
                Expr::rational(p["c"].div(&p["r"].sub(&Rational::integer(1)))?),
            );
            if recipe.family == "polynomial_forcing" {
                (Expr::add(vec![Expr::mul(vec![r, a]), forcing]), base)
            } else {
                let den = Expr::add(vec![r, Expr::mul(vec![forcing, a.clone()])]);
                conditions.push(nz(
                    den.clone(),
                    "逆数で得た数列は正の初項・正の係数と付加項をもち、元の分母は正となる。",
                ));
                conditions.push(nz(
                    a.clone(),
                    "逆数の数列が全項で正なので、各項は零にならない。",
                ));
                (Expr::div(a, den), Expr::div(Expr::integer(1), base))
            }
        }
        "affine" | "scaled_affine" | "reciprocal_scaled" | "mobius" => {
            let r = Expr::rational(p["r"].clone());
            let force = Rational::integer(1).sub(&p["r"]).mul(&p["c"]);
            let base = Expr::add(vec![
                c,
                Expr::mul(vec![d, Expr::pow(r.clone(), NatExpr::offset(-1))]),
            ]);
            if recipe.family == "affine" {
                (
                    Expr::add(vec![Expr::mul(vec![r, a]), Expr::rational(force)]),
                    base,
                )
            } else if recipe.family == "mobius" {
                let h = p["h"].clone();
                let m0 = Rational::integer(1).add(&h.mul(&force));
                let m1 = h
                    .mul(&p["r"].sub(&Rational::integer(1)))
                    .sub(&h.pow(2).mul(&force));
                let m3 = p["r"].sub(&h.mul(&force));
                let den = Expr::add(vec![
                    Expr::mul(vec![Expr::rational(force), a.clone()]),
                    Expr::rational(m3),
                ]);
                conditions.push(nz(
                    den.clone(),
                    "不動点からの差の逆数をv_nとおくと、元の分母はv_{n+1}/v_n>0となる。",
                ));
                conditions.push(nz(
                    Expr::sub(a.clone(), Expr::rational(h.clone())),
                    "a_n-h=1/v_nであり、v_nは全項で正なので零にならない。",
                ));
                (
                    Expr::div(
                        Expr::add(vec![
                            Expr::mul(vec![Expr::rational(m0), a]),
                            Expr::rational(m1),
                        ]),
                        den,
                    ),
                    Expr::add(vec![Expr::rational(h), Expr::div(Expr::integer(1), base)]),
                )
            } else {
                let profile = p["profile"].integer_u32().unwrap();
                let g = scale(profile, 0);
                let gn = scale(profile, 1);
                conditions.push(nz(
                    scale(profile % 6, 0),
                    "nは1以上なので、倍率の多項式は正であり零にならない。",
                ));
                let ratio = if profile >= 6 {
                    Expr::div(scale(profile % 6, 0), scale(profile % 6, 1))
                } else {
                    Expr::div(gn.clone(), g.clone())
                };
                let coef = Expr::mul(vec![r, ratio]);
                let forcing = Expr::mul(vec![Expr::rational(force), gn]);
                let base = Expr::mul(vec![g, base]);
                if recipe.family == "scaled_affine" {
                    (Expr::add(vec![Expr::mul(vec![coef, a]), forcing]), base)
                } else {
                    let den = Expr::add(vec![coef, Expr::mul(vec![forcing, a.clone()])]);
                    conditions.push(nz(
                        den.clone(),
                        "正の補助数列v_n=1/a_nから、元の分母はv_{n+1}/v_n>0である。",
                    ));
                    conditions.push(nz(
                        a.clone(),
                        "正の補助数列の逆数なので、各項は零にならない。",
                    ));
                    (Expr::div(a, den), Expr::div(Expr::integer(1), base))
                }
            }
        }
        _ => return Err("未登録の1次漸化式です。".into()),
    };
    let first = formula.evaluate(1, &|_, _| Err("一般項に数列の項はありません。".into()))?;
    Ok(ProblemIR {
        kind: RecurrenceKind::FirstOrder,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(Expr::term("a", 1), rhs),
        initials: vec![initial(first)],
        secondary_recurrences: vec![],
        secondary_general_terms: vec![],
        general_term: Equation::new(Expr::term("a", 0), formula),
        conditions,
    })
}

type Poly = BTreeMap<u32, Rational>;
fn pc(value: Rational) -> Poly {
    if value.is_zero() {
        BTreeMap::new()
    } else {
        BTreeMap::from([(0, value)])
    }
}
fn pa(a: &Poly, b: &Poly) -> Poly {
    let mut out = a.clone();
    for (k, v) in b {
        let x = out
            .get(k)
            .cloned()
            .unwrap_or_else(|| Rational::integer(0))
            .add(v);
        if x.is_zero() {
            out.remove(k);
        } else {
            out.insert(*k, x);
        }
    }
    out
}
fn pm(a: &Poly, b: &Poly) -> Poly {
    let mut out = Poly::new();
    for (i, x) in a {
        for (j, y) in b {
            out = pa(&out, &BTreeMap::from([(*i + *j, x.mul(y))]));
        }
    }
    out
}
fn pp(a: &Poly, n: u32) -> Poly {
    (0..n).fold(pc(Rational::integer(1)), |p, _| pm(&p, a))
}
fn proportional(a: &Poly, b: &Poly) -> Option<Rational> {
    let (k, v) = b.last_key_value()?;
    let ratio = a
        .get(k)
        .cloned()
        .unwrap_or_else(|| Rational::integer(0))
        .div(v)
        .ok()?;
    let scaled: Poly = b
        .iter()
        .filter_map(|(d, c)| {
            let x = c.mul(&ratio);
            if x.is_zero() {
                None
            } else {
                Some((*d, x))
            }
        })
        .collect();
    (*a == scaled).then_some(ratio)
}
#[derive(Clone)]
struct RF {
    num: Poly,
    den: Poly,
}
impl RF {
    fn literal(v: Rational) -> Self {
        Self {
            num: pc(v),
            den: pc(Rational::integer(1)),
        }
    }
    fn add(&self, x: &Self) -> Self {
        Self {
            num: pa(&pm(&self.num, &x.den), &pm(&x.num, &self.den)),
            den: pm(&self.den, &x.den),
        }
    }
    fn neg(&self) -> Self {
        Self {
            num: self
                .num
                .iter()
                .map(|(d, c)| (*d, Rational::integer(-1).mul(c)))
                .collect(),
            den: self.den.clone(),
        }
    }
    fn sub(&self, x: &Self) -> Self {
        self.add(&x.neg())
    }
    fn mul(&self, x: &Self) -> Self {
        Self {
            num: pm(&self.num, &x.num),
            den: pm(&self.den, &x.den),
        }
    }
    fn div(&self, x: &Self) -> Option<Self> {
        if x.num.is_empty() {
            None
        } else {
            Some(Self {
                num: pm(&self.num, &x.den),
                den: pm(&self.den, &x.num),
            })
        }
    }
    fn scalar(&self) -> Option<Rational> {
        proportional(&self.num, &self.den)
    }
    fn poly(&self) -> Option<Poly> {
        if self.den.len() == 1 && self.den.contains_key(&0) {
            let d = &self.den[&0];
            Some(
                self.num
                    .iter()
                    .map(|(k, v)| Ok((*k, v.div(d)?)))
                    .collect::<Result<_, String>>()
                    .ok()?,
            )
        } else {
            None
        }
    }
}
fn rf(expr: &Expr) -> Option<RF> {
    Some(match expr {
        Expr::Rational { value } => RF::literal(value.clone()),
        Expr::NatCast { value } => {
            let off = match value {
                NatExpr::Index => 0,
                NatExpr::Add { left, right } if **left == NatExpr::Index => {
                    let NatExpr::Constant { value } = **right else {
                        return None;
                    };
                    value as i64
                }
                NatExpr::Sub { left, right } if **left == NatExpr::Index => {
                    let NatExpr::Constant { value } = **right else {
                        return None;
                    };
                    -(value as i64)
                }
                NatExpr::Constant { value } => {
                    return Some(RF::literal(Rational::integer(*value as i64)))
                }
                _ => return None,
            };
            RF {
                num: pa(
                    &BTreeMap::from([(1, Rational::integer(1))]),
                    &pc(Rational::integer(off)),
                ),
                den: pc(Rational::integer(1)),
            }
        }
        Expr::Add { terms } => terms
            .iter()
            .try_fold(RF::literal(Rational::integer(0)), |a, e| {
                Some(a.add(&rf(e)?))
            })?,
        Expr::Mul { factors } => factors
            .iter()
            .try_fold(RF::literal(Rational::integer(1)), |a, e| {
                Some(a.mul(&rf(e)?))
            })?,
        Expr::Div {
            numerator,
            denominator,
        } => rf(numerator)?.div(&rf(denominator)?)?,
        Expr::Pow {
            base,
            exponent: NatExpr::Constant { value },
        } => {
            let b = rf(base)?;
            RF {
                num: pp(&b.num, *value),
                den: pp(&b.den, *value),
            }
        }
        _ => return None,
    })
}
fn independent(e: &Expr) -> bool {
    match e {
        Expr::Term { .. } | Expr::Sum { .. } => false,
        Expr::Add { terms } => terms.iter().all(independent),
        Expr::Mul { factors } => factors.iter().all(independent),
        Expr::Div {
            numerator,
            denominator,
        } => independent(numerator) && independent(denominator),
        Expr::Pow { base, .. } => independent(base),
        _ => true,
    }
}
pub fn split(e: &Expr) -> Option<(Expr, Expr)> {
    if *e == Expr::term("a", 0) {
        return Some((Expr::integer(1), Expr::integer(0)));
    }
    if independent(e) {
        return Some((Expr::integer(0), e.clone()));
    }
    Some(match e {
        Expr::Add { terms } => {
            let xs = terms.iter().map(split).collect::<Option<Vec<_>>>()?;
            (
                Expr::add(xs.iter().map(|p| p.0.clone()).collect()),
                Expr::add(xs.into_iter().map(|p| p.1).collect()),
            )
        }
        Expr::Mul { factors } => {
            let mut a = Expr::integer(0);
            let mut b = Expr::integer(1);
            for e in factors {
                let (c, d) = split(e)?;
                if c != Expr::integer(0) && a != Expr::integer(0) {
                    return None;
                }
                a = Expr::add(vec![
                    Expr::mul(vec![a, d.clone()]),
                    Expr::mul(vec![b.clone(), c]),
                ]);
                b = Expr::mul(vec![b, d]);
            }
            (a, b)
        }
        Expr::Div {
            numerator,
            denominator,
        } if independent(denominator) => {
            let (a, b) = split(numerator)?;
            (
                Expr::div(a, *denominator.clone()),
                Expr::div(b, *denominator.clone()),
            )
        }
        _ => return None,
    })
}
fn solve_shift(force: &RF, coefficient: &RF, gn: &RF) -> Option<(Rational, Rational)> {
    // force + h(coefficient-1) = q*g(n+1), an exact polynomial identity.
    let b = coefficient.sub(&RF::literal(Rational::integer(1)));
    let a = pm(&pm(&force.num, &b.den), &gn.den);
    let b = pm(&pm(&b.num, &force.den), &gn.den);
    let c = pm(&pm(&gn.num, &force.den), &coefficient.den);
    if let Some(q) = proportional(&a, &c) {
        return Some((Rational::integer(0), q));
    }
    let degrees = a
        .keys()
        .chain(b.keys())
        .chain(c.keys())
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let get = |p: &Poly, k: &u32| p.get(k).cloned().unwrap_or_else(|| Rational::integer(0));
    for (i, di) in degrees.iter().enumerate() {
        for dj in degrees.iter().skip(i + 1) {
            let (ai, aj, bi, bj, ci, cj) = (
                get(&a, di),
                get(&a, dj),
                get(&b, di),
                get(&b, dj),
                get(&c, di),
                get(&c, dj),
            );
            let determinant = bj.mul(&ci).sub(&bi.mul(&cj));
            if determinant.is_zero() {
                continue;
            }
            let h = ai.mul(&cj).sub(&aj.mul(&ci)).div(&determinant).ok()?;
            let q = bj.mul(&ai).sub(&bi.mul(&aj)).div(&determinant).ok()?;
            let left = pa(&a, &b.iter().map(|(k, v)| (*k, v.mul(&h))).collect());
            let right = c
                .iter()
                .filter_map(|(k, v)| {
                    let x = v.mul(&q);
                    if x.is_zero() {
                        None
                    } else {
                        Some((*k, x))
                    }
                })
                .collect::<Poly>();
            if left == right {
                return Some((h, q));
            }
        }
    }
    None
}
#[derive(Clone, Debug)]
pub enum Route {
    Affine {
        ratio: Rational,
        forcing: Rational,
    },
    Scaled {
        profile: u32,
        ratio: Rational,
        shift: Rational,
        forcing: Rational,
    },
    Particular {
        ratio: Rational,
        slope: Rational,
        intercept: Rational,
    },
    Difference {
        forcing: Expr,
        degree: u8,
    },
    Reciprocal {
        inner: Box<Route>,
        coefficient: Expr,
        forcing: Expr,
    },
    Mobius {
        shift: Rational,
        ratio: Rational,
        forcing: Rational,
    },
}
fn scalar_route(coef: &Expr, force: &Expr) -> Option<Route> {
    let cf = rf(coef)?;
    let ff = rf(force)?;
    if let Some(r) = cf.scalar() {
        if let Some(q) = ff.scalar() {
            if !r.is_zero() && !r.is_one() && !q.is_zero() {
                return Some(Route::Affine {
                    ratio: r,
                    forcing: q,
                });
            }
        }
        if let Some(p) = ff.poly() {
            let degree = *p.last_key_value()?.0 as u8;
            if r.is_one() && (1..=2).contains(&degree) {
                return Some(Route::Difference {
                    forcing: force.clone(),
                    degree,
                });
            }
            if !r.is_zero() && !r.is_one() && degree == 1 {
                let alpha = p.get(&1)?.clone();
                let beta = p.get(&0).cloned().unwrap_or_else(|| Rational::integer(0));
                let slope = alpha.div(&Rational::integer(1).sub(&r)).ok()?;
                let intercept = beta.sub(&slope).div(&Rational::integer(1).sub(&r)).ok()?;
                return Some(Route::Particular {
                    ratio: r,
                    slope,
                    intercept,
                });
            }
        }
    }
    for profile in 0..12 {
        let g = rf(&scale(profile, 0))?;
        let gn = rf(&scale(profile, 1))?;
        let ratio = cf.mul(&g).div(&gn)?.scalar();
        let Some(ratio) = ratio else {
            continue;
        };
        if ratio.is_zero() {
            continue;
        }
        if let Some((shift, forcing)) = solve_shift(&ff, &cf, &gn) {
            if !shift.is_zero() || !forcing.is_zero() {
                return Some(Route::Scaled {
                    profile,
                    ratio,
                    shift,
                    forcing,
                });
            }
        }
    }
    None
}
fn sqrt_rat(x: &Rational) -> Option<Rational> {
    let a = x.0.numer().to_i64()?;
    let b = x.0.denom().to_i64()?;
    if a < 0 {
        return None;
    }
    let root = |v: i64| {
        let mut lo = 0i64;
        let mut hi = v.min(3_037_000_499) + 1;
        while hi - lo > 1 {
            let m = lo + (hi - lo) / 2;
            if (m as i128) * (m as i128) <= v as i128 {
                lo = m
            } else {
                hi = m
            }
        }
        (lo as i128 * lo as i128 == v as i128).then_some(lo)
    };
    Some(Rational::fraction(root(a)?, root(b)?))
}
pub fn detect(problem: &ProblemIR) -> Option<Route> {
    if problem.kind != RecurrenceKind::FirstOrder {
        return None;
    }
    let rhs = &problem.recurrence.rhs;
    if let Some((coef, force)) = split(rhs) {
        if let Some(route) = scalar_route(&coef, &force) {
            return Some(route);
        }
    }
    let Expr::Div {
        numerator,
        denominator,
    } = rhs
    else {
        return None;
    };
    let (p, q) = split(numerator)?;
    let (r, s) = split(denominator)?;
    if q == Expr::integer(0) {
        let coef = Expr::div(s, p.clone());
        let force = Expr::div(r, p);
        let route = scalar_route(&coef, &force)?;
        return Some(Route::Reciprocal {
            inner: Box::new(route),
            coefficient: coef,
            forcing: force,
        });
    }
    let (p, q, r, s) = (
        rf(&p)?.scalar()?,
        rf(&q)?.scalar()?,
        rf(&r)?.scalar()?,
        rf(&s)?.scalar()?,
    );
    if r.is_zero() {
        return None;
    }
    let discriminant = s.sub(&p).pow(2).add(&Rational::integer(4).mul(&r).mul(&q));
    let sqrt = sqrt_rat(&discriminant)?;
    let Expr::Rational { value: first } = &problem.initials.first()?.rhs else {
        return None;
    };
    for sign in [-1, 1] {
        let h = p
            .sub(&s)
            .add(&Rational::integer(sign).mul(&sqrt))
            .div(&Rational::integer(2).mul(&r))
            .ok()?;
        let lead = p.sub(&h.mul(&r));
        let coefficient = r.mul(&h).add(&s);
        if !lead.is_positive() || !coefficient.is_positive() || first.0 <= h.0 {
            continue;
        }
        let ratio = coefficient.div(&lead).ok()?;
        let forcing = r.div(&lead).ok()?;
        if !ratio.is_one() {
            return Some(Route::Mobius {
                shift: h,
                ratio,
                forcing,
            });
        }
    }
    None
}

fn route_meta(
    route: &Route,
) -> (
    u8,
    &'static str,
    &'static str,
    Vec<&'static str>,
    u8,
    u8,
    u8,
) {
    match route {
        Route::Affine { .. } => (
            5,
            "不動点を引き、等比数列に帰着する",
            "不動点を引いて等比数列にする",
            vec!["fixed_point", "geometric"],
            1,
            0,
            0,
        ),
        Route::Scaled {
            profile,
            ratio,
            shift,
            forcing,
        } => {
            let mono = matches!(profile % 6, 0 | 2);
            let score = if !forcing.is_zero() {
                14
            } else if ratio.is_one() && shift.is_zero() {
                if mono {
                    6
                } else {
                    9
                }
            } else if mono {
                10
            } else {
                14
            };
            let mut ops = Vec::new();
            if !shift.is_zero() {
                ops.push("shift");
            }
            ops.push("index_scale");
            if ratio.is_one() && !forcing.is_zero() {
                ops.extend(["difference_sum", "evaluate_sum"]);
            } else {
                if !forcing.is_zero() {
                    ops.push("fixed_point");
                }
                ops.push(if ratio.is_one() {
                    "constant"
                } else {
                    "geometric"
                });
            }
            (
                score,
                if !forcing.is_zero() {
                    "階比の正規化と定数項の消去を組み合わせる"
                } else {
                    "階比の正規化と定数の移動を組み合わせる"
                },
                "倍率で正規化して定数項を消す",
                ops,
                1 + u8::from(!shift.is_zero()) + u8::from(!forcing.is_zero() && !ratio.is_one()),
                0,
                if matches!(profile % 6, 2..=4) || (*profile < 6 && !forcing.is_zero()) {
                    2
                } else {
                    1
                },
            )
        }
        Route::Particular { .. } => (
            8,
            "一次式の特解を引いて等比数列に帰着する",
            "一次式の特解を引く",
            vec!["polynomial_shift", "geometric"],
            2,
            0,
            1,
        ),
        Route::Difference { degree, .. } => (
            if *degree < 2 { 3 } else { 4 },
            "多項式の階差を初項から足し合わせる",
            "多項式の階差を足し合わせる",
            vec![
                "difference_sum",
                if *degree < 2 {
                    "evaluate_sum"
                } else {
                    "evaluate_quadratic_sum"
                },
            ],
            0,
            0,
            *degree,
        ),
        Route::Reciprocal { inner, .. } => {
            let (mut score, _, _, mut ops, r, _, degree) = route_meta(inner);
            ops.insert(0, "reciprocal");
            score = if degree == 0 { 14 } else { 20.max(score) };
            (
                score,
                "逆数と変数係数・特解を組み合わせる",
                "逆数をとって補助数列を解く",
                ops,
                r + 1,
                1,
                degree,
            )
        }
        Route::Mobius { .. } => (
            14,
            "シンプルな1次分数を不動点・逆数で解く",
            "不動点からの差の逆数をとる",
            vec!["fixed_point", "reciprocal", "fixed_point", "geometric"],
            3,
            1,
            0,
        ),
    }
}
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    let Some(route) = detect(problem) else {
        return Ok(None);
    };
    let (score, reason, method, ops, discovery, domain, degree) = route_meta(&route);
    let classification = crate::generator::make_classification(
        problem, score, method, reason, &ops, discovery, domain, degree,
    )?;
    crate::generator::include_intermediate_values(
        problem,
        classification,
        &route_intermediate_values(problem, &route)?,
    )
    .map(Some)
}

/// Match the reference route's arithmetic budget using only recovered
/// coefficients and initial values, including constants introduced by a
/// substitution. The claimed general term cannot affect this measurement.
fn route_intermediate_values(problem: &ProblemIR, route: &Route) -> Result<Vec<Rational>, String> {
    let first = problem
        .initials
        .first()
        .ok_or("初期条件が不足しています。")?
        .rhs
        .evaluate(1, &|_, _| Err("初期条件が定数ではありません。".into()))?;
    if let Route::Mobius { shift, .. } = route {
        let Expr::Div {
            numerator,
            denominator,
        } = &problem.recurrence.rhs
        else {
            return Err("1次分数の式が登録形ではありません。".into());
        };
        let (p, q) = split(numerator).ok_or("1次分数の分子を分離できません。")?;
        let (r, s) = split(denominator).ok_or("1次分数の分母を分離できません。")?;
        let scalar = |e: &Expr| {
            rf(e)
                .and_then(|v| v.scalar())
                .ok_or("1次分数の係数が定数ではありません。".to_string())
        };
        let (p, q, r, s) = (scalar(&p)?, scalar(&q)?, scalar(&r)?, scalar(&s)?);
        let lead = p.sub(&shift.mul(&r));
        let coefficient = r.mul(shift).add(&s);
        // The reference measures the outer Mobius route, rather than
        // recursively adding the scalar certificate's intermediate values.
        return Ok(vec![p, q, r, s, shift.clone(), lead, coefficient, first]);
    }
    scalar_intermediate_values(route, &first)
}

fn scalar_intermediate_values(route: &Route, first: &Rational) -> Result<Vec<Rational>, String> {
    let zero = Rational::integer(0);
    let one = Rational::integer(1);
    Ok(match route {
        Route::Affine { ratio, forcing } => {
            let fixed = forcing.div(&one.sub(ratio))?;
            vec![
                zero,
                ratio.clone(),
                forcing.clone(),
                first.clone(),
                fixed.clone(),
                first.sub(&fixed),
            ]
        }
        Route::Scaled {
            profile,
            ratio,
            shift,
            forcing,
        } => {
            let g1 =
                scale(*profile, 0).evaluate(1, &|_, _| Err("倍率に数列項があります。".into()))?;
            let normalized_first = first.sub(shift).div(&g1)?;
            let mut values = vec![
                shift.clone(),
                ratio.clone(),
                forcing.clone(),
                normalized_first.clone(),
            ];
            if !ratio.is_one() {
                let fixed = forcing.div(&one.sub(ratio))?;
                values.extend([fixed.clone(), normalized_first.sub(&fixed)]);
            }
            values
        }
        Route::Particular {
            ratio,
            slope,
            intercept,
        } => {
            let alpha = one.sub(ratio).mul(slope);
            let beta = slope.add(&one.sub(ratio).mul(intercept));
            vec![
                ratio.clone(),
                alpha,
                beta,
                slope.clone(),
                intercept.clone(),
                first.sub(&slope.add(intercept)),
            ]
        }
        Route::Difference { forcing, .. } => {
            let polynomial = rf(forcing)
                .and_then(|v| v.poly())
                .ok_or("階差の係数を多項式として測定できません。")?;
            let mut values = vec![first.clone()];
            values.extend(polynomial.into_values());
            values
        }
        Route::Reciprocal { inner, .. } => scalar_intermediate_values(inner, &one.div(first)?)?,
        Route::Mobius { .. } => return Err("1次分数の外側の係数が必要です。".into()),
    })
}
fn ds(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn ieq(name: &str, value: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: name.into(),
            index: NatExpr::constant(1),
        },
        Expr::rational(value),
    )
}
fn aux(
    name: &str,
    definition: Expr,
    first: Rational,
    ratio: Rational,
    forcing: Expr,
) -> AuxiliarySequence {
    AuxiliarySequence {
        name: name.into(),
        definition: Equation::new(Expr::term(name, 0), definition),
        initial: ieq(name, first),
        recurrence: Equation::new(
            Expr::term(name, 1),
            Expr::add(vec![
                Expr::mul(vec![Expr::rational(ratio), Expr::term(name, 0)]),
                forcing,
            ]),
        ),
    }
}
fn explain_scalar(
    route: &Route,
    name: &str,
    first: Rational,
    steps: &mut Vec<DerivationStep>,
    auxiliaries: &mut Vec<AuxiliarySequence>,
) -> Result<(), String> {
    let a = Expr::term(name, 0);
    match route {
        Route::Affine { ratio, forcing } => {
            let fixed = forcing.div(&Rational::integer(1).sub(ratio))?;
            let b = aux(
                "b",
                Expr::sub(a, Expr::rational(fixed.clone())),
                first.sub(&fixed),
                ratio.clone(),
                Expr::integer(0),
            );
            steps.push(ds(
                "毎回同じ値になる不動点を、x=r x+qから求める。",
                Some(Equation::new(
                    Expr::integer(0),
                    Expr::sub(
                        Expr::rational(forcing.clone()),
                        Expr::rational(Rational::integer(1).sub(ratio).mul(&fixed)),
                    ),
                )),
                "不動点方程式の係数比較",
            ));
            steps.push(ds(
                "不動点を引いた補助数列を定義する。",
                Some(b.definition.clone()),
                "定数項を消去する",
            ));
            steps.push(ds(
                "補助数列は等比数列である。",
                Some(b.recurrence.clone()),
                "元の漸化式から不動点方程式を引く",
            ));
            steps.push(ds(
                "初項を計算し、公比をn−1回掛ける。",
                Some(Equation::new(
                    Expr::term("b", 0),
                    Expr::mul(vec![
                        Expr::rational(first.sub(&fixed)),
                        Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
                    ]),
                )),
                "等比数列の一般項",
            ));
            auxiliaries.push(b);
        }
        Route::Scaled {
            profile,
            ratio,
            shift,
            forcing,
        } => {
            let g = scale(*profile, 0);
            let g1 = g.evaluate(1, &|_, _| Err("無効な項".into()))?;
            let b1 = first.sub(shift).div(&g1)?;
            let b = aux(
                "b",
                Expr::div(Expr::sub(a, Expr::rational(shift.clone())), g),
                b1.clone(),
                ratio.clone(),
                Expr::rational(forcing.clone()),
            );
            steps.push(ds("左右の係数を倍率g_nとg_{n+1}として比べる。g_nはn≧1で正なので、次の置換を定義できる。",Some(b.definition.clone()),"倍率の正値性と定数の移動"));
            steps.push(ds(
                "置換を代入し、共通の倍率を相殺すると定数係数の漸化式になる。",
                Some(b.recurrence.clone()),
                "両辺をg_{n+1}で割る",
            ));
            steps.push(ds(
                "初期条件にも同じ置換を用いる。",
                Some(b.initial.clone()),
                "n=1を代入する",
            ));
            auxiliaries.push(b);
            if ratio.is_one() && !forcing.is_zero() {
                steps.push(ds(
                    "正規化した数列は等差数列である。初項からn−1回だけ同じ差を加える。",
                    Some(Equation::new(
                        Expr::term("b", 0),
                        Expr::add(vec![
                            Expr::rational(b1),
                            Expr::mul(vec![Expr::rational(forcing.clone()), Expr::offset(-1)]),
                        ]),
                    )),
                    "等差数列の一般項",
                ));
            } else if !forcing.is_zero() {
                let fixed = forcing.div(&Rational::integer(1).sub(ratio))?;
                let t = aux(
                    "t",
                    Expr::sub(Expr::term("b", 0), Expr::rational(fixed.clone())),
                    b1.sub(&fixed),
                    ratio.clone(),
                    Expr::integer(0),
                );
                steps.push(ds(
                    "さらに不動点q/(1-r)を引くと、等比数列になる。",
                    Some(t.definition.clone()),
                    "定数項の消去",
                ));
                steps.push(ds(
                    "初項と公比から等比数列の一般項を求める。",
                    Some(Equation::new(
                        Expr::term("t", 0),
                        Expr::mul(vec![
                            Expr::rational(b1.sub(&fixed)),
                            Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
                        ]),
                    )),
                    "等比数列の一般項",
                ));
                auxiliaries.push(t);
            } else {
                steps.push(ds(
                    if ratio.is_one() {
                        "補助数列はすべて初項に等しい。"
                    } else {
                        "補助数列の初項と公比から一般項を求める。"
                    },
                    Some(Equation::new(
                        Expr::term("b", 0),
                        Expr::mul(vec![
                            Expr::rational(b1),
                            Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
                        ]),
                    )),
                    "定数列または等比数列の一般項",
                ));
            }
        }
        Route::Particular {
            ratio,
            slope,
            intercept,
        } => {
            let particular = Expr::add(vec![
                Expr::mul(vec![Expr::rational(slope.clone()), Expr::index()]),
                Expr::rational(intercept.clone()),
            ]);
            let amplitude = first.sub(&slope.add(intercept));
            steps.push(ds(
                "付加項がnの一次式なので、特解をu_n=An+Bとおき、nの係数と定数を比較する。",
                Some(Equation::new(Expr::term("u", 0), particular.clone())),
                "(1-r)A=α、A+(1-r)B=βを解く",
            ));
            let b = aux(
                "b",
                Expr::sub(a, particular),
                amplitude.clone(),
                ratio.clone(),
                Expr::integer(0),
            );
            steps.push(ds(
                "特解を引いた数列を定義する。",
                Some(b.definition.clone()),
                "付加項の消去",
            ));
            steps.push(ds(
                "補助数列は等比数列になる。",
                Some(b.recurrence.clone()),
                "元の式から特解の式を引く",
            ));
            steps.push(ds(
                "補助数列の初項を代入して一般項を求める。",
                Some(Equation::new(
                    Expr::term("b", 0),
                    Expr::mul(vec![
                        Expr::rational(amplitude),
                        Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
                    ]),
                )),
                "等比数列の一般項",
            ));
            auxiliaries.push(b);
        }
        Route::Difference { forcing, degree } => {
            steps.push(ds(
                "隣り合う項の差を初項から第n−1項まで足す。途中の数列の項は相殺される。",
                Some(Equation::new(
                    Expr::sub(Expr::term(name, 1), a),
                    forcing.clone(),
                )),
                "階差の総和",
            ));
            steps.push(ds(
                if *degree == 1 {
                    "一次式の和は、整数の和Σk=(n−1)n/2で計算する。"
                } else {
                    "二次式の和には、整数の和と平方和Σk²=(n−1)n(2n−1)/6を用いる。"
                },
                None,
                "多項式を各単項式の和に分ける",
            ));
            steps.push(ds(
                "n=1では総和は空で0になり、初項とも一致する。",
                None,
                "空和の境界条件",
            ));
        }
        _ => return Err("補助数列の解法が不正です。".into()),
    }
    Ok(())
}
pub fn explain(
    problem: &ProblemIR,
    classification: &Classification,
) -> Result<Option<DerivationIR>, String> {
    let Some(route) = detect(problem) else {
        return Ok(None);
    };
    let Expr::Rational { value: first } = &problem.initials[0].rhs else {
        return Err("初項が数値ではありません。".into());
    };
    let mut steps = Vec::new();
    let mut auxiliaries = Vec::new();
    let hint = match &route {
        Route::Reciprocal {
            inner,
            coefficient,
            forcing,
        } => {
            let v1 = Rational::integer(1).div(first)?;
            let v = aux(
                "v",
                Expr::div(Expr::integer(1), Expr::term("a", 0)),
                v1.clone(),
                Rational::integer(1),
                Expr::integer(0),
            );
            let mut v = v;
            v.recurrence = Equation::new(
                Expr::term("v", 1),
                Expr::add(vec![
                    Expr::mul(vec![coefficient.clone(), Expr::term("v", 0)]),
                    forcing.clone(),
                ]),
            );
            steps.push(ds(
                "まず逆数の補助数列を考える。",
                Some(v.definition.clone()),
                "各項が非零であることは、得られる補助数列の正値性で確認する",
            ));
            steps.push(ds(
                "与式の逆数をとると、一次式の漸化式になる。",
                Some(v.recurrence.clone()),
                "分子と分母を元の項で割る",
            ));
            auxiliaries.push(v);
            explain_scalar(inner, "v", v1, &mut steps, &mut auxiliaries)?;
            steps.push(ds("補助数列は全項で正である。元の分母はv_{n+1}/v_n>0だから零にならず、逆数による置換はすべての項で成立する。",None,"正の一般項、または正の初項と漸化式による帰納法"));
            "まず逆数をとり、係数の倍率と付加項を比べる。"
        }
        Route::Mobius {
            shift,
            ratio,
            forcing,
        } => {
            let v1 = Rational::integer(1).div(&first.sub(shift))?;
            let v = aux(
                "v",
                Expr::div(
                    Expr::integer(1),
                    Expr::sub(Expr::term("a", 0), Expr::rational(shift.clone())),
                ),
                v1.clone(),
                ratio.clone(),
                Expr::rational(forcing.clone()),
            );
            steps.push(ds(
                "x=(px+q)/(rx+s)を解き、不動点hに着目する。",
                Some(Equation::new(
                    Expr::Term {
                        sequence: "h".into(),
                        index: NatExpr::constant(0),
                    },
                    Expr::rational(shift.clone()),
                )),
                "rx²+(s-p)x-q=0の有理数解",
            ));
            steps.push(ds(
                "不動点からの差の逆数をとる。",
                Some(v.definition.clone()),
                "分子からh倍の分母を引く",
            ));
            steps.push(ds(
                "分数の式は定数係数の一次式に変わる。",
                Some(v.recurrence.clone()),
                "不動点方程式で定数項を整理する",
            ));
            auxiliaries.push(v);
            explain_scalar(
                &Route::Affine {
                    ratio: ratio.clone(),
                    forcing: forcing.clone(),
                },
                "v",
                v1,
                &mut steps,
                &mut auxiliaries,
            )?;
            steps.push(ds("v_nは全項で正である。a_n-h=1/v_n≠0で、元の分母は正の定数倍v_{n+1}/v_nとなり、すべての項で定義できる。",None,"置換の定義域と元の分母の非零性"));
            "分子と分母が一次式なので、不動点からの差の逆数を考える。"
        }
        _ => {
            explain_scalar(&route, "a", first.clone(), &mut steps, &mut auxiliaries)?;
            "係数と付加項を比べ、定数項を消す置換または階差の和を考える。"
        }
    };
    steps.push(ds(
        "補助数列の置換を元に戻すと、求める一般項を得る。",
        Some(problem.general_term.clone()),
        "逆変換と初期条件の確認",
    ));
    Ok(Some(DerivationIR {
        method_name: classification.method_name.clone(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(family: &str, profile: u32) -> BTreeMap<String, Rational> {
        let mut p = params(&[("c", Rational::integer(1)), ("d", Rational::integer(8))]);
        if !matches!(family, "shifted_scaled" | "polynomial_difference") {
            p.insert("r".into(), Rational::integer(2));
        }
        if matches!(
            family,
            "shifted_scaled" | "scaled_affine" | "reciprocal_scaled" | "polynomial_difference"
        ) {
            p.insert("profile".into(), Rational::integer(profile as i64));
        }
        if family == "mobius" {
            p.insert("h".into(), Rational::integer(2));
        }
        p
    }
    #[test]
    fn first_order_detection_and_exact_recurrences() {
        for family in catalog() {
            for profile in 0..if family.id == "polynomial_difference" {
                6
            } else if family.id.contains("scaled") {
                12
            } else {
                1
            } {
                let Ok(recipe) = build_recipe(&family.id, 1, p(&family.id, profile)) else {
                    continue;
                };
                let ir = compile(&recipe).unwrap();
                let Some(classification) = classify(&ir).unwrap() else {
                    panic!("{} {}", family.id, profile)
                };
                println!("{} {} => {}", family.id, profile, classification.score);
                assert!(explain(&ir, &classification).unwrap().is_some());
                let term = |_: &str, k: u32| {
                    ir.general_term
                        .rhs
                        .evaluate(k, &|_, _| Err("badterm".into()))
                };
                for n in 1..=20 {
                    assert_eq!(
                        ir.recurrence.lhs.evaluate(n, &term).unwrap(),
                        ir.recurrence.rhs.evaluate(n, &term).unwrap(),
                        "{} {} n{}",
                        family.id,
                        profile,
                        n
                    );
                }
                let mut damaged = ir.clone();
                damaged.general_term.rhs = Expr::integer(999);
                assert_eq!(
                    classification.score,
                    classify(&damaged).unwrap().unwrap().score
                );
                assert_eq!(
                    classification.numeric_cost,
                    classify(&damaged).unwrap().unwrap().numeric_cost
                );
            }
        }
    }
    #[test]
    #[ignore = "requires prepared Lean 4.19 + mathlib"]
    fn first_order_profiles_have_audited_lean_proofs() {
        use crate::verifier::Verifier;
        use std::{
            path::PathBuf,
            sync::{atomic::AtomicBool, Arc},
            time::Duration,
        };
        let verifier = Verifier::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean"));
        assert!(verifier.status().ready, "{}", verifier.status().message);
        for family in catalog() {
            if std::env::var("FIRST_ORDER_FAMILY")
                .is_ok_and(|f| f.split(',').all(|item| item != family.id))
            {
                continue;
            }
            for profile in 0..if family.id == "polynomial_difference" {
                6
            } else if family.id.contains("scaled") {
                12
            } else {
                1
            } {
                if std::env::var("FIRST_ORDER_MIN_PROFILE")
                    .ok()
                    .and_then(|s| s.parse::<u32>().ok())
                    .is_some_and(|min| profile < min)
                {
                    continue;
                }
                let Ok(recipe) = build_recipe(&family.id, 1, p(&family.id, profile)) else {
                    continue;
                };
                let problem = crate::generator::replay(&recipe).unwrap();
                let result = verifier.verify(
                    &problem,
                    Arc::new(AtomicBool::new(false)),
                    Duration::from_secs(90),
                );
                println!(
                    "{} {} => {:?}",
                    family.id,
                    profile,
                    result.as_ref().map(|r| r.elapsed_ms)
                );
                assert!(result.is_ok(), "{} {}: {:?}", family.id, profile, result);
            }
        }
    }
}
