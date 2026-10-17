//! Rational boundary cases, detected from the displayed question.
use crate::generator::{
    Classification, DerivationIR, DerivationStep, FamilyInfo, ProblemIR, Recipe, RecipeCore,
    RecurrenceKind,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};
use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::{BTreeMap, BTreeSet};
type Params = BTreeMap<String, Rational>;
fn z() -> Rational {
    Rational::integer(0)
}
fn one() -> Rational {
    Rational::integer(1)
}
fn neg(x: &Rational) -> Rational {
    z().sub(x)
}
fn rat(x: &Rational) -> Expr {
    Expr::rational(x.clone())
}
fn map(items: &[(&str, Rational)]) -> Params {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}
fn geom(c: &Rational, r: &Rational) -> Expr {
    Expr::mul(vec![rat(c), Expr::pow(rat(r), NatExpr::offset(-1))])
}
fn initial(n: u32, v: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: "a".into(),
            index: NatExpr::constant(n),
        },
        rat(&v),
    )
}
#[derive(Clone, Debug)]
pub struct VariantInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub level: u8,
}
pub fn variants(family: &str) -> Vec<VariantInfo> {
    let rows = match family {
        "cases_constant" => vec![("zero", "零数列", 1), ("nonzero", "零以外の定数数列", 1)],
        "cases_reset" => vec![("initial_exception", "初項だけが異なる定数列", 1)],
        "cases_arithmetic" => vec![("negative_difference", "負の公差", 1)],
        "cases_geometric" => vec![
            ("negative_integer", "負の整数公比", 1),
            ("proper_positive", "0より大きく1未満の公比", 1),
            ("proper_negative", "−1より大きく0未満の公比", 1),
            ("minus_one", "公比−1", 1),
            ("positive_fraction", "1より大きい非整数公比", 1),
        ],
        "cases_affine_fixed" => vec![("fixed_initial", "初項が不動点", 1)],
        "cases_affine" => vec![
            ("negative_multiplier", "負の倍率", 2),
            ("proper_multiplier", "0より大きく1未満の倍率", 2),
            ("positive_fraction", "1より大きい非整数倍率", 2),
        ],
        "cases_second_distinct" => vec![
            ("opposite_signs", "異符号の2根", 3),
            ("both_negative", "異なる2負根", 3),
            ("rational_roots", "非整数有理数の根", 3),
        ],
        "cases_second_repeated" => vec![
            ("root_one", "重根1・同次", 2),
            ("root_positive", "1以外の正の重根・同次", 3),
            ("root_negative", "負の重根・同次", 3),
            ("root_fraction", "分数の重根・同次", 3),
        ],
        "cases_second_forced" => vec![
            ("root_one", "重根1・定数項あり", 2),
            ("root_other", "1以外の重根・定数項あり", 4),
            ("distinct", "異なる2根・定数項あり", 4),
        ],
        "cases_second_zero" => vec![
            ("one_zero", "一方が零根・次数低下", 2),
            ("double_zero", "重根0・初めの2項が例外", 2),
        ],
        "cases_second_single_mode" => vec![
            ("distinct_drop", "異なる根の一方の成分が消える", 2),
            ("repeated_drop", "重根のn倍の成分が消える", 2),
        ],
        _ => vec![],
    };
    rows.into_iter()
        .map(|(id, label, level)| VariantInfo { id, label, level })
        .collect()
}
pub fn variant_id(recipe: &Recipe) -> Option<String> {
    variant_name(&recipe.family, &recipe.parameters)
        .ok()
        .map(str::to_string)
}
pub fn catalog() -> Vec<FamilyInfo> {
    [
        ("cases_constant", "定数数列"),
        ("cases_reset", "初項例外と倍率0"),
        ("cases_arithmetic", "負の公差の等差数列"),
        ("cases_geometric", "符号・分数公比の等比数列"),
        ("cases_affine_fixed", "不動点から始まる一次漸化式"),
        ("cases_affine", "符号・分数倍率の一次漸化式"),
        ("cases_second_distinct", "符号・有理数根の3項間"),
        ("cases_second_repeated", "同次3項間の重根"),
        ("cases_second_forced", "定数項と特性根の分岐"),
        ("cases_second_zero", "零根による次数低下"),
        ("cases_second_single_mode", "初期条件による成分の消失"),
    ]
    .into_iter()
    .map(|(id, label)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels: variants(id)
            .iter()
            .map(|v| v.level)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    })
    .collect()
}
pub fn supports(family: &str) -> bool {
    !variants(family).is_empty()
}
// Sampling is uniform over distinct reduced rationals, rather than pairs.
fn pool() -> Vec<Rational> {
    (-6..=6)
        .flat_map(|n| (1..=4).map(move |d| Rational::fraction(n, d)))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
fn choose(rng: &mut ChaCha8Rng, predicate: impl Fn(&Rational) -> bool) -> Rational {
    let values: Vec<_> = pool().into_iter().filter(predicate).collect();
    values[rng.gen_range(0..values.len())].clone()
}
pub fn sample(family: &str, level: u8, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    let available: Vec<_> = variants(family)
        .into_iter()
        .filter(|v| v.level == level)
        .collect();
    if available.is_empty() {
        return Err("指定された型・難易度の分岐がありません。".into());
    }
    sample_variant(family, available[rng.gen_range(0..available.len())].id, rng)
}
pub fn sample_variant(family: &str, variant: &str, rng: &mut ChaCha8Rng) -> Result<Params, String> {
    let index = variants(family)
        .iter()
        .position(|v| v.id == variant)
        .ok_or("未登録の細分類です。")?;
    let c = choose(rng, |v| !v.is_zero());
    let d = choose(rng, |v| !v.is_zero());
    let mut p = if matches!(
        family,
        "cases_constant"
            | "cases_reset"
            | "cases_arithmetic"
            | "cases_geometric"
            | "cases_affine_fixed"
            | "cases_affine"
    ) {
        let (r, q, first) = match family {
            "cases_constant" => (one(), z(), if variant == "zero" { z() } else { c }),
            "cases_reset" => {
                let first = if c == d { c.add(&one()) } else { c };
                (z(), d, first)
            }
            "cases_arithmetic" => (one(), choose(rng, |v| v < &z()), c),
            "cases_geometric" => {
                let r = match variant {
                    "negative_integer" => Rational::integer(-rng.gen_range(2..=5)),
                    "proper_positive" => choose(rng, |v| v.is_positive() && v < &one()),
                    "proper_negative" => choose(rng, |v| v < &z() && v > &Rational::integer(-1)),
                    "minus_one" => Rational::integer(-1),
                    _ => choose(rng, |v| v > &one() && !v.0.is_integer()),
                };
                (r, z(), c)
            }
            "cases_affine_fixed" => {
                let r = choose(rng, |v| !v.is_zero() && !v.is_one());
                (r.clone(), one().sub(&r).mul(&c), c)
            }
            _ => {
                let r = match variant {
                    "negative_multiplier" => choose(rng, |v| v < &z()),
                    "proper_multiplier" => choose(rng, |v| v.is_positive() && v < &one()),
                    _ => choose(rng, |v| v > &one() && !v.0.is_integer()),
                };
                let q = one().sub(&r).mul(&d);
                let first = if c == d { c.add(&one()) } else { c };
                (r, q, first)
            }
        };
        map(&[("p", r), ("q", q), ("initial", first)])
    } else {
        let (r, s, a, b, h, f) = match family {
            "cases_second_distinct" => {
                let r = match variant {
                    "opposite_signs" | "both_negative" => choose(rng, |v| v < &z()),
                    _ => choose(rng, |v| !v.is_zero() && !v.is_one() && !v.0.is_integer()),
                };
                let s = match variant {
                    "both_negative" => choose(rng, |v| v < &z() && v != &r),
                    "opposite_signs" => choose(rng, |v| v > &one()),
                    _ => choose(rng, |v| !v.is_zero() && !v.is_one() && v != &r),
                };
                (r, s, c, d, z(), z())
            }
            "cases_second_repeated" => {
                let r = match variant {
                    "root_one" => one(),
                    "root_negative" => choose(rng, |v| v < &z()),
                    "root_fraction" => choose(rng, |v| !v.is_zero() && !v.0.is_integer()),
                    _ => choose(rng, |v| v > &one()),
                };
                (r.clone(), r, c, d, z(), z())
            }
            "cases_second_forced" => {
                let r = if variant == "root_one" {
                    one()
                } else {
                    choose(rng, |v| !v.is_zero() && !v.is_one())
                };
                let s = if variant == "distinct" {
                    choose(rng, |v| !v.is_zero() && !v.is_one() && v != &r)
                } else {
                    r.clone()
                };
                if variant == "root_one" {
                    (r, s, c, d, z(), choose(rng, |v| !v.is_zero()))
                } else {
                    let f = one().sub(&r).mul(&one().sub(&s)).mul(&d);
                    (r, s, c, choose(rng, |v| !v.is_zero()), d, f)
                }
            }
            "cases_second_zero" => {
                if variant == "double_zero" {
                    (z(), z(), c, d, z(), z())
                } else {
                    (
                        z(),
                        choose(rng, |v| !v.is_zero() && !v.is_one()),
                        c,
                        d,
                        z(),
                        z(),
                    )
                }
            }
            "cases_second_single_mode" => {
                let r = choose(rng, |v| !v.is_zero() && !v.is_one());
                let s = if variant == "repeated_drop" {
                    r.clone()
                } else {
                    choose(rng, |v| !v.is_zero() && !v.is_one() && v != &r)
                };
                (r, s, c, z(), z(), z())
            }
            _ => return Err("未登録の基本分岐です。".into()),
        };
        map(&[
            ("r", r),
            ("s", s),
            ("a", a),
            ("b", b),
            ("shift", h),
            ("forcing", f),
        ])
    };
    p.insert("variant".into(), Rational::integer(index as i64));
    validate_parameters(family, &p)?;
    Ok(p)
}
fn variant_name(family: &str, p: &Params) -> Result<&'static str, String> {
    let index = p
        .get("variant")
        .and_then(Rational::integer_u32)
        .ok_or("細分類番号がありません。")?;
    variants(family)
        .get(index as usize)
        .map(|v| v.id)
        .ok_or("細分類番号が範囲外です。".into())
}
pub fn validate_parameters(family: &str, p: &Params) -> Result<(), String> {
    let variant = variant_name(family, p)?;
    let first_order = matches!(
        family,
        "cases_constant"
            | "cases_reset"
            | "cases_arithmetic"
            | "cases_geometric"
            | "cases_affine_fixed"
            | "cases_affine"
    );
    let names = if first_order {
        vec!["p", "q", "initial", "variant"]
    } else {
        vec!["r", "s", "a", "b", "shift", "forcing", "variant"]
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.into_iter().collect() {
        return Err("基本分岐Recipeのパラメータ集合が一致しません。".into());
    }
    if p.values()
        .any(|v| v.0.numer().abs() > BigInt::from(4096) || v.0.denom() > &BigInt::from(256))
    {
        return Err("分岐パラメータの数値範囲を超えています。".into());
    }
    let valid = if first_order {
        let (r, q, c) = (&p["p"], &p["q"], &p["initial"]);
        match family {
            "cases_constant" => {
                r.is_one()
                    && q.is_zero()
                    && (if variant == "zero" {
                        c.is_zero()
                    } else {
                        !c.is_zero()
                    })
            }
            "cases_reset" => r.is_zero() && c != q,
            "cases_arithmetic" => r.is_one() && q < &z(),
            "cases_geometric" => {
                q.is_zero()
                    && !c.is_zero()
                    && match variant {
                        "negative_integer" => r < &Rational::integer(-1) && r.0.is_integer(),
                        "proper_positive" => r.is_positive() && r < &one(),
                        "proper_negative" => r < &z() && r > &Rational::integer(-1),
                        "minus_one" => r == &Rational::integer(-1),
                        _ => r > &one() && !r.0.is_integer(),
                    }
            }
            "cases_affine_fixed" => {
                !r.is_zero() && !r.is_one() && !q.is_zero() && r.mul(c).add(q) == *c
            }
            _ => {
                !q.is_zero()
                    && r.mul(c).add(q) != *c
                    && match variant {
                        "negative_multiplier" => r < &z(),
                        "proper_multiplier" => r.is_positive() && r < &one(),
                        _ => r > &one() && !r.0.is_integer(),
                    }
            }
        }
    } else {
        let (r, s, a, b, h, f) = (
            &p["r"],
            &p["s"],
            &p["a"],
            &p["b"],
            &p["shift"],
            &p["forcing"],
        );
        let normal = !r.is_zero() && !s.is_zero();
        let forcing_consistent = if r.is_one() && s.is_one() {
            h.is_zero()
        } else {
            *f == one().sub(r).mul(&one().sub(s)).mul(h)
        };
        forcing_consistent
            && match family {
                "cases_second_distinct" => {
                    normal
                        && r != s
                        && !r.is_one()
                        && !s.is_one()
                        && !a.is_zero()
                        && !b.is_zero()
                        && h.is_zero()
                        && f.is_zero()
                        && match variant {
                            "opposite_signs" => r < &z() && s > &z(),
                            "both_negative" => r < &z() && s < &z(),
                            _ => !r.0.is_integer() || !s.0.is_integer(),
                        }
                }
                "cases_second_repeated" => {
                    normal
                        && r == s
                        && !b.is_zero()
                        && h.is_zero()
                        && f.is_zero()
                        && match variant {
                            "root_one" => r.is_one(),
                            "root_positive" => r > &one(),
                            "root_negative" => r < &z(),
                            _ => !r.0.is_integer(),
                        }
                }
                "cases_second_forced" => {
                    normal
                        && !a.is_zero()
                        && !b.is_zero()
                        && !f.is_zero()
                        && match variant {
                            "root_one" => r.is_one() && s.is_one(),
                            "root_other" => r == s && !r.is_one(),
                            _ => r != s && !r.is_one() && !s.is_one(),
                        }
                }
                "cases_second_zero" => {
                    r.is_zero()
                        && h.is_zero()
                        && f.is_zero()
                        && !a.is_zero()
                        && !b.is_zero()
                        && (if variant == "double_zero" {
                            s.is_zero()
                        } else {
                            !s.is_zero() && !s.is_one()
                        })
                }
                "cases_second_single_mode" => {
                    normal
                        && !r.is_one()
                        && !s.is_one()
                        && !a.is_zero()
                        && b.is_zero()
                        && h.is_zero()
                        && f.is_zero()
                        && (if variant == "repeated_drop" {
                            r == s
                        } else {
                            r != s
                        })
                }
                _ => false,
            }
    };
    if valid {
        Ok(())
    } else {
        Err("パラメータが選択された数学的分岐を満たしていません。".into())
    }
}
pub fn build_recipe(family: &str, seed: u64, p: Params) -> Result<Recipe, String> {
    validate_parameters(family, &p)?;
    Ok(Recipe {
        schema_version: crate::generator::SCHEMA_VERSION.into(),
        rules_version: crate::generator::CURRENT_RULES_VERSION.into(),
        rng_version: crate::generator::RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p.clone(),
        core: RecipeCore {
            id: "core".into(),
            kind: family.into(),
            parameters: p,
        },
        blocks: vec![],
        output: "core".into(),
        index_start: 1,
    })
}
fn first_formula(p: &Rational, q: &Rational, c: &Rational) -> Result<Expr, String> {
    if p.is_one() {
        Ok(Expr::add(vec![
            rat(c),
            Expr::mul(vec![rat(q), Expr::offset(-1)]),
        ]))
    } else {
        let h = q.div(&one().sub(p))?;
        Ok(Expr::add(vec![rat(&h), geom(&c.sub(&h), p)]))
    }
}
fn repeated_formula(r: &Rational, a: &Rational, b: &Rational, h: &Rational, f: &Rational) -> Expr {
    let k = Expr::offset(-1);
    if r.is_zero() {
        Expr::add(vec![
            rat(f),
            geom(&a.sub(f), r),
            Expr::mul(vec![
                rat(&b.sub(f)),
                k,
                Expr::pow(Expr::integer(0), NatExpr::offset(-2)),
            ]),
        ])
    } else {
        let base = Expr::mul(vec![
            Expr::add(vec![rat(a), Expr::mul(vec![rat(b), k.clone()])]),
            Expr::pow(rat(r), NatExpr::offset(-1)),
        ]);
        let particular = if r.is_one() {
            Expr::mul(vec![
                rat(&f.div(&Rational::integer(2)).expect("two nonzero")),
                k.clone(),
                Expr::sub(k, Expr::integer(1)),
            ])
        } else {
            rat(h)
        };
        Expr::add(vec![base, particular])
    }
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    validate_parameters(&recipe.family, &recipe.parameters)?;
    let p = &recipe.parameters;
    let first_order = p.contains_key("initial");
    let (rhs, formula, kind) = if first_order {
        (
            Expr::add(vec![
                Expr::mul(vec![rat(&p["p"]), Expr::term("a", 0)]),
                rat(&p["q"]),
            ]),
            first_formula(&p["p"], &p["q"], &p["initial"])?,
            RecurrenceKind::FirstOrder,
        )
    } else {
        let (r, s) = (&p["r"], &p["s"]);
        let rhs = Expr::add(vec![
            Expr::mul(vec![rat(&r.add(s)), Expr::term("a", 1)]),
            Expr::mul(vec![rat(&neg(&r.mul(s))), Expr::term("a", 0)]),
            rat(&p["forcing"]),
        ]);
        let formula = if r == s {
            repeated_formula(r, &p["a"], &p["b"], &p["shift"], &p["forcing"])
        } else {
            Expr::add(vec![geom(&p["a"], r), geom(&p["b"], s), rat(&p["shift"])])
        };
        (rhs, formula, RecurrenceKind::SecondOrder)
    };
    let initials = (1..=if first_order { 1 } else { 2 })
        .map(|n| {
            formula
                .evaluate(n, &|_, _| Err("一般項に数列項があります。".into()))
                .map(|v| initial(n, v))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ProblemIR {
        kind,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(Expr::term("a", if first_order { 1 } else { 2 }), rhs),
        initials,
        general_term: Equation::new(Expr::term("a", 0), formula),
        conditions: vec![],
        secondary_recurrences: vec![],
        secondary_general_terms: vec![],
    })
}
// Linear decomposition over {1,a_n,a_{n+1},a_{n+2}}.
fn linear(e: &Expr) -> Option<[Rational; 4]> {
    let mut out = [z(), z(), z(), z()];
    match e {
        Expr::Rational { value } => out[0] = value.clone(),
        Expr::Term { sequence, index } if sequence == "a" => {
            let j = (0..=2).find(|j| *index == NatExpr::offset(*j))?;
            out[(j + 1) as usize] = one();
        }
        Expr::Add { terms } => {
            for term in terms {
                let v = linear(term)?;
                for j in 0..4 {
                    out[j] = out[j].add(&v[j]);
                }
            }
        }
        Expr::Mul { factors } => {
            out[0] = one();
            for factor in factors {
                let v = linear(factor)?;
                if out[1..].iter().any(|x| !x.is_zero()) && v[1..].iter().any(|x| !x.is_zero()) {
                    return None;
                }
                let old = out.clone();
                out[0] = old[0].mul(&v[0]);
                for j in 1..4 {
                    out[j] = old[j].mul(&v[0]).add(&v[j].mul(&old[0]));
                }
            }
        }
        Expr::Div {
            numerator,
            denominator,
        } => {
            let den = linear(denominator)?;
            if den[0].is_zero() || den[1..].iter().any(|x| !x.is_zero()) {
                return None;
            }
            let n = linear(numerator)?;
            for j in 0..4 {
                out[j] = n[j].div(&den[0]).ok()?;
            }
        }
        _ => return None,
    }
    Some(out)
}
fn sqrt_integer(n: &BigInt) -> Option<BigInt> {
    if n.is_negative() {
        return None;
    }
    if n.is_zero() {
        return Some(BigInt::zero());
    }
    let two = BigInt::from(2u8);
    let mut x = n.clone();
    let mut y = (&x + BigInt::one()) / &two;
    while y < x {
        x = y;
        y = (&x + n / &x) / &two;
    }
    if &x * &x == *n {
        Some(x)
    } else {
        None
    }
}
fn sqrt_rational(x: &Rational) -> Option<Rational> {
    Some(Rational(num_rational::BigRational::new(
        sqrt_integer(x.0.numer())?,
        sqrt_integer(x.0.denom())?,
    )))
}
fn root_pair(p: &Rational, q: &Rational) -> Option<(Rational, Rational)> {
    let d = sqrt_rational(&p.mul(p).add(&Rational::integer(4).mul(q)))?;
    Some((
        p.sub(&d).div(&Rational::integer(2)).ok()?,
        p.add(&d).div(&Rational::integer(2)).ok()?,
    ))
}
#[derive(Clone, Debug)]
pub enum Route {
    Constant {
        value: Rational,
    },
    First {
        p: Rational,
        q: Rational,
        initial: Rational,
    },
    Reduced {
        p: Rational,
        q: Rational,
        initial: Rational,
    },
    Zero {
        p: Rational,
        first: Rational,
        second: Rational,
    },
    Repeated {
        r: Rational,
        a: Rational,
        b: Rational,
        shift: Rational,
        forcing: Rational,
    },
    Distinct {
        r: Rational,
        s: Rational,
        a: Rational,
        b: Rational,
        shift: Rational,
        forcing: Rational,
    },
}
#[derive(Clone, Debug)]
pub struct Detected {
    pub route: Route,
    pub formula: Expr,
}
fn initial_value(problem: &ProblemIR, n: u32) -> Option<Rational> {
    let e = problem
        .initials
        .iter()
        .find(|e| e.lhs == initial(n, z()).lhs)?;
    if let Expr::Rational { value } = &e.rhs {
        Some(value.clone())
    } else {
        None
    }
}
/// Irrational and nonreal roots are declined explicitly, never guessed.
pub fn detect(problem: &ProblemIR) -> Result<Option<Detected>, String> {
    if problem.index_start != 1
        || problem.recurrence_start != 1
        || !problem.secondary_recurrences.is_empty()
    {
        return Ok(None);
    }
    let degree = match problem.kind {
        RecurrenceKind::FirstOrder => 1,
        RecurrenceKind::SecondOrder => 2,
        _ => return Ok(None),
    };
    let Some(l) = linear(&problem.recurrence.lhs) else {
        return Ok(None);
    };
    let Some(r) = linear(&problem.recurrence.rhs) else {
        return Ok(None);
    };
    let c: Vec<_> = (0..4).map(|j| l[j].sub(&r[j])).collect();
    let lead = &c[degree + 1];
    if lead.is_zero() || c[degree + 2..].iter().any(|v| !v.is_zero()) {
        return Ok(None);
    }
    let Some(first) = initial_value(problem, 1) else {
        return Ok(None);
    };
    let f = neg(&c[0]).div(lead)?;
    if degree == 1 {
        let p = neg(&c[1]).div(lead)?;
        let route = if p.mul(&first).add(&f) == first {
            Route::Constant {
                value: first.clone(),
            }
        } else {
            Route::First {
                p: p.clone(),
                q: f.clone(),
                initial: first.clone(),
            }
        };
        return Ok(Some(Detected {
            route,
            formula: first_formula(&p, &f, &first)?,
        }));
    }
    let p = neg(&c[2]).div(lead)?;
    let q = neg(&c[1]).div(lead)?;
    let Some(second) = initial_value(problem, 2) else {
        return Ok(None);
    };
    if first == second && p.add(&q).mul(&first).add(&f) == first {
        return Ok(Some(Detected {
            route: Route::Constant {
                value: first.clone(),
            },
            formula: rat(&first),
        }));
    }
    if q.is_zero() && f.is_zero() {
        let formula = if p.is_zero() {
            repeated_formula(&z(), &first, &second, &z(), &z())
        } else {
            let b = second.div(&p)?;
            Expr::add(vec![geom(&first.sub(&b), &z()), geom(&b, &p)])
        };
        let route = if second == p.mul(&first) {
            Route::Reduced {
                p: p.clone(),
                q: z(),
                initial: first.clone(),
            }
        } else {
            Route::Zero { p, first, second }
        };
        return Ok(Some(Detected { route, formula }));
    }
    let Some((r, s)) = root_pair(&p, &q) else {
        return Ok(None);
    };
    if r == s {
        if r.is_zero() {
            return Ok(None);
        }
        let shift = if r.is_one() {
            z()
        } else {
            f.div(&one().sub(&r).pow(2))?
        };
        let a = first.sub(&shift);
        let b = second.sub(&shift).div(&r)?.sub(&a);
        let formula = repeated_formula(&r, &a, &b, &shift, &f);
        let route = if b.is_zero() && f.is_zero() {
            Route::Reduced {
                p: r,
                q: z(),
                initial: first,
            }
        } else if r.is_one() && f.is_zero() {
            Route::Reduced {
                p: one(),
                q: b,
                initial: first,
            }
        } else if b.is_zero() && !r.is_one() {
            Route::Reduced {
                q: one().sub(&r).mul(&shift),
                p: r,
                initial: first,
            }
        } else {
            Route::Repeated {
                r,
                a,
                b,
                shift,
                forcing: f,
            }
        };
        return Ok(Some(Detected { route, formula }));
    }
    if (r.is_one() || s.is_one()) && !f.is_zero() {
        return Ok(None);
    }
    let shift = if f.is_zero() {
        z()
    } else {
        f.div(&one().sub(&p).sub(&q))?
    };
    let t = first.sub(&shift);
    let u = second.sub(&shift);
    let a = u.sub(&s.mul(&t)).div(&r.sub(&s))?;
    let b = t.sub(&a);
    let formula = Expr::add(vec![rat(&shift), geom(&a, &r), geom(&b, &s)]);
    let route = if a.is_zero() {
        Route::Reduced {
            p: s.clone(),
            q: one().sub(&s).mul(&shift),
            initial: first,
        }
    } else if b.is_zero() {
        Route::Reduced {
            p: r.clone(),
            q: one().sub(&r).mul(&shift),
            initial: first,
        }
    } else if r.is_one() {
        let h = shift.add(&a);
        Route::Reduced {
            p: s.clone(),
            q: one().sub(&s).mul(&h),
            initial: first,
        }
    } else if s.is_one() {
        let h = shift.add(&b);
        Route::Reduced {
            p: r.clone(),
            q: one().sub(&r).mul(&h),
            initial: first,
        }
    } else {
        Route::Distinct {
            r,
            s,
            a,
            b,
            shift,
            forcing: f,
        }
    };
    Ok(Some(Detected { route, formula }))
}
fn meta(route: &Route) -> (u8, &'static str, Vec<&'static str>, u8) {
    match route {
        Route::Constant { .. } => (1, "定数解を初期条件から確認する", vec!["constant"], 0),
        Route::First { p, .. } if p.is_zero() => {
            (1, "倍率0と初項例外を分ける", vec!["constant"], 0)
        }
        Route::First { p, .. } if p.is_one() => (
            3,
            "符号を含めて等差数列を解く",
            vec!["difference_sum", "evaluate_sum"],
            0,
        ),
        Route::First { q, .. } if q.is_zero() => {
            (2, "符号・分数公比の等比数列を解く", vec!["geometric"], 0)
        }
        Route::First { .. } => (
            5,
            "有理数の不動点を引く",
            vec!["fixed_point", "geometric"],
            1,
        ),
        Route::Reduced { p, .. } if p.is_one() => (
            6,
            "初期条件から等差数列に帰着する",
            vec!["reduce_order", "difference_sum", "evaluate_sum"],
            1,
        ),
        Route::Reduced { q, .. } if !q.is_zero() => (
            6,
            "初期条件から一次の不動点解法に帰着する",
            vec!["reduce_order", "fixed_point", "geometric"],
            1,
        ),
        Route::Reduced { .. } => (
            6,
            "初期条件から低い次数の解法に帰着する",
            vec!["reduce_order", "geometric"],
            1,
        ),
        Route::Zero { .. } => (
            6,
            "零根により第2項以降を解く",
            vec!["reduce_order", "geometric"],
            1,
        ),
        Route::Repeated { r, forcing, .. } if r.is_one() && !forcing.is_zero() => (
            6,
            "一定の二階差を和で戻す",
            vec!["difference", "difference_sum", "evaluate_sum"],
            1,
        ),
        Route::Repeated { forcing, .. } if !forcing.is_zero() => (
            15,
            "特解を分離して重根の3項間を解く",
            vec!["polynomial_shift", "characteristic_repeated"],
            2,
        ),
        Route::Repeated { .. } => (
            10,
            "重根の3項間を等比化する",
            vec!["characteristic_repeated"],
            1,
        ),
        Route::Distinct { forcing, .. } if !forcing.is_zero() => (
            14,
            "定数特解と異なる有理数根を使う",
            vec!["shift", "characteristic_distinct"],
            2,
        ),
        Route::Distinct { .. } => (
            10,
            "異なる有理数根の3項間を解く",
            vec!["characteristic_distinct"],
            1,
        ),
    }
}
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    let Some(d) = detect(problem)? else {
        return Ok(None);
    };
    let (score, name, operations, discovery) = meta(&d.route);
    Ok(Some(crate::generator::make_classification(
        problem,
        score,
        name,
        name,
        &operations,
        discovery,
        0,
        0,
    )?))
}
pub fn classify_degenerate(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    if let Some(d) = detect(problem)? {
        if matches!(
            &d.route,
            Route::Constant { .. } | Route::Reduced { .. } | Route::Zero { .. }
        ) || matches!(&d.route,Route::First{p,..}if p.is_zero())
        {
            return classify(problem);
        }
    }
    Ok(None)
}
fn step(text: impl Into<String>, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn first_steps(p: &Rational, q: &Rational, c: &Rational) -> Result<Vec<DerivationStep>, String> {
    if p.is_zero() {
        return Ok(vec![step(format!("右辺は前項に依存しないので、第2項以降はすべて {}。初項 {} は別に扱う。0⁰=1として式でも表せる。",q,c),None,"倍率0では初項を区別する")]);
    }
    if p.is_one() {
        return Ok(vec![step(
            format!("各回に {} を足す。初項から第n項まではn−1回進む。", q),
            None,
            "等差数列の一般項",
        )]);
    }
    if q.is_zero() {
        return Ok(vec![step(
            format!("公比は {}。負や分数でも、n−1回掛ける公式は同じである。", p),
            None,
            "等比数列の一般項",
        )]);
    }
    let h = q.div(&one().sub(p))?;
    Ok(vec![
        step(
            format!("一定の解xを代入し、不動点の方程式 x={}x+{} を解く。", p, q),
            None,
            "未知数を残した方程式を解く",
        ),
        step(
            format!("不動点は {}。bₙ=aₙ−({}) とおく。", h, h),
            Some(Equation::new(
                Expr::term("b", 0),
                Expr::sub(Expr::term("a", 0), rat(&h)),
            )),
            "1−p≠0を確認する",
        ),
        step(
            format!("bₙ₊₁={} bₙ、b₁={} なので等比数列である。", p, c.sub(&h)),
            Some(Equation::new(
                Expr::term("b", 1),
                Expr::mul(vec![rat(p), Expr::term("b", 0)]),
            )),
            "不動点の方程式を引く",
        ),
    ])
}
pub fn explain(
    problem: &ProblemIR,
    classification: &Classification,
) -> Result<Option<DerivationIR>, String> {
    let Some(d) = detect(problem)? else {
        return Ok(None);
    };
    let (_, name, _, _) = meta(&d.route);
    if classification.method_name != name {
        return Ok(None);
    }
    let(hint,mut steps)=match &d.route{
  Route::Constant{value}=>("初期条件と同じ値が続くか確認する。",vec![step(format!("すべての項を {} とすると初期条件と元の漸化式を満たす。次項は一意に定まるため、これが求める数列である。",value),None,"定数解の代入と一意性")]),
  Route::First{p,q,initial}=>("倍率0・倍率1・定数解への退化を先に調べる。",first_steps(p,q,initial)?),
  Route::Reduced{p,q,initial}=>{
   let mut steps=vec![step(format!("aₙ₊₁={} aₙ+{} となるかを調べる。この関係のずれをeₙとすると、元の3項間からeₙ₊₁はeₙの定数倍となる。初期条件よりe₁=0なので、すべてのずれが零になる。",p,q),Some(Equation::new(Expr::term("a",1),Expr::add(vec![Expr::mul(vec![rat(p),Expr::term("a",0)]),rat(q)]))),"ずれの初項と漸化式から帰納する")];steps.extend(first_steps(p,q,initial)?);("初期条件により特性成分の一方が消えないか調べる。",steps)
  },
  Route::Zero{p,first,second}=>{
   let text=if p.is_zero(){format!("元の式はaₙ₊₂=0。初項 {} と第2項 {} は独立で、第3項以降が零になる。",first,second)}else{format!("元の式はaₙ₊₂={}aₙ₊₁。第2項 {} からは公比 {} の等比数列で、初項 {} は独立である。",p,second,p,first)};
   ("aₙの係数が零なら第2項以降だけの関係になる。",vec![step(text,None,"零根の成分は最初の項だけに影響する"),step("0⁰=1を用いた一般項で初めの項を再現できる。初項・第2項は必ず個別に照合する。",None,"初期条件と例外項を区別する")])
  },
  Route::Repeated{r,a,b,forcing,..} if r.is_one() && !forcing.is_zero()=>{
   let definition=Equation::new(Expr::term("b",0),Expr::sub(Expr::term("a",1),Expr::term("a",0)));
   let recurrence=Equation::new(Expr::term("b",1),Expr::add(vec![Expr::term("b",0),rat(forcing)]));
   ("第2階差が一定なら、階差は等差数列になる。",vec![
    step("隣り合う項の差bₙ=aₙ₊₁−aₙを作る。",Some(definition),"元の3項間を差の式として整理する"),
    step(format!("階差は公差 {} の等差数列で、初階差は {}。",forcing,b),Some(recurrence),"bₙ₊₁−bₙが一定である"),
    step(format!("bₙ={}+{}(n−1)を第1項から足し合わせる。初項 {} を加えると二次式になる。",b,forcing,a),None,"等差数列の和で元の数列へ戻す"),
   ])
  },
  Route::Repeated{r,a,b,shift,forcing}=>{
   let mut steps=vec![];
   if !forcing.is_zero(){let text=if r.is_one(){format!("k=n−1として特解 {}k(k−1)/2 を引くと定数の第2階差が消える。これは階差が等差数列になることからも求められる。",forcing)}else{format!("一定の特解 {} を引く。これは(1−{})²h={}を解いて得られる。",shift,r,forcing)};steps.push(step(text,None,"特性根1で特解の形が変わる"));}
   steps.extend([
    step(format!("同次部分の特性方程式は(λ−{})²=0。重根では同じ等比数列を2つ足しても一般解にならない。",r),None,"特性多項式の判別式が零"),
    step(format!("特解を除いた数列をbₙとする。cₙ=bₙ/({})^(n−1) とおくとcₙ₊₂−2cₙ₊₁+cₙ=0となり、cₙは等差数列になる。",r),None,"r≠0を確認して正規化する"),
    step(format!("初期条件よりc₁={}、公差={}。同次解は({}+{}(n−1))({})^(n−1)。特解があれば加える。",a,b,a,b,r),None,"重根の一般解と2つの初期条件"),
   ]);("重根では等比の倍率を取り除いた数列を考える。",steps)
  },
  Route::Distinct{r,s,a,b,shift,forcing}=>{
   let mut steps=vec![];if !forcing.is_zero(){steps.push(step(format!("一定の特解 {} を引いて同次式にする。h=(p+q)h+Fを解くとこの値を得る。",shift),None,"1−p−q≠0を確認する"));}
   steps.extend([
    step(format!("特性方程式は(λ−{})(λ−{})=0。異なる2根は {} と {}。",r,s,r,s),None,"表示された係数から特性根を求める"),
    step(format!("同次解をA({})^(n−1)+B({})^(n−1)とおき、第1項と第2項を代入する。",r,s),None,"異なる2根なので2本の一次方程式は一意に解ける"),
    step(format!("初期条件を解くとA={}、B={}。定数特解があれば最後に加える。",a,b),None,"2つの初期条件で2つの係数を決める"),
   ]);("退化を調べてから特性方程式の根を求める。",steps)
  },
 };
    steps.push(step(
        "初期条件から求めた一般項を元の式へ代入して確認する。",
        Some(Equation::new(Expr::term("a", 0), d.formula.clone())),
        "問題の式から一般項を独立に再構成した",
    ));
    let auxiliaries = match &d.route {
        Route::First { p, q, initial } | Route::Reduced { p, q, initial }
            if !p.is_zero() && !p.is_one() && !q.is_zero() =>
        {
            let h = q.div(&one().sub(p))?;
            vec![crate::generator::AuxiliarySequence {
                name: "b".into(),
                definition: Equation::new(
                    Expr::term("b", 0),
                    Expr::sub(Expr::term("a", 0), rat(&h)),
                ),
                initial: Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(&initial.sub(&h)),
                ),
                recurrence: Equation::new(
                    Expr::term("b", 1),
                    Expr::mul(vec![rat(p), Expr::term("b", 0)]),
                ),
            }]
        }
        Route::Repeated { r, b, forcing, .. } if r.is_one() && !forcing.is_zero() => {
            vec![crate::generator::AuxiliarySequence {
                name: "b".into(),
                definition: Equation::new(
                    Expr::term("b", 0),
                    Expr::sub(Expr::term("a", 1), Expr::term("a", 0)),
                ),
                initial: Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(b),
                ),
                recurrence: Equation::new(
                    Expr::term("b", 1),
                    Expr::add(vec![Expr::term("b", 0), rat(forcing)]),
                ),
            }]
        }
        _ => vec![],
    };
    Ok(Some(DerivationIR {
        method_name: name.into(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    }))
}
/// Independent pedagogical routes, with reconstructed final formulas.
pub fn alternative_derivations(problem: &ProblemIR) -> Result<Vec<DerivationIR>, String> {
    let Some(d) = detect(problem)? else {
        return Ok(vec![]);
    };
    let mut out = vec![];
    match &d.route {
        Route::First { p, q, initial } if !p.is_zero() && !p.is_one() && !q.is_zero() => {
            let j = NatExpr::Bound { name: "j".into() };
            let sum = Expr::Sum {
                variable: "j".into(),
                lower: NatExpr::constant(1),
                upper: NatExpr::offset(-1),
                body: Box::new(Expr::pow(
                    rat(p),
                    NatExpr::Sub {
                        left: Box::new(j),
                        right: Box::new(NatExpr::constant(1)),
                    },
                )),
            };
            let expanded = Expr::add(vec![geom(initial, p), Expr::mul(vec![rat(q), sum])]);
            out.push(DerivationIR {
                method_name: "更新式を反復し有限和をまとめる".into(),
                hint: "前の項を繰り返し代入して、各定数項が何回倍率を掛けられるか数える。".into(),
                auxiliaries: vec![],
                steps: vec![
                    step(
                        "更新式をn−1回展開する。n=1では和を空和0として扱う。",
                        Some(Equation::new(Expr::term("a", 0), expanded)),
                        "代入の反復を帰納法で確認する",
                    ),
                    step(
                        format!(
                            "公比 {} は1ではないので、有限等比級数の和を使って整理する。",
                            p
                        ),
                        None,
                        "負や分数の公比でも有限和の公式は成り立つ",
                    ),
                    step(
                        "初項と定数項の寄与をまとめる。",
                        Some(Equation::new(Expr::term("a", 0), d.formula.clone())),
                        "不動点の置換を使わない導出",
                    ),
                ],
                alternatives: vec![],
            });
            if !p.is_zero() && !q.is_zero() {
                let delta = p.sub(&one()).mul(initial).add(q);
                let aux = crate::generator::AuxiliarySequence {
                    name: "b".into(),
                    definition: Equation::new(
                        Expr::term("b", 0),
                        Expr::sub(Expr::term("a", 1), Expr::term("a", 0)),
                    ),
                    initial: Equation::new(
                        Expr::Term {
                            sequence: "b".into(),
                            index: NatExpr::constant(1),
                        },
                        rat(&delta),
                    ),
                    recurrence: Equation::new(
                        Expr::term("b", 1),
                        Expr::mul(vec![rat(p), Expr::term("b", 0)]),
                    ),
                };
                out.push(DerivationIR {
                    method_name: "階差を等比数列として解き和で戻す".into(),
                    hint: "隣接する2つの更新式を引くと定数項が消える。".into(),
                    auxiliaries: vec![aux.clone()],
                    steps: vec![
                        step(
                            "bₙ=aₙ₊₁−aₙ と定める。",
                            Some(aux.definition),
                            "元の式をnとn+1で引き算する",
                        ),
                        step(
                            format!("階差はbₙ₊₁={}bₙ、初階差は {}。", p, delta),
                            Some(aux.recurrence),
                            "定数項が相殺される",
                        ),
                        step(
                            "aₙ=a₁+Σbⱼ として等比級数の和を求める。",
                            Some(Equation::new(Expr::term("a", 0), d.formula.clone())),
                            "階差の和は途中の項が相殺される",
                        ),
                    ],
                    alternatives: vec![],
                });
            }
        }
        Route::Distinct { r, s, a, shift, .. } => {
            let base = Expr::sub(Expr::term("a", 0), rat(shift));
            let nextbase = Expr::sub(Expr::term("a", 1), rat(shift));
            let definition = Equation::new(
                Expr::term("b", 0),
                Expr::sub(nextbase, Expr::mul(vec![rat(s), base])),
            );
            let amplitude = r.sub(s).mul(a);
            let recurrence = Equation::new(
                Expr::term("b", 1),
                Expr::mul(vec![rat(r), Expr::term("b", 0)]),
            );
            let auxiliary = crate::generator::AuxiliarySequence {
                name: "b".into(),
                definition: definition.clone(),
                initial: Equation::new(
                    Expr::Term {
                        sequence: "b".into(),
                        index: NatExpr::constant(1),
                    },
                    rat(&amplitude),
                ),
                recurrence: recurrence.clone(),
            };
            out.push(DerivationIR {
                method_name: "特性根で一次式を作り次数を下げる".into(),
                hint: "aₙ₊₁−s aₙのような差を作り、等比数列を見つける。".into(),
                auxiliaries: vec![auxiliary],
                steps: vec![
                    step(
                        format!(
                            "定数特解 {} を除いた数列について、根 {} を使って差を作る。",
                            shift, s
                        ),
                        Some(definition),
                        "特性多項式を一次式の積として使う",
                    ),
                    step(
                        format!(
                            "元の3項間からbₙ₊₁={}bₙ。初期条件からb₁={}なので一般項が求まる。",
                            r, amplitude
                        ),
                        Some(recurrence),
                        "二次の漸化式を一次に下げる",
                    ),
                    step(
                        format!(
                            "元の数列へ戻す一次漸化式を解く。異なる根なので {}−{} は零ではない。",
                            r, s
                        ),
                        Some(Equation::new(Expr::term("a", 0), d.formula.clone())),
                        "等比項を含む一次漸化式の有限和を整理する",
                    ),
                ],
                alternatives: vec![],
            });
        }
        Route::Repeated {
            r,
            a,
            b,
            shift,
            forcing,
        } => {
            let k = Expr::offset(-1);
            let particular = if r.is_one() {
                Expr::mul(vec![
                    rat(&forcing.div(&Rational::integer(2))?),
                    k.clone(),
                    Expr::sub(k.clone(), Expr::integer(1)),
                ])
            } else {
                rat(shift)
            };
            out.push(DerivationIR{method_name:"重根の一般解を代入し係数を決める".into(),hint:"重根rに対して(A+B(n−1))r^(n−1)を代入して確かめる。".into(),auxiliaries:vec![],steps:vec![
    step("特解を除いた同次式に(A+B(n−1))r^(n−1)を代入する。全てのA、Bで成立することを展開で確認する。",None,"重根の条件p=2r、q=−r²を使う"),
    step(format!("第1項、第2項からA={}、B={}。r≠0なのでBも一意に求まる。",a,b),None,"2つの初期条件を解く"),
    step("特解を加えた一般項を元の式に代入し、一意性から解とする。",Some(Equation::new(Expr::term("a",0),Expr::add(vec![Expr::mul(vec![Expr::add(vec![rat(a),Expr::mul(vec![rat(b),k])]),Expr::pow(rat(r),NatExpr::offset(-1))]),particular]))),"正規化を使わず代入と一意性で示す"),
   ],alternatives:vec![]});
        }
        _ => {}
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    #[test]
    fn registered_variants_are_exact_and_forward_detected() {
        let mut rng = ChaCha8Rng::seed_from_u64(917);
        for family in catalog() {
            for v in variants(&family.id) {
                for _ in 0..8 {
                    let p = sample_variant(&family.id, v.id, &mut rng).unwrap();
                    let recipe = build_recipe(&family.id, 91, p).unwrap();
                    let problem = compile(&recipe).unwrap();
                    let d = detect(&problem).unwrap().unwrap();
                    assert_eq!(variant_id(&recipe).as_deref(), Some(v.id));
                    let term = |_: &str, n| {
                        problem
                            .general_term
                            .rhs
                            .evaluate(n, &|_, _| Err("open".into()))
                    };
                    for n in 1..=20 {
                        assert_eq!(
                            problem.recurrence.lhs.evaluate(n, &term).unwrap(),
                            problem.recurrence.rhs.evaluate(n, &term).unwrap(),
                            "{} {} n={}",
                            family.id,
                            v.id,
                            n
                        );
                        assert_eq!(
                            d.formula.evaluate(n, &|_, _| Err("open".into())).unwrap(),
                            term("a", n).unwrap(),
                            "{} {} reconstruction n={}",
                            family.id,
                            v.id,
                            n
                        );
                    }
                    let mut hidden = problem.clone();
                    hidden.general_term.rhs = Expr::integer(999);
                    assert_eq!(detect(&hidden).unwrap().unwrap().formula, d.formula);
                    for route in alternative_derivations(&hidden).unwrap() {
                        let eq = route.steps.last().unwrap().equation.as_ref().unwrap();
                        for n in 1..=20 {
                            assert_eq!(
                                eq.rhs.evaluate(n, &|_, _| Err("open".into())).unwrap(),
                                term("a", n).unwrap()
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn variant_claims_and_parameter_keys_are_checked() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let mut p = sample_variant("cases_second_repeated", "root_one", &mut rng).unwrap();
        p.insert("r".into(), Rational::integer(2));
        assert!(build_recipe("cases_second_repeated", 1, p).is_err());
        let mut p = sample_variant("cases_reset", "initial_exception", &mut rng).unwrap();
        p.insert("extra".into(), one());
        assert!(validate_parameters("cases_reset", &p).is_err());
        assert!(sample_variant("cases_reset", "nonexistent", &mut rng).is_err());
    }
    #[test]
    fn constant_and_initial_exception_are_different() {
        let mut rng = ChaCha8Rng::seed_from_u64(7);
        let p = sample_variant("cases_reset", "initial_exception", &mut rng).unwrap();
        let problem = compile(&build_recipe("cases_reset", 7, p).unwrap()).unwrap();
        assert!(
            matches!(detect(&problem).unwrap().unwrap().route,Route::First{p,..}if p.is_zero())
        );
        let p = sample_variant("cases_affine_fixed", "fixed_initial", &mut rng).unwrap();
        let problem = compile(&build_recipe("cases_affine_fixed", 7, p).unwrap()).unwrap();
        assert!(matches!(
            detect(&problem).unwrap().unwrap().route,
            Route::Constant { .. }
        ));
    }
    #[test]
    fn roots_are_exact_and_unsupported_cases_declined() {
        assert_eq!(
            root_pair(&Rational::fraction(1, 6), &Rational::fraction(1, 6)),
            Some((Rational::fraction(-1, 3), Rational::fraction(1, 2)))
        );
        assert!(root_pair(&z(), &Rational::integer(-1)).is_none());
        assert!(root_pair(&z(), &Rational::integer(2)).is_none());
        let mut rng = ChaCha8Rng::seed_from_u64(8);
        let p = sample_variant("cases_second_repeated", "root_fraction", &mut rng).unwrap();
        let mut problem = compile(&build_recipe("cases_second_repeated", 8, p).unwrap()).unwrap();
        problem.recurrence.rhs = Expr::mul(vec![Expr::term("a", 0), Expr::term("a", 1)]);
        assert!(detect(&problem).unwrap().is_none());
    }
    #[test]
    fn repeated_root_one_with_zero_initial_difference_still_has_quadratic_forcing() {
        let mut rng = ChaCha8Rng::seed_from_u64(80);
        let params = sample_variant("cases_second_forced", "root_one", &mut rng).unwrap();
        let mut problem =
            compile(&build_recipe("cases_second_forced", 80, params).unwrap()).unwrap();
        problem.initials[1].rhs = problem.initials[0].rhs.clone();
        let detected = detect(&problem).unwrap().unwrap();
        assert!(
            matches!(detected.route, Route::Repeated { ref r, ref b, ref forcing, .. }
            if r.is_one() && b.is_zero() && !forcing.is_zero())
        );
        let term = |_: &str, n| detected.formula.evaluate(n, &|_, _| Err("open".into()));
        for n in 1..=20 {
            assert_eq!(
                problem.recurrence.lhs.evaluate(n, &term).unwrap(),
                problem.recurrence.rhs.evaluate(n, &term).unwrap()
            );
        }
    }
}
