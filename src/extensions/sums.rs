//! Actual partial sums, their boundary at S₀ = 0, and mixed sum relations.
use crate::generator::{
    AuxiliarySequence, Classification, DerivationIR, DerivationStep, DomainCondition, FamilyInfo,
    ProblemIR, Recipe, RecipeBlock, RecipeCore, RecurrenceKind,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_traits::Signed;
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;

pub fn supports(family: &str) -> bool {
    matches!(family, "pure_sum" | "pure_sum_scaled" | "sum_relation")
}

pub fn catalog() -> Vec<FamilyInfo> {
    [
        ("pure_sum", "部分和の漸化式", 2),
        ("pure_sum_scaled", "正規化した部分和の3項間", 5),
        ("sum_relation", "部分和と一般項の関係", 2),
    ]
    .into_iter()
    .map(|(id, label, level)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels: vec![level],
    })
    .collect()
}

pub fn sample(
    family: &str,
    _level: u8,
    rng: &mut ChaCha8Rng,
) -> Result<BTreeMap<String, Rational>, String> {
    let mut p = BTreeMap::from([
        ("c".into(), Rational::integer(rng.gen_range(1..=5))),
        ("d".into(), Rational::integer(rng.gen_range(1..=6))),
        ("r".into(), Rational::integer(rng.gen_range(2..=4))),
        ("profile".into(), Rational::integer(rng.gen_range(0..=1))),
    ]);
    if family == "pure_sum_scaled" {
        let mut s = Rational::integer(rng.gen_range(2..=4));
        while s == p["r"] {
            s = Rational::integer(rng.gen_range(2..=4));
        }
        p.insert("s".into(), s);
        // Only linear scales meet the reference's coefficient-degree budget.
        let profile = [0, 1, 5][rng.gen_range(0..3)];
        p.insert("profile".into(), Rational::integer(profile));
        if profile != 0 {
            // For g(0) != 0, impose the actual S₀ boundary rather than assuming it.
            p.insert(
                "c".into(),
                Rational::integer(-1)
                    .mul(&p["d"])
                    .mul(&p["s"])
                    .div(&p["r"])
                    .unwrap(),
            );
        }
    }
    Ok(p)
}

pub fn validate_parameters(family: &str, p: &BTreeMap<String, Rational>) -> Result<(), String> {
    if !supports(family) {
        return Err("未登録の総和型です。".into());
    }
    let expected = if family == "pure_sum_scaled" {
        vec!["c", "d", "profile", "r", "s"]
    } else {
        vec!["c", "d", "profile", "r"]
    };
    if p.keys().map(String::as_str).collect::<Vec<_>>() != expected {
        return Err("総和型のパラメータ集合が登録規則と一致しません。".into());
    }
    if p["c"].is_zero() || !p["d"].is_positive() {
        return Err("総和型の振幅が退化しています。".into());
    }
    for name in ["c", "d"] {
        if p[name].0.abs() > Rational::integer(16).0 {
            return Err("総和型の振幅は絶対値16以下です。".into());
        }
    }
    let r = p["r"].integer_u32().ok_or("公比は整数です。")?;
    if !(2..=5).contains(&r) {
        return Err("公比は2から5です。".into());
    }
    let profile = p["profile"]
        .integer_u32()
        .ok_or("総和型のプロファイルが整数ではありません。")?;
    if family == "pure_sum_scaled" {
        let s = p["s"].integer_u32().ok_or("特性根は整数です。")?;
        if !(2..=5).contains(&s) || s == r || ![0, 1, 5].contains(&profile) {
            return Err("正規化した部分和の特性根・倍率が登録範囲外です。".into());
        }
        let g0 = scale(profile, -1).evaluate(1, &|_, _| Err("数列項はありません。".into()))?;
        if !g0
            .mul(&p["d"].div(&p["r"])?.add(&p["c"].div(&p["s"])?))
            .is_zero()
        {
            return Err("部分和の境界S₀=0が成立しません。".into());
        }
    } else if profile > 1 || !p["c"].is_positive() {
        return Err("部分和型の多項式プロファイルが登録範囲外です。".into());
    }
    Ok(())
}

/// The six reference scale profiles; offset is applied to the public n.
pub fn scale(profile: u32, offset: i32) -> Expr {
    let n = Expr::offset(offset);
    match profile {
        0 => n,
        1 => Expr::add(vec![n, Expr::integer(1)]),
        2 => Expr::pow(n, NatExpr::constant(2)),
        3 => Expr::mul(vec![n.clone(), Expr::add(vec![n, Expr::integer(1)])]),
        4 => Expr::mul(vec![n.clone(), Expr::add(vec![n, Expr::integer(2)])]),
        5 => Expr::add(vec![Expr::mul(vec![Expr::integer(2), n]), Expr::integer(1)]),
        _ => Expr::integer(0),
    }
}

pub fn sum(offset: i32) -> Expr {
    Expr::Sum {
        variable: "k".into(),
        lower: NatExpr::constant(1),
        upper: NatExpr::offset(offset),
        body: Box::new(Expr::Term {
            sequence: "a".into(),
            index: NatExpr::Bound { name: "k".into() },
        }),
    }
}

fn sum_initial(index: u32, value: Rational) -> Equation {
    Equation::new(
        Expr::Sum {
            variable: "k".into(),
            lower: NatExpr::constant(1),
            upper: NatExpr::constant(index),
            body: Box::new(Expr::Term {
                sequence: "a".into(),
                index: NatExpr::Bound { name: "k".into() },
            }),
        },
        Expr::rational(value),
    )
}

pub fn build_recipe(
    family: &str,
    seed: u64,
    p: BTreeMap<String, Rational>,
) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    let mut blocks = Vec::new();
    let mut add_block = |kind: &str, parameters: BTreeMap<String, Rational>| {
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
    if family == "pure_sum_scaled" {
        add_block(
            "linear_combination",
            BTreeMap::from([
                ("amplitude".into(), p["c"].clone()),
                ("ratio".into(), p["s"].clone()),
            ]),
        );
        add_block(
            "index_scale",
            BTreeMap::from([("profile".into(), p["profile"].clone())]),
        );
    } else {
        let intercept = if family == "pure_sum" {
            Rational::integer(-1).mul(&p["d"]).div(&p["r"])?
        } else {
            p["c"].clone()
        };
        let slope = if p["profile"].is_zero() {
            Rational::integer(0)
        } else {
            p["c"].clone()
        };
        let intercept = if family == "sum_relation" && !p["profile"].is_zero() {
            Rational::integer(0)
        } else {
            intercept
        };
        add_block(
            "index_add",
            BTreeMap::from([("constant".into(), intercept), ("linear".into(), slope)]),
        );
    }
    add_block(
        if family == "sum_relation" {
            "sum_relation_encode"
        } else {
            "partial_sum_encode"
        },
        BTreeMap::new(),
    );
    let output = blocks.last().unwrap().id.clone();
    Ok(Recipe {
        schema_version: crate::generator::SCHEMA_VERSION.into(),
        rules_version: crate::generator::EXTENSION_RULES_VERSION.into(),
        rng_version: crate::generator::RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p.clone(),
        core: RecipeCore {
            id: "core".into(),
            kind: "geometric".into(),
            parameters: BTreeMap::from([
                ("amplitude".into(), p["d"].clone()),
                ("ratio".into(), p["r"].clone()),
            ]),
        },
        blocks,
        output,
        index_start: 1,
    })
}

pub fn sum_formula(recipe: &Recipe) -> Result<Expr, String> {
    let p = &recipe.parameters;
    let mode = |amp: &Rational, ratio: &Rational| {
        Expr::mul(vec![
            Expr::rational(amp.clone()),
            Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
        ])
    };
    Ok(match recipe.family.as_str() {
        "pure_sum" => Expr::add(vec![
            mode(&p["d"], &p["r"]),
            Expr::rational(Rational::integer(-1).mul(&p["d"]).div(&p["r"])?),
            if p["profile"].is_zero() {
                Expr::integer(0)
            } else {
                Expr::mul(vec![Expr::rational(p["c"].clone()), Expr::index()])
            },
        ]),
        "pure_sum_scaled" => Expr::mul(vec![
            scale(p["profile"].integer_u32().unwrap(), 0),
            Expr::add(vec![mode(&p["d"], &p["r"]), mode(&p["c"], &p["s"])]),
        ]),
        "sum_relation" => {
            let geometric = Expr::mul(vec![
                Expr::rational(p["d"].div(&p["r"].sub(&Rational::integer(1)))?),
                Expr::sub(
                    Expr::pow(Expr::rational(p["r"].clone()), NatExpr::n()),
                    Expr::integer(1),
                ),
            ]);
            let polynomial = if p["profile"].is_zero() {
                Expr::mul(vec![Expr::rational(p["c"].clone()), Expr::index()])
            } else {
                Expr::mul(vec![
                    Expr::rational(p["c"].div(&Rational::integer(2))?),
                    Expr::index(),
                    Expr::offset(1),
                ])
            };
            Expr::add(vec![geometric, polynomial])
        }
        _ => return Err("未登録の部分和型です。".into()),
    })
}

pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    validate_parameters(&recipe.family, &recipe.parameters)?;
    let p = &recipe.parameters;
    let r = &p["r"];
    let c = &p["c"];
    let d = &p["d"];
    let mode = |amp: Rational, ratio: &Rational| {
        Expr::mul(vec![
            Expr::rational(amp),
            Expr::pow(Expr::rational(ratio.clone()), NatExpr::offset(-1)),
        ])
    };
    let mut conditions = Vec::new();
    let (kind, recurrence, formula, initials) = match recipe.family.as_str() {
        "pure_sum" => {
            let u = if p["profile"].is_zero() {
                Rational::integer(0)
            } else {
                c.clone()
            };
            let forcing = Expr::add(vec![
                Expr::mul(vec![
                    Expr::rational(Rational::integer(1).sub(r).mul(&u)),
                    Expr::index(),
                ]),
                Expr::rational(u.add(&r.sub(&Rational::integer(1)).mul(d).div(r)?)),
            ]);
            let a = mode(d.mul(&r.sub(&Rational::integer(1))).div(r)?, r);
            let formula = Expr::add(vec![a, Expr::rational(u.clone())]);
            (
                RecurrenceKind::PureSum,
                Equation::new(
                    sum(1),
                    Expr::add(vec![
                        Expr::mul(vec![Expr::rational(r.clone()), sum(0)]),
                        forcing,
                    ]),
                ),
                formula,
                vec![sum_initial(1, d.sub(&d.div(r)?).add(&u))],
            )
        }
        "pure_sum_scaled" => {
            let profile = p["profile"].integer_u32().unwrap();
            let s = &p["s"];
            let g0 = scale(profile, 0);
            let g1 = scale(profile, 1);
            let g2 = scale(profile, 2);
            let lead = Expr::mul(vec![g0.clone(), g1.clone()]);
            let q = Expr::mul(vec![Expr::rational(r.add(s)), g0.clone(), g2.clone()]);
            let z = Expr::mul(vec![
                Expr::rational(Rational::integer(-1).mul(r).mul(s)),
                g1,
                g2,
            ]);
            conditions.push(DomainCondition {
                kind: "nonzero".into(),
                expression: g0.clone(),
                explanation: "n≥1では部分和を正規化する分母g_nは正である。".into(),
            });
            conditions.push(DomainCondition {
                kind: "nonzero".into(),
                expression: lead.clone(),
                explanation: "n≥1では正規化の倍率g_nとg_(n+1)は正である。".into(),
            });
            let now = Expr::mul(vec![
                g0,
                Expr::add(vec![mode(d.clone(), r), mode(c.clone(), s)]),
            ]);
            let prev = Expr::mul(vec![
                scale(profile, -1),
                Expr::add(vec![mode(d.div(r)?, r), mode(c.div(s)?, s)]),
            ]);
            let partial_formula = sum_formula(recipe)?;
            let values = (1..=2)
                .map(|i| {
                    partial_formula
                        .evaluate(i, &|_, _| Err("初期値に数列項が含まれています。".into()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            (
                RecurrenceKind::PureSum,
                Equation::new(
                    Expr::mul(vec![lead, sum(2)]),
                    Expr::add(vec![Expr::mul(vec![q, sum(1)]), Expr::mul(vec![z, sum(0)])]),
                ),
                Expr::sub(now, prev),
                vec![
                    sum_initial(1, values[0].clone()),
                    sum_initial(2, values[1].clone()),
                ],
            )
        }
        "sum_relation" => {
            let alpha = r.div(&r.sub(&Rational::integer(1)))?;
            let h = if p["profile"].is_zero() {
                Expr::rational(c.clone())
            } else {
                Expr::mul(vec![Expr::rational(c.clone()), Expr::index()])
            };
            let polynomial_sum = if p["profile"].is_zero() {
                Expr::mul(vec![Expr::rational(c.clone()), Expr::index()])
            } else {
                Expr::mul(vec![
                    Expr::rational(c.div(&Rational::integer(2))?),
                    Expr::index(),
                    Expr::offset(1),
                ])
            };
            let forcing = Expr::add(vec![
                polynomial_sum,
                Expr::mul(vec![
                    Expr::rational(Rational::integer(-1).mul(&alpha)),
                    h.clone(),
                ]),
                Expr::rational(
                    Rational::integer(-1)
                        .mul(d)
                        .div(&r.sub(&Rational::integer(1)))?,
                ),
            ]);
            let formula = Expr::add(vec![mode(d.clone(), r), h]);
            let initial = Equation::new(
                Expr::Term {
                    sequence: "a".into(),
                    index: NatExpr::constant(1),
                },
                Expr::rational(d.add(c)),
            );
            (
                RecurrenceKind::SumRelation,
                Equation::new(
                    sum(0),
                    Expr::add(vec![
                        Expr::mul(vec![Expr::rational(alpha), Expr::term("a", 0)]),
                        forcing,
                    ]),
                ),
                formula,
                vec![initial],
            )
        }
        _ => return Err("未登録の総和型です。".into()),
    };
    Ok(ProblemIR {
        kind,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence,
        initials,
        general_term: Equation::new(Expr::term("a", 0), formula),
        conditions,
        secondary_recurrences: Vec::new(),
        secondary_general_terms: Vec::new(),
    })
}

type Polynomial = BTreeMap<u32, Rational>;
fn poly_add(a: Polynomial, b: Polynomial) -> Polynomial {
    let mut a = a;
    for (degree, value) in b {
        let x = a.entry(degree).or_insert_with(|| Rational::integer(0));
        *x = x.add(&value);
    }
    a.retain(|_, value| !value.is_zero());
    a
}
fn poly_mul(a: &Polynomial, b: &Polynomial) -> Polynomial {
    let mut out = BTreeMap::new();
    for (i, x) in a {
        for (j, y) in b {
            let z = out.entry(i + j).or_insert_with(|| Rational::integer(0));
            *z = z.add(&x.mul(y));
        }
    }
    out.retain(|_, value| !value.is_zero());
    out
}
fn natural_poly(value: &NatExpr) -> Result<Polynomial, String> {
    Ok(match value {
        NatExpr::Index => BTreeMap::from([(1, Rational::integer(1))]),
        NatExpr::Constant { value } => BTreeMap::from([(0, Rational::integer(*value as i64))]),
        NatExpr::Add { left, right } => poly_add(natural_poly(left)?, natural_poly(right)?),
        NatExpr::Sub { left, right } => poly_add(
            natural_poly(left)?,
            natural_poly(right)?
                .into_iter()
                .map(|(i, c)| (i, Rational::integer(-1).mul(&c)))
                .collect(),
        ),
        _ => return Err("多項式の添字に未登録の変数が含まれています。".into()),
    })
}
fn polynomial(e: &Expr) -> Result<Polynomial, String> {
    let p = match e {
        Expr::Rational { value } => BTreeMap::from([(0, value.clone())]),
        Expr::NatCast { value } => natural_poly(value)?,
        Expr::Add { terms } => terms.iter().try_fold(BTreeMap::new(), |p, x| {
            Ok::<_, String>(poly_add(p, polynomial(x)?))
        })?,
        Expr::Mul { factors } => factors
            .iter()
            .try_fold(BTreeMap::from([(0, Rational::integer(1))]), |p, x| {
                Ok::<_, String>(poly_mul(&p, &polynomial(x)?))
            })?,
        Expr::Div {
            numerator,
            denominator,
        } => {
            let d = polynomial(denominator)?;
            if d.keys().any(|i| *i != 0) {
                return Err("多項式の分母が定数ではありません。".into());
            }
            let d = d.get(&0).cloned().unwrap_or_else(|| Rational::integer(0));
            polynomial(numerator)?
                .into_iter()
                .map(|(i, c)| c.div(&d).map(|v| (i, v)))
                .collect::<Result<_, _>>()?
        }
        Expr::Pow {
            base,
            exponent: NatExpr::Constant { value },
        } if *value <= 2 => {
            let b = polynomial(base)?;
            (0..*value).fold(BTreeMap::from([(0, Rational::integer(1))]), |p, _| {
                poly_mul(&p, &b)
            })
        }
        _ => return Err("登録範囲の多項式ではありません。".into()),
    };
    Ok(p.into_iter().filter(|(_, c)| !c.is_zero()).collect())
}
fn coeff(p: &Polynomial, degree: u32) -> Rational {
    p.get(&degree)
        .cloned()
        .unwrap_or_else(|| Rational::integer(0))
}
fn from_poly(p: &Polynomial) -> Expr {
    Expr::add(
        p.iter()
            .rev()
            .map(|(i, c)| {
                Expr::mul(vec![
                    Expr::rational(c.clone()),
                    if *i == 0 {
                        Expr::integer(1)
                    } else if *i == 1 {
                        Expr::index()
                    } else {
                        Expr::pow(Expr::index(), NatExpr::constant(*i))
                    },
                ])
            })
            .collect(),
    )
}
fn contains(expr: &Expr, target: &Expr) -> bool {
    if expr == target {
        return true;
    }
    match expr {
        Expr::Add { terms } => terms.iter().any(|x| contains(x, target)),
        Expr::Mul { factors } => factors.iter().any(|x| contains(x, target)),
        Expr::Div {
            numerator,
            denominator,
        } => contains(numerator, target) || contains(denominator, target),
        Expr::Pow { base, .. } => contains(base, target),
        _ => false,
    }
}
pub(crate) fn split(expr: &Expr, target: &Expr) -> Result<(Expr, Expr), String> {
    if expr == target {
        return Ok((Expr::integer(1), Expr::integer(0)));
    }
    if !contains(expr, target) {
        return Ok((Expr::integer(0), expr.clone()));
    }
    match expr {
        Expr::Add { terms } => {
            let p = terms
                .iter()
                .map(|x| split(x, target))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                Expr::add(p.iter().map(|x| x.0.clone()).collect()),
                Expr::add(p.into_iter().map(|x| x.1).collect()),
            ))
        }
        Expr::Mul { factors } => {
            let mut a = Expr::integer(1);
            let mut b = Expr::integer(1);
            let mut seen = false;
            for x in factors {
                let (q, r) = split(x, target)?;
                if q != Expr::integer(0) {
                    if seen {
                        return Err("総和の式が非線形です。".into());
                    }
                    seen = true;
                    a = Expr::mul(vec![a, q]);
                    b = Expr::mul(vec![b, r]);
                } else {
                    a = Expr::mul(vec![a, r.clone()]);
                    b = Expr::mul(vec![b, r]);
                }
            }
            Ok((a, b))
        }
        Expr::Div {
            numerator,
            denominator,
        } if !contains(denominator, target) => {
            let (a, b) = split(numerator, target)?;
            Ok((
                Expr::div(a, *denominator.clone()),
                Expr::div(b, *denominator.clone()),
            ))
        }
        _ => Err("総和の式を一次式として分離できません。".into()),
    }
}
fn constant(e: &Expr) -> Result<Rational, String> {
    let p = polynomial(e)?;
    if p.keys().any(|d| *d > 0) {
        Err("係数が定数ではありません。".into())
    } else {
        Ok(coeff(&p, 0))
    }
}
fn initial_value(p: &ProblemIR, i: usize) -> Result<Rational, String> {
    constant(
        &p.initials
            .get(i)
            .ok_or("部分和の初期条件が不足しています。")?
            .rhs,
    )
}
fn geometric(amp: Rational, r: &Rational) -> Expr {
    Expr::mul(vec![
        Expr::rational(amp),
        Expr::pow(Expr::rational(r.clone()), NatExpr::offset(-1)),
    ])
}
fn step(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: Vec::new(),
    }
}

/// All parameters of this route are recovered from the statement and initials.
/// Neither a Recipe nor an alleged general term participates in recognition.
pub struct SumRoute {
    pub score: u8,
    pub reason: String,
    pub operations: Vec<String>,
    pub discovery: u8,
    pub domain: u8,
    pub degree: u8,
    pub derivation: DerivationIR,
    pub intermediate_values: Vec<Rational>,
    #[allow(dead_code)] // Used in independent route-recovery acceptance tests.
    pub formula: Expr,
}

pub fn forward(problem: &ProblemIR) -> Result<SumRoute, String> {
    if problem.kind == RecurrenceKind::SumRelation {
        return relation_route(problem);
    }
    if problem.kind != RecurrenceKind::PureSum {
        return Err("部分和の問題ではありません。".into());
    }
    if problem.initials.len() == 2 {
        return scaled_route(problem);
    }
    let (lead, rest) = split(&problem.recurrence.lhs, &sum(1))?;
    if constant(&lead)? != Rational::integer(1) || rest != Expr::integer(0) {
        return Err("部分和の次項の係数が登録形ではありません。".into());
    }
    let (ratio, forcing) = split(&problem.recurrence.rhs, &sum(0))?;
    let r = constant(&ratio)?;
    if r.is_zero() || r.is_one() {
        return Err("部分和の公比が退化しています。".into());
    }
    let f = polynomial(&forcing)?;
    if f.keys().any(|degree| *degree > 1) {
        return Err("部分和の非同次項は高々一次式です。".into());
    }
    let one_minus_r = Rational::integer(1).sub(&r);
    let u = coeff(&f, 1).div(&one_minus_r)?;
    let v = coeff(&f, 0).sub(&u).div(&one_minus_r)?;
    let initial = initial_value(problem, 0)?;
    let amplitude = initial.sub(&u).sub(&v);
    if !amplitude.div(&r)?.add(&v).is_zero() {
        return Err("求めた部分和がS₀=0を満たしません。".into());
    }
    let formula = Expr::add(vec![
        geometric(amplitude.mul(&r.sub(&Rational::integer(1))).div(&r)?, &r),
        Expr::rational(u.clone()),
    ]);
    let partial_formula = Expr::add(vec![
        geometric(amplitude, &r),
        Expr::mul(vec![Expr::rational(u.clone()), Expr::index()]),
        Expr::rational(v),
    ]);
    let a2 = r
        .sub(&Rational::integer(1))
        .mul(&initial)
        .add(&coeff(&f, 1))
        .add(&coeff(&f, 0));
    let scalar_forcing = coeff(&f, 1);
    let scalar_recurrence = Equation::new(
        Expr::term("a", 1),
        Expr::add(vec![
            Expr::mul(vec![Expr::rational(r.clone()), Expr::term("a", 0)]),
            Expr::rational(scalar_forcing),
        ]),
    );
    let mut steps = vec![
        step("部分和をS_nとおく。S₀=0、S₁=a₁であり、隣り合う部分和の差はa_(n+1)である。", Some(Equation::new(Expr::term("S", 0), sum(0))), "部分和の定義と空和の境界"),
        step("添字を1つ進めた式と元の式の差をとる。非同次項の隣接差は定数なので、総和が消える。", Some(scalar_recurrence), "S_(n+1)−S_n=a_(n+1)。この導出はn≥2で使う"),
        step(&format!("元の部分和の式をn=1で使うとa₂={}となる。この値は上の漸化式にa₁を入れた値とも一致する。", a2), None, "n=1の境界を別に確認して漸化式を全てのn≥1へ延長"),
        step(&format!("定数{}を引けば公比{}の等比数列となる。初項からその係数を定める。", u, r), None, "定数の特解と等比数列"),
        step("部分和を先に解く方法でも同じ一般項が得られる。", Some(Equation::new(Expr::term("S", 0), partial_formula)), "部分和の一次漸化式とS₁"),
        step("S_n−S_(n−1)をとる。n=1でもS₀=0であるため、次の式が成立する。", Some(Equation::new(Expr::term("a", 0), formula.clone())), "部分和の差と初期境界"),
    ];
    steps.push(step("初期条件と元の部分和の漸化式を全ての添字で満たすことをLeanで検証する。部分和と各項は一意に決まる。", None, "部分和の一意性と隣接差"));
    let operations = if u.is_zero() {
        vec!["eliminate_sum".into(), "geometric".into()]
    } else {
        vec![
            "eliminate_sum".into(),
            "fixed_point".into(),
            "geometric".into(),
        ]
    };
    Ok(SumRoute {
        score: 6,
        reason: "部分和を求める、または差で総和を消す".into(),
        operations,
        discovery: if u.is_zero() { 1 } else { 2 },
        domain: 0,
        degree: if u.is_zero() { 0 } else { 1 },
        intermediate_values: vec![initial, a2, r],
        formula,
        derivation: DerivationIR {
            method_name: "部分和の差で総和を消す".into(),
            hint: "隣接する2式の差をとり、S_(n+1)−S_n=a_(n+1)を使う。n=1の境界も確認する。".into(),
            auxiliaries: Vec::new(),
            steps,
            alternatives: vec!["部分和S_nの漸化式を先に解き、a_n=S_n−S_(n−1)で戻す。".into()],
        },
    })
}

pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    if !matches!(
        problem.kind,
        RecurrenceKind::PureSum | RecurrenceKind::SumRelation
    ) {
        return Ok(None);
    }
    let route = forward(problem)?;
    let operations = route
        .operations
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let classification = crate::generator::make_classification(
        problem,
        route.score,
        &route.derivation.method_name,
        &route.reason,
        &operations,
        route.discovery,
        route.domain,
        route.degree,
    )?;
    crate::generator::include_intermediate_values(
        problem,
        classification,
        &route.intermediate_values,
    )
    .map(Some)
}

pub fn explain(
    problem: &ProblemIR,
    classification: &Classification,
) -> Result<Option<DerivationIR>, String> {
    if !matches!(
        classification.method_name.as_str(),
        "部分和の差で総和を消す"
            | "部分和と項の関係から総和を消す"
            | "部分和を正規化して3項間を解く"
    ) {
        return Ok(None);
    }
    Ok(Some(forward(problem)?.derivation))
}

fn relation_route(problem: &ProblemIR) -> Result<SumRoute, String> {
    if problem.recurrence.lhs != sum(0) {
        return Err("総和と項の関係が登録形ではありません。".into());
    }
    let (alpha, forcing) = split(&problem.recurrence.rhs, &Expr::term("a", 0))?;
    let alpha = constant(&alpha)?;
    if alpha.is_one() {
        return Err("総和と項の係数が1で一意性を保証できません。".into());
    }
    let f = polynomial(&forcing)?;
    if f.keys().any(|degree| *degree > 2) {
        return Err("総和関係の非同次項は高々二次式です。".into());
    }
    let den = alpha.sub(&Rational::integer(1));
    let r = alpha.div(&den)?;
    if r.is_zero() || r.is_one() {
        return Err("総和関係の差から得られる公比が退化しています。".into());
    }
    let forcing_slope = Rational::integer(-2).mul(&coeff(&f, 2)).div(&den)?;
    let forcing_intercept = Rational::integer(-1)
        .mul(&coeff(&f, 2).add(&coeff(&f, 1)))
        .div(&den)?;
    // a_(n+1) = r a_n + F(n), with a linear particular solution u*n+v.
    let u = forcing_slope.div(&Rational::integer(1).sub(&r))?;
    let v = forcing_intercept
        .sub(&u)
        .div(&Rational::integer(1).sub(&r))?;
    let first = initial_value(problem, 0)?;
    let forcing_at_one = coeff(&f, 0).add(&coeff(&f, 1)).add(&coeff(&f, 2));
    if first != alpha.mul(&first).add(&forcing_at_one) {
        return Err("n=1の部分和の関係と初期条件が一致しません。".into());
    }
    let amplitude = first.sub(&u).sub(&v);
    // sum_relation retains its scalar route's intermediate values in the
    // reference. Recover these from the relation and initials, not its answer.
    let intermediate_values = if u.is_zero() {
        vec![
            Rational::integer(0),
            r.clone(),
            forcing_intercept.clone(),
            first.clone(),
            v.clone(),
            first.sub(&v),
        ]
    } else {
        vec![
            r.clone(),
            forcing_slope.clone(),
            forcing_intercept.clone(),
            u.clone(),
            v.clone(),
            amplitude.clone(),
        ]
    };
    let h = Expr::add(vec![
        Expr::mul(vec![Expr::rational(u.clone()), Expr::index()]),
        Expr::rational(v.clone()),
    ]);
    let formula = Expr::add(vec![geometric(amplitude.clone(), &r), h.clone()]);
    let scalar_rhs = Expr::add(vec![
        Expr::mul(vec![Expr::rational(r.clone()), Expr::term("a", 0)]),
        from_poly(&BTreeMap::from([
            (1, forcing_slope),
            (0, forcing_intercept),
        ])),
    ]);
    let definition = Equation::new(Expr::term("b", 0), Expr::sub(Expr::term("a", 0), h));
    let binit = Equation::new(
        Expr::Term {
            sequence: "b".into(),
            index: NatExpr::constant(1),
        },
        Expr::rational(amplitude),
    );
    let brec = Equation::new(
        Expr::term("b", 1),
        Expr::mul(vec![Expr::rational(r.clone()), Expr::term("b", 0)]),
    );
    let steps = vec![
        step("S_n=Σa_kとおく。この問題は部分和S_nと項a_nの両方を含む関係式である。", Some(Equation::new(Expr::term("S", 0), sum(0))), "部分和の定義"),
        step("添字を1つ進めた式から元の式を引くと、左辺はa_(n+1)となる。α≠1なのでその係数で割って総和を消す。", Some(Equation::new(Expr::term("a", 1), scalar_rhs)), "S_(n+1)−S_n=a_(n+1)と多項式の差"),
        step(&format!("非同次項を消す一次式の特解は{}n+{}である。これを引く補助数列を定義する。", u, v), Some(definition.clone()), "一次式の係数比較"),
        step("補助数列は等比数列となる。", Some(brec.clone()), "特解を引いて非同次項を消す"),
        step("初期条件で等比数列の係数を定め、元の数列へ戻す。", Some(Equation::new(Expr::term("a", 0), formula.clone())), "初項と逆変換"),
        step("n=1の部分和S₁=a₁も元の関係式を満たす。一般項・全添字の総和の関係・一意性をLeanで検証する。", None, "部分和の初期境界とα≠1による一意性"),
    ];
    Ok(SumRoute {
        score: 6,
        reason: "隣接する式の差で総和を消す".into(),
        operations: vec![
            "eliminate_sum".into(),
            if u.is_zero() {
                "fixed_point".into()
            } else {
                "polynomial_shift".into()
            },
            "geometric".into(),
        ],
        discovery: 2,
        domain: 0,
        degree: if u.is_zero() { 0 } else { 1 },
        intermediate_values,
        formula,
        derivation: DerivationIR {
            method_name: "部分和と項の関係から総和を消す".into(),
            hint: "n+1の式からnの式を引く。部分和の差がa_(n+1)になることを使う。".into(),
            auxiliaries: vec![AuxiliarySequence {
                name: "b".into(),
                definition,
                initial: binit,
                recurrence: brec,
            }],
            steps,
            alternatives: Vec::new(),
        },
    })
}

fn scaled_route(problem: &ProblemIR) -> Result<SumRoute, String> {
    let (lead, leftover) = split(&problem.recurrence.lhs, &sum(2))?;
    if leftover != Expr::integer(0) {
        return Err("部分和の3項間の左辺が登録形ではありません。".into());
    }
    let (next_coefficient, rest) = split(&problem.recurrence.rhs, &sum(1))?;
    let (current_coefficient, forcing) = split(&rest, &sum(0))?;
    if forcing != Expr::integer(0) {
        return Err("部分和の3項間に未登録の非同次項があります。".into());
    }
    let mut found = None;
    for profile in [0, 1, 5] {
        let g0 = scale(profile, 0);
        let g1 = scale(profile, 1);
        let g2 = scale(profile, 2);
        if polynomial(&lead)? != polynomial(&Expr::mul(vec![g0.clone(), g1.clone()]))? {
            continue;
        }
        for r in 2..=5 {
            for s in (r + 1)..=5 {
                let r = Rational::integer(r);
                let s = Rational::integer(s);
                if polynomial(&next_coefficient)?
                    == polynomial(&Expr::mul(vec![
                        Expr::rational(r.add(&s)),
                        g0.clone(),
                        g2.clone(),
                    ]))?
                    && polynomial(&current_coefficient)?
                        == polynomial(&Expr::mul(vec![
                            Expr::rational(Rational::integer(-1).mul(&r).mul(&s)),
                            g1.clone(),
                            g2.clone(),
                        ]))?
                {
                    found = Some((profile, r, s));
                }
            }
        }
    }
    let (profile, r, s) = found.ok_or("部分和の正規化と異なる特性根を検出できません。")?;
    let first = initial_value(problem, 0)?
        .div(&scale(profile, 0).evaluate(1, &|_, _| Err("数列項はありません。".into()))?)?;
    let second = initial_value(problem, 1)?
        .div(&scale(profile, 0).evaluate(2, &|_, _| Err("数列項はありません。".into()))?)?;
    let x = second.sub(&s.mul(&first)).div(&r.sub(&s))?;
    let y = first.sub(&x);
    let gprev = scale(profile, -1).evaluate(1, &|_, _| Err("数列項はありません。".into()))?;
    if !gprev.mul(&x.div(&r)?.add(&y.div(&s)?)).is_zero() {
        return Err("正規化して解いた部分和はS₀=0を満たしません。".into());
    }
    let partial_formula = Expr::mul(vec![
        scale(profile, 0),
        Expr::add(vec![geometric(x.clone(), &r), geometric(y.clone(), &s)]),
    ]);
    let prev = Expr::mul(vec![
        scale(profile, -1),
        Expr::add(vec![geometric(x.div(&r)?, &r), geometric(y.div(&s)?, &s)]),
    ]);
    let formula = Expr::sub(partial_formula.clone(), prev);
    let definition = Equation::new(Expr::term("b", 0), Expr::div(sum(0), scale(profile, 0)));
    let binit = Equation::new(
        Expr::Term {
            sequence: "b".into(),
            index: NatExpr::constant(1),
        },
        Expr::rational(first),
    );
    let brec = Equation::new(
        Expr::term("b", 2),
        Expr::add(vec![
            Expr::mul(vec![Expr::rational(r.add(&s)), Expr::term("b", 1)]),
            Expr::mul(vec![
                Expr::rational(Rational::integer(-1).mul(&r).mul(&s)),
                Expr::term("b", 0),
            ]),
        ]),
    );
    let steps = vec![
        step("部分和S_nを導入する。元の問題はS_nの3項間漸化式であり、2つの初期条件も部分和の値である。", Some(Equation::new(Expr::term("S", 0), sum(0))), "部分和とその初期条件"),
        step("n≥1で倍率g_nは正である。S_nを倍率g_nで割った補助数列を考える。", Some(definition.clone()), "正規化の分母が零でないことを確認"),
        step("倍率を相殺すると、補助数列は定数係数の3項間漸化式を満たす。", Some(brec.clone()), "3つの倍率を整理して正規化"),
        step(&format!("特性方程式の異なる2根は{}、{}である。S₁/g₁とS₂/g₂から2つの係数を定める。", r, s), Some(binit.clone()), "特性方程式と2つの初期条件"),
        step("倍率を掛け戻して部分和を求める。", Some(Equation::new(Expr::term("S", 0), partial_formula)), "S_n=g_n b_n"),
        step("a_n=S_n−S_(n−1)で各項を求める。n=1ではS₀=0を使うため同じ式が成立する。", Some(Equation::new(Expr::term("a", 0), formula.clone())), "部分和の差、指数を下げる際には1/rと1/sで表す"),
        step("部分和の境界S₀=0、2つの部分和の初期値、全添字の漸化式、各項の一意性をLeanで検証する。", None, "正規化した3項間と部分和の差の一意性"),
    ];
    Ok(SumRoute {
        score: 20,
        reason: "部分和の正規化と3項間と差を組み合わせる".into(),
        operations: vec![
            "index_scale".into(),
            "characteristic_distinct".into(),
            "recover_sum".into(),
        ],
        discovery: 3,
        domain: 1,
        degree: 2,
        // The reference outer partial-sum route measures its supplied sum
        // initials; these already occur in the question's visible constants.
        intermediate_values: Vec::new(),
        formula,
        derivation: DerivationIR {
            method_name: "部分和を正規化して3項間を解く".into(),
            hint: "部分和S_n自体を数列と見て正規化し、3項間を解いてから隣接差でa_nに戻す。".into(),
            auxiliaries: vec![AuxiliarySequence {
                name: "b".into(),
                definition,
                initial: binit,
                recurrence: brec,
            }],
            steps,
            alternatives: Vec::new(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    #[test]
    fn partial_sum_routes_recover_terms_and_include_first_boundary() {
        for family in ["pure_sum", "pure_sum_scaled", "sum_relation"] {
            for seed in 0..24 {
                let mut rng = ChaCha8Rng::seed_from_u64(seed);
                let recipe =
                    build_recipe(family, seed, sample(family, 2, &mut rng).unwrap()).unwrap();
                let p = compile(&recipe).unwrap();
                let route = forward(&p).unwrap();
                let value = |_: &str, k| {
                    p.general_term
                        .rhs
                        .evaluate(k, &|_, _| Err("一般項に数列項があります。".into()))
                };
                for n in 1..=8 {
                    assert_eq!(
                        p.recurrence.lhs.evaluate(n, &value).unwrap(),
                        p.recurrence.rhs.evaluate(n, &value).unwrap(),
                        "{family} seed {seed} n {n}"
                    );
                    assert_eq!(
                        route.formula.evaluate(n, &value).unwrap(),
                        p.general_term.rhs.evaluate(n, &value).unwrap()
                    );
                }
                for initial in &p.initials {
                    assert_eq!(
                        initial.lhs.evaluate(1, &value).unwrap(),
                        initial.rhs.evaluate(1, &value).unwrap()
                    );
                }
                let mut poisoned = p.clone();
                poisoned.general_term.rhs = Expr::integer(987_654);
                let recovered = forward(&poisoned).unwrap();
                assert_eq!(route.formula, recovered.formula);
                assert_eq!(route.score, recovered.score);
                assert_eq!(route.intermediate_values, recovered.intermediate_values);
            }
        }
    }

    #[test]
    fn invalid_sum_zero_boundary_and_relation_boundary_are_rejected() {
        let p = BTreeMap::from([
            ("c".into(), Rational::integer(1)),
            ("d".into(), Rational::integer(2)),
            ("r".into(), Rational::integer(2)),
            ("s".into(), Rational::integer(3)),
            ("profile".into(), Rational::integer(1)),
        ]);
        assert!(validate_parameters("pure_sum_scaled", &p).is_err());
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let recipe = build_recipe(
            "sum_relation",
            1,
            sample("sum_relation", 2, &mut rng).unwrap(),
        )
        .unwrap();
        let mut p = compile(&recipe).unwrap();
        p.initials[0].rhs = Expr::integer(0);
        assert!(forward(&p).is_err());
    }

    #[test]
    #[ignore = "Runs the installed Lean 4.19 checker against representative sum profiles"]
    fn lean_checks_all_sum_profiles() {
        use std::{fs, process::Command};
        for family in ["pure_sum", "pure_sum_scaled", "sum_relation"] {
            for profile in if family == "pure_sum_scaled" {
                vec![0, 1, 5]
            } else {
                vec![0, 1]
            } {
                let mut p = BTreeMap::from([
                    ("c".into(), Rational::integer(2)),
                    ("d".into(), Rational::integer(3)),
                    ("r".into(), Rational::integer(2)),
                    ("profile".into(), Rational::integer(profile)),
                ]);
                if family == "pure_sum_scaled" {
                    p.insert("s".into(), Rational::integer(3));
                    if profile != 0 {
                        p.insert("c".into(), Rational::fraction(-9, 2));
                    }
                }
                let recipe = build_recipe(family, 10, p).unwrap();
                let problem = crate::generator::replay(&recipe).unwrap();
                let source = crate::proofs::sums::source(&problem).unwrap().unwrap();
                let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("lean");
                let path = root
                    .join("generated")
                    .join(format!("sums-profile-{family}-{profile}.lean"));
                fs::write(&path, source).unwrap();
                let output = Command::new(
                    std::path::PathBuf::from(std::env::var_os("HOME").unwrap())
                        .join(".elan/bin/lake"),
                )
                .current_dir(&root)
                .args(["env", "lean"])
                .arg(&path)
                .output()
                .unwrap();
                assert!(
                    output.status.success(),
                    "{family} profile {profile}: {} {}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                assert!(!String::from_utf8_lossy(&output.stdout).contains("sorryAx"));
            }
        }
    }
}
