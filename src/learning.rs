//! Question-based, structured solution routes. A route check is finite and is
//! deliberately distinct from the native proof of the original recurrence.
use crate::generator::{
    AuxiliarySequence, Classification, DerivationIR, DerivationStep, GeneratedProblem, ProblemIR,
    RecurrenceKind, SolutionStep,
};
use crate::math::{Equation, Expr, NatExpr, Rational};
use crate::route_check::{self, RouteValidation};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ASSESSMENT_VERSION: &str = "education-2.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MethodInfo {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionRoute {
    pub id: String,
    pub method_id: String,
    pub method_name: String,
    pub hint: String,
    pub level: u8,
    pub score: u8,
    pub classification: Classification,
    pub derivation: DerivationIR,
    pub solution: Vec<SolutionStep>,
    pub validation: RouteValidation,
    /// Complete answer and essential auxiliary equations are finitely checked.
    pub complete: bool,
}

pub fn methods() -> Vec<MethodInfo> {
    [
        ("constant", "一定値・初項の例外を直接調べる"),
        ("arithmetic", "等差数列として解く"),
        ("geometric", "等比数列として解く"),
        ("fixed_point", "不動点を引く"),
        ("normalize", "倍率で正規化する"),
        ("difference", "階差を取る"),
        ("particular", "特解を引く"),
        ("characteristic", "特性方程式を使う"),
        ("reduce_order", "次数を下げる"),
        ("reciprocal", "逆数を取る"),
        ("cross_ratio", "二つの不動点との比を取る"),
        ("partial_sum", "部分和を先に求める"),
        ("eliminate_sum", "隣接差で総和を消す"),
        ("logarithm", "対数を取る"),
        ("iterate", "更新式を順に展開する"),
        ("polynomial", "多項式を当てはめる"),
        ("ansatz", "一般解の形を仮定して代入する"),
        ("pair", "連立を一次結合で分離する"),
        ("ratio_product", "階比を掛け合わせる"),
    ]
    .into_iter()
    .map(|(id, label)| MethodInfo {
        id: id.into(),
        label: label.into(),
    })
    .collect()
}

fn method_id(c: &Classification) -> &'static str {
    let has = |s: &str| c.operations.iter().any(|op| op == s);
    if has("reduce_order") {
        "reduce_order"
    } else if has("logarithm") {
        "logarithm"
    } else if has("split_pair") {
        "pair"
    } else if has("reciprocal") {
        "reciprocal"
    } else if has("index_scale") {
        "normalize"
    } else if has("eliminate_sum") {
        "eliminate_sum"
    } else if has("characteristic_distinct") || has("characteristic_repeated") {
        "characteristic"
    } else if has("polynomial_shift") || has("particular_solution") {
        "particular"
    } else if c.method_name.contains("階差") {
        "difference"
    } else if has("fixed_point") {
        "fixed_point"
    } else if c.method_name.contains("等差") {
        "arithmetic"
    } else if has("geometric") {
        "geometric"
    } else if has("ratio_product") {
        "ratio_product"
    } else {
        "constant"
    }
}

pub fn level(score: u8) -> u8 {
    match score {
        0..=3 => 1,
        4..=7 => 2,
        8..=12 => 3,
        13..=18 => 4,
        _ => 5,
    }
}

fn assessed(problem: &ProblemIR, original: &Classification, modern: bool) -> Classification {
    let mut c = original.clone();
    if modern {
        // These rules depend on recovered methods, never the Recipe or answer.
        if c.method_name == "階差を等差数列として解く" {
            c.score = 6;
        }
        if problem.kind == RecurrenceKind::SumRelation {
            c.score = if c.operations.iter().any(|s| s == "polynomial_shift") {
                11
            } else {
                8
            };
            c.reason = "総和の消去に加え、帰着した一次漸化式の解法を評価する".into();
        }
        if c.operations == ["reciprocal", "difference_sum", "evaluate_sum"]
            && c.metrics.coefficient_degree == 0
        {
            c.score = 8;
        }
        c.level = level(c.score);
        c.rules_version = ASSESSMENT_VERSION.into();
    }
    c
}

fn ds(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: vec![],
    }
}
fn rat(x: &Rational) -> Expr {
    Expr::rational(x.clone())
}
fn at(name: &str, index: u32) -> Expr {
    Expr::Term {
        sequence: name.into(),
        index: NatExpr::constant(index),
    }
}
fn geometric(amplitude: &Rational, ratio: &Rational) -> Expr {
    Expr::mul(vec![
        rat(amplitude),
        Expr::pow(rat(ratio), NatExpr::offset(-1)),
    ])
}
fn summation(body: Expr) -> Expr {
    Expr::Sum {
        variable: "k".into(),
        lower: NatExpr::constant(1),
        upper: NatExpr::offset(-1),
        body: Box::new(body),
    }
}
fn derive(
    name: &str,
    hint: &str,
    auxiliaries: Vec<AuxiliarySequence>,
    steps: Vec<DerivationStep>,
) -> DerivationIR {
    DerivationIR {
        method_name: name.into(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    }
}
type Alternative = (String, u8, DerivationIR);

fn scalar_alternatives(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    use crate::generator::{initial_value, linear_split, literal};
    if problem.kind != RecurrenceKind::FirstOrder || problem.recurrence.lhs != Expr::term("a", 1) {
        return Ok(vec![]);
    }
    let (coefficient, forcing) = match linear_split(&problem.recurrence.rhs, &Expr::term("a", 0)) {
        Ok(v) => v,
        Err(_) => return Ok(vec![]),
    };
    let Some(p) = literal(&coefficient) else {
        return Ok(vec![]);
    };
    let Some(q) = literal(&forcing) else {
        return Ok(vec![]);
    };
    let first = initial_value(problem, 0)?;
    let mut out = vec![];
    if !p.is_zero() && !p.is_one() {
        let definition = Equation::new(
            Expr::term("u", 0),
            Expr::div(Expr::term("a", 0), Expr::pow(rat(&p), NatExpr::offset(-1))),
        );
        let recurrence = Equation::new(
            Expr::term("u", 1),
            Expr::add(vec![
                Expr::term("u", 0),
                Expr::div(rat(&q), Expr::pow(rat(&p), NatExpr::n())),
            ]),
        );
        let aux = AuxiliarySequence {
            name: "u".into(),
            definition: definition.clone(),
            initial: Equation::new(at("u", 1), rat(&first)),
            recurrence: recurrence.clone(),
        };
        let k = NatExpr::Bound { name: "k".into() };
        let normalized = Expr::add(vec![
            rat(&first),
            summation(Expr::div(rat(&q), Expr::pow(rat(&p), k))),
        ]);
        let answer = if p.is_one() {
            Expr::add(vec![
                rat(&first),
                Expr::mul(vec![rat(&q), Expr::offset(-1)]),
            ])
        } else {
            Expr::add(vec![
                geometric(&first, &p),
                Expr::mul(vec![
                    rat(&q.div(&p.sub(&Rational::integer(1)))?),
                    Expr::sub(Expr::pow(rat(&p), NatExpr::offset(-1)), Expr::integer(1)),
                ]),
            ])
        };
        out.push((
            "normalize".into(),
            if q.is_zero() { 3 } else { 7 },
            derive(
                "公比の冪で割って変化を足す",
                "倍率の冪で割ると、付加項だけを足せる形になる。",
                vec![aux],
                vec![
                    ds(
                        "倍率は非零なので、そのn−1乗で割った数列を定義する。",
                        Some(definition),
                        "非零の冪による置換",
                    ),
                    ds(
                        "元の更新式を代入すると、隣り合う補助数列の差が求まる。",
                        Some(recurrence),
                        "両辺を倍率のn乗で割る",
                    ),
                    ds(
                        "変化を初項から足し合わせる。",
                        Some(Equation::new(Expr::term("u", 0), normalized)),
                        "有限和の相殺",
                    ),
                    ds(
                        "有限等比級数をまとめ、元の倍率を掛け戻す。",
                        Some(Equation::new(Expr::term("a", 0), answer)),
                        "n=1の空和と初期条件も含む",
                    ),
                ],
            ),
        ));
    }
    if !p.is_one() && !p.is_zero() && !q.is_zero() {
        let d1 = p.sub(&Rational::integer(1)).mul(&first).add(&q);
        if !d1.is_zero() {
            let definition = Equation::new(
                Expr::term("v", 0),
                Expr::sub(Expr::term("a", 1), Expr::term("a", 0)),
            );
            let recurrence = Equation::new(
                Expr::term("v", 1),
                Expr::mul(vec![rat(&p), Expr::term("v", 0)]),
            );
            let aux = AuxiliarySequence {
                name: "v".into(),
                definition: definition.clone(),
                initial: Equation::new(at("v", 1), rat(&d1)),
                recurrence: recurrence.clone(),
            };
            let answer = Expr::add(vec![
                rat(&first),
                Expr::mul(vec![
                    rat(&d1.div(&p.sub(&Rational::integer(1)))?),
                    Expr::sub(Expr::pow(rat(&p), NatExpr::offset(-1)), Expr::integer(1)),
                ]),
            ]);
            out.push((
                "difference".into(),
                6,
                derive(
                    "階差を等比数列にする",
                    "同じ定数項を持つ隣接する2式の差を取る。",
                    vec![aux],
                    vec![
                        ds(
                            "隣り合う項の差を新しい数列とする。",
                            Some(definition),
                            "階差の定義",
                        ),
                        ds(
                            "隣接する更新式の差を取ると定数項が消える。",
                            Some(recurrence),
                            "同じ定数項の相殺",
                        ),
                        ds(
                            "階差数列の初項と公比から一般項を求める。",
                            Some(Equation::new(Expr::term("v", 0), geometric(&d1, &p))),
                            "等比数列の一般項",
                        ),
                        ds(
                            "階差の有限等比級数を初項に加える。",
                            Some(Equation::new(Expr::term("a", 0), answer)),
                            "元の数列への復元",
                        ),
                    ],
                ),
            ));
        }
    }
    Ok(out)
}

fn polynomial_second(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    if problem.kind != RecurrenceKind::SecondOrder || problem.recurrence.lhs != Expr::term("a", 2) {
        return Ok(vec![]);
    }
    let (p, rest) = crate::generator::linear_split(&problem.recurrence.rhs, &Expr::term("a", 1))?;
    let (q, f) = crate::generator::linear_split(&rest, &Expr::term("a", 0))?;
    if crate::generator::literal(&p) != Some(Rational::integer(2))
        || crate::generator::literal(&q) != Some(Rational::integer(-1))
    {
        return Ok(vec![]);
    }
    let Some(f) = crate::generator::literal(&f) else {
        return Ok(vec![]);
    };
    let first = crate::generator::initial_value(problem, 0)?;
    let second = crate::generator::initial_value(problem, 1)?;
    let a = f.div(&Rational::integer(2))?;
    let b = second.sub(&first).sub(&a);
    let formula = Expr::add(vec![
        rat(&first),
        Expr::mul(vec![rat(&b), Expr::offset(-1)]),
        Expr::mul(vec![
            rat(&a),
            Expr::pow(Expr::offset(-1), NatExpr::constant(2)),
        ]),
    ]);
    Ok(vec![(
        "polynomial".into(),
        7,
        derive(
            "二次式を当てはめる",
            "二階差が一定なので、一般項を二次式として係数を合わせる。",
            vec![],
            vec![
                ds(
                    "(n−1)の二次式を置く。二階差では二次の係数の2倍だけが残る。",
                    None,
                    "二階差による次数の低下",
                ),
                ds(
                    &format!(
                        "二次の係数は{}、一次の係数は{}、定数は{}となる。",
                        a, b, first
                    ),
                    None,
                    "二階差と2つの初期条件の係数比較",
                ),
                ds(
                    "求めた係数を代入する。初期条件と元の式に戻して確かめられる。",
                    Some(Equation::new(Expr::term("a", 0), formula)),
                    "一般項の復元",
                ),
            ],
        ),
    )])
}

fn polynomial_normalization(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    use crate::generator::{initial_value, linear_split, literal};
    if problem.kind != RecurrenceKind::FirstOrder || problem.recurrence.lhs != Expr::term("a", 1) {
        return Ok(vec![]);
    }
    let Ok((p, f)) = linear_split(&problem.recurrence.rhs, &Expr::term("a", 0)) else {
        return Ok(vec![]);
    };
    let Some(p) = literal(&p) else {
        return Ok(vec![]);
    };
    if p.is_zero() || p.is_one() {
        return Ok(vec![]);
    }
    let Ok((u, v)) = linear_split(&f, &Expr::index()) else {
        return Ok(vec![]);
    };
    let (Some(u), Some(v)) = (literal(&u), literal(&v)) else {
        return Ok(vec![]);
    };
    if u.is_zero() {
        return Ok(vec![]);
    }
    let first = initial_value(problem, 0)?;
    let denominator = Rational::integer(1).sub(&p);
    let alpha = u.div(&denominator)?;
    let beta = v.sub(&alpha).div(&denominator)?;
    let answer = Expr::add(vec![
        geometric(&first.sub(&alpha).sub(&beta), &p),
        Expr::mul(vec![rat(&alpha), Expr::index()]),
        rat(&beta),
    ]);
    let definition = Equation::new(
        Expr::term("u", 0),
        Expr::div(Expr::term("a", 0), Expr::pow(rat(&p), NatExpr::offset(-1))),
    );
    let recurrence = Equation::new(
        Expr::term("u", 1),
        Expr::add(vec![
            Expr::term("u", 0),
            Expr::div(f, Expr::pow(rat(&p), NatExpr::n())),
        ]),
    );
    let k = NatExpr::Bound { name: "k".into() };
    let body = Expr::div(
        Expr::add(vec![
            Expr::mul(vec![rat(&u), Expr::NatCast { value: k.clone() }]),
            rat(&v),
        ]),
        Expr::pow(rat(&p), k),
    );
    let aux = AuxiliarySequence {
        name: "u".into(),
        definition: definition.clone(),
        initial: Equation::new(at("u", 1), rat(&first)),
        recurrence: recurrence.clone(),
    };
    Ok(vec![(
        "normalize".into(),
        10,
        derive(
            "公比の冪で割り、一次式の重み付き総和を求める",
            "各項を倍率の冪で割り、等比級数と添字を掛けた等比級数の和を使う。",
            vec![aux],
            vec![
                ds(
                    "非零の倍率の冪で正規化する。",
                    Some(definition),
                    "非零の冪による置換",
                ),
                ds(
                    "元の式を倍率のn乗で割ると、補助数列の階差が求まる。",
                    Some(recurrence),
                    "更新式の代入",
                ),
                ds(
                    "初項から変化を足す。n=1では和は0になる。",
                    Some(Equation::new(
                        Expr::term("u", 0),
                        Expr::add(vec![rat(&first), summation(body)]),
                    )),
                    "有限和の相殺",
                ),
                ds(
                    "等比級数とその添字付きの和を整理して、元の倍率を掛け戻す。",
                    Some(Equation::new(Expr::term("a", 0), answer)),
                    "有限等比級数を倍率倍して引く",
                ),
            ],
        ),
    )])
}

fn fractional_alternatives(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    use crate::extensions::first_order::{detect, Route};
    let Some(Route::Mobius {
        shift,
        ratio,
        forcing,
    }) = detect(problem)
    else {
        return Ok(vec![]);
    };
    if forcing.is_zero() || ratio.is_zero() || ratio.is_one() {
        return Ok(vec![]);
    }
    let other = shift.add(&Rational::integer(1).sub(&ratio).div(&forcing)?);
    let first = crate::generator::initial_value(problem, 0)?;
    if first == other || first == shift {
        return Ok(vec![]);
    }
    let z1 = first.sub(&shift).div(&first.sub(&other))?;
    let multiplier = Rational::integer(1).div(&ratio)?;
    let definition = Equation::new(
        Expr::term("z", 0),
        Expr::div(
            Expr::sub(Expr::term("a", 0), rat(&shift)),
            Expr::sub(Expr::term("a", 0), rat(&other)),
        ),
    );
    let recurrence = Equation::new(
        Expr::term("z", 1),
        Expr::mul(vec![rat(&multiplier), Expr::term("z", 0)]),
    );
    let z = geometric(&z1, &multiplier);
    let answer = Expr::div(
        Expr::sub(rat(&shift), Expr::mul(vec![rat(&other), z.clone()])),
        Expr::sub(Expr::integer(1), z.clone()),
    );
    let aux = AuxiliarySequence {
        name: "z".into(),
        definition: definition.clone(),
        initial: Equation::new(at("z", 1), rat(&z1)),
        recurrence: recurrence.clone(),
    };
    Ok(vec![(
        "cross_ratio".into(),
        14,
        derive(
            "二つの不動点との比を取る",
            "二つの不動点からの差を比にすると、等比数列になる。",
            vec![aux],
            vec![
                ds(
                    &format!("不動点方程式から{}と{}を得る。", shift, other),
                    None,
                    "元の一次分数に一定値を代入する",
                ),
                ds(
                    "二つの不動点からの差の比を定義する。分母が零でない範囲で用いる。",
                    Some(definition),
                    "不動点からの差の比",
                ),
                ds(
                    "元の式を代入し、不動点方程式で整理する。",
                    Some(recurrence),
                    "一次分数の因数の相殺",
                ),
                ds(
                    "初項を入れて等比数列を求める。",
                    Some(Equation::new(Expr::term("z", 0), z)),
                    "等比数列の一般項",
                ),
                ds(
                    "比の定義を元の数列について解く。得た比は1にならないことも確認する。",
                    Some(Equation::new(Expr::term("a", 0), answer)),
                    "逆変換と分母条件",
                ),
            ],
        ),
    )])
}

fn partial_sum_alternative(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    if problem.kind != RecurrenceKind::PureSum {
        return Ok(vec![]);
    }
    let route = crate::extensions::sums::forward(problem)?;
    let sum = crate::extensions::sums::sum(0);
    let sum_next = crate::extensions::sums::sum(1);
    if problem.recurrence.lhs != sum_next {
        return Ok(vec![]);
    }
    let (r, f) = crate::extensions::sums::split(&problem.recurrence.rhs, &sum)?;
    let Some(r) = crate::generator::literal(&r) else {
        return Ok(vec![]);
    };
    let Some(closed) = route
        .derivation
        .steps
        .iter()
        .filter_map(|s| s.equation.as_ref())
        .find(|e| e.lhs == Expr::term("S", 0) && !crate::generator::has_sum(&e.rhs))
        .map(|e| e.rhs.clone())
    else {
        return Ok(vec![]);
    };
    let initial = problem.initials[0].rhs.clone();
    let definition = Equation::new(Expr::term("S", 0), sum);
    let recurrence = Equation::new(
        Expr::term("S", 1),
        Expr::add(vec![Expr::mul(vec![rat(&r), Expr::term("S", 0)]), f]),
    );
    let aux = AuxiliarySequence {
        name: "S".into(),
        definition: definition.clone(),
        initial: Equation::new(at("S", 1), initial),
        recurrence: recurrence.clone(),
    };
    Ok(vec![(
        "partial_sum".into(),
        8,
        derive(
            "部分和の漸化式を先に解く",
            "部分和を一つの数列として解き、その隣接差で各項を取り戻す。",
            vec![aux],
            vec![
                ds(
                    "部分和をS_nとする。S_0=0である。",
                    Some(definition),
                    "部分和の定義",
                ),
                ds(
                    "与えられた式を部分和の漸化式として読む。",
                    Some(recurrence),
                    "一次非同次漸化式への帰着",
                ),
                ds(
                    "定数または一次式の特解を引いて解き、初期条件を入れる。",
                    Some(Equation::new(Expr::term("S", 0), closed)),
                    "特解と等比数列",
                ),
                ds(
                    "隣り合う部分和の差を取る。n=1では空和S_0=0を使う。",
                    Some(Equation::new(
                        Expr::term("a", 0),
                        Expr::sub(
                            Expr::term("S", 0),
                            Expr::Term {
                                sequence: "S".into(),
                                index: NatExpr::offset(-1),
                            },
                        ),
                    )),
                    "部分和から各項を復元",
                ),
                ds(
                    "指数を整理すると次の一般項になる。",
                    Some(Equation::new(Expr::term("a", 0), route.formula)),
                    "有限和と初期境界",
                ),
            ],
        ),
    )])
}

fn power_alternatives(problem: &ProblemIR) -> Result<Vec<Alternative>, String> {
    use crate::extensions::powers::{ratio_exponents, replace_nat_index, second_exponents};
    use crate::generator::{initial_value, linear_split, literal};
    let target = if problem.kind == RecurrenceKind::FirstOrder {
        Expr::term("a", 0)
    } else if problem.kind == RecurrenceKind::MultiplicativeSecond {
        Expr::pow(Expr::term("a", 1), NatExpr::constant(2))
    } else {
        return Ok(vec![]);
    };
    let Ok((coefficient, rest)) = linear_split(&problem.recurrence.rhs, &target) else {
        return Ok(vec![]);
    };
    if rest != Expr::integer(0) {
        return Ok(vec![]);
    }
    let Expr::Pow { base, exponent } = coefficient else {
        return Ok(vec![]);
    };
    let Some(base) = literal(&base) else {
        return Ok(vec![]);
    };
    if !base.is_positive() || base.is_one() {
        return Ok(vec![]);
    }
    let first = initial_value(problem, 0)?;
    if problem.kind == RecurrenceKind::FirstOrder {
        for profile in 0..=2 {
            let (total, increment) = ratio_exponents(profile);
            if exponent != increment {
                continue;
            }
            let factor = Expr::pow(rat(&base), total);
            let definition = Equation::new(
                Expr::term("u", 0),
                Expr::div(Expr::term("a", 0), factor.clone()),
            );
            let recurrence = Equation::new(Expr::term("u", 1), Expr::term("u", 0));
            let aux = AuxiliarySequence {
                name: "u".into(),
                definition: definition.clone(),
                initial: Equation::new(at("u", 1), rat(&first)),
                recurrence: recurrence.clone(),
            };
            return Ok(vec![(
                "normalize".into(),
                if profile == 2 { 15 } else { 10 },
                derive(
                    "指数の累積和で割って不変量を作る",
                    "倍率の指数を累積した冪で割り、隣接する項が等しくなることを示す。",
                    vec![aux],
                    vec![
                        ds(
                            "倍率の指数を累積した冪で割る。底は正なので割り算できる。",
                            Some(definition),
                            "正の冪による正規化",
                        ),
                        ds(
                            "更新式に代入すると指数が相殺され、補助数列は一定になる。",
                            Some(recurrence),
                            "累積指数の隣接差は元の倍率の指数",
                        ),
                        ds(
                            "初項から補助数列の一定値を定める。",
                            Some(Equation::new(Expr::term("u", 0), rat(&first))),
                            "定数数列",
                        ),
                        ds(
                            "正規化の定義を戻す。",
                            Some(Equation::new(
                                Expr::term("a", 0),
                                Expr::mul(vec![rat(&first), factor]),
                            )),
                            "元の数列への復元",
                        ),
                    ],
                ),
            )]);
        }
        return Ok(vec![]);
    }
    fn log_integer(value: &Rational, base: &Rational) -> Option<u32> {
        let mut power = Rational::integer(1);
        for n in 0..=32 {
            if &power == value {
                return Some(n);
            }
            power = power.mul(base);
        }
        None
    }
    let second = initial_value(problem, 1)?;
    let (Some(e), Some(next)) = (log_integer(&first, &base), log_integer(&second, &base)) else {
        return Ok(vec![]);
    };
    let Some(d) = next.checked_sub(e) else {
        return Ok(vec![]);
    };
    for profile in 0..=2 {
        for c in 1..=9 {
            let (total, increment) = second_exponents(profile, e, d, c);
            if increment != exponent {
                continue;
            }
            let nexttotal = replace_nat_index(&total, &NatExpr::offset(1));
            let delta = NatExpr::Sub {
                left: Box::new(nexttotal),
                right: Box::new(total.clone()),
            };
            let formula = Expr::pow(rat(&base), delta);
            let definition = Equation::new(
                Expr::term("v", 0),
                Expr::div(Expr::term("a", 1), Expr::term("a", 0)),
            );
            let recurrence = Equation::new(
                Expr::term("v", 1),
                Expr::mul(vec![
                    Expr::pow(rat(&base), exponent.clone()),
                    Expr::term("v", 0),
                ]),
            );
            let aux = AuxiliarySequence {
                name: "v".into(),
                definition: definition.clone(),
                initial: Equation::new(at("v", 1), rat(&second.div(&first)?)),
                recurrence: recurrence.clone(),
            };
            return Ok(vec![(
                "ratio_product".into(),
                21,
                derive(
                    "隣接比を解いてから元の項を復元する",
                    "正の項どうしの比を作ると一次の階比型になる。比を順に掛けて元の項を求める。",
                    vec![aux],
                    vec![
                        ds(
                            "初期値と倍率は正なので、隣接比を定義できる。",
                            Some(definition),
                            "非零の項による除算",
                        ),
                        ds(
                            "元の積の関係を隣接する項の積で割る。",
                            Some(recurrence),
                            "隣接比の一次漸化式",
                        ),
                        ds(
                            "比の初期値から倍率を掛け合わせ、指数の有限和を求める。",
                            Some(Equation::new(Expr::term("v", 0), formula)),
                            "指数法則と有限和",
                        ),
                        ds(
                            "a_n=a_1 v_1…v_{n−1}で戻し、指数の和をまとめる。",
                            Some(Equation::new(
                                Expr::term("a", 0),
                                Expr::pow(rat(&base), total),
                            )),
                            "隣接比の積と空積",
                        ),
                    ],
                ),
            )]);
        }
    }
    Ok(vec![])
}

fn route(
    problem: &ProblemIR,
    mut c: Classification,
    d: DerivationIR,
    id: &str,
) -> Result<SolutionRoute, String> {
    let validation = route_check::validate(problem, &d)?;
    let mut essential = d.clone();
    essential.steps.retain(|s| {
        s.equation.as_ref().is_some_and(|e| {
            e.lhs == problem.general_term.lhs
                || problem
                    .secondary_general_terms
                    .iter()
                    .any(|g| g.lhs == e.lhs)
        })
    });
    let complete = !essential.steps.is_empty()
        && route_check::validate(problem, &essential)?.unresolved_equations == 0;
    // Difficulty is a registered curriculum rubric; these parts are a separate
    // operation/notation diagnostic, recomputed for each route.
    c.parts.b = c
        .operations
        .iter()
        .map(|op| match op.as_str() {
            "constant" => 1,
            "geometric" | "fixed_point" | "difference" | "difference_sum" | "ratio_product" => 2,
            "index_scale" | "reciprocal" | "eliminate_sum" | "logarithm" => 3,
            "polynomial_shift" | "particular_solution" => 4,
            "characteristic_distinct" => 5,
            "characteristic_repeated" => 6,
            "evaluate_sum" | "evaluate_product" => 1,
            _ => 0,
        })
        .sum();
    c.parts.a = c.numeric_cost.min(6);
    c.total_score = c.parts.b + c.parts.r + c.parts.a + c.parts.t + c.parts.p + c.parts.u;
    let solution = d
        .steps
        .iter()
        .map(|s| SolutionStep {
            text: s.text.clone(),
            latex: s.equation.as_ref().map(Equation::latex),
            reason: s.reason.clone(),
        })
        .collect();
    Ok(SolutionRoute {
        id: format!("{id}:{}", d.method_name),
        method_id: id.into(),
        method_name: d.method_name.clone(),
        hint: d.hint.clone(),
        level: c.level,
        score: c.score,
        classification: c,
        derivation: d,
        solution,
        validation,
        complete,
    })
}

pub fn build_routes(
    problem: &ProblemIR,
    classification: &Classification,
    derivation: &DerivationIR,
    modern: bool,
) -> Result<Vec<SolutionRoute>, String> {
    let base = assessed(problem, classification, modern);
    let primary = route(problem, base.clone(), derivation.clone(), method_id(&base))?;
    let mut result = vec![primary];
    let mut alternatives = scalar_alternatives(problem)?;
    alternatives.extend(polynomial_second(problem)?);
    alternatives.extend(polynomial_normalization(problem)?);
    alternatives.extend(fractional_alternatives(problem)?);
    alternatives.extend(partial_sum_alternative(problem)?);
    alternatives.extend(power_alternatives(problem)?);
    // New extension modules also expose independent forward derivations.
    for d in crate::extensions::elementary::alternative_derivations(problem)? {
        let id = if d.method_name.contains("反復") {
            "iterate"
        } else if d.method_name.contains("階差") {
            "difference"
        } else if d.method_name.contains("一般解") {
            "ansatz"
        } else {
            "reduce_order"
        };
        alternatives.push((id.into(), base.score.saturating_add(1).min(25), d));
    }
    for d in crate::extensions::exponential::alternative_derivations(problem)? {
        let id = if d.method_name.contains("3項間") {
            "characteristic"
        } else if d.method_name.contains("対数") {
            "logarithm"
        } else if d.method_name.contains("特解") {
            "particular"
        } else {
            "iterate"
        };
        alternatives.push((id.into(), base.score.saturating_add(1).min(25), d));
    }
    let mut names = BTreeSet::from([result[0].method_id.clone()]);
    for (id, score, d) in alternatives {
        if !names.insert(id.clone()) {
            continue;
        }
        let mut c = base.clone();
        c.score = score;
        c.level = level(score);
        c.method_name = d.method_name.clone();
        c.operations = match id.as_str() {
            "normalize" => vec!["index_scale", "difference_sum", "evaluate_sum"],
            "difference" => vec!["difference", "geometric", "evaluate_sum"],
            "iterate" => vec!["evaluate_sum"],
            "polynomial" | "ansatz" => vec!["polynomial_shift"],
            "characteristic" => vec!["characteristic_distinct"],
            "reduce_order" => vec!["geometric", "evaluate_sum"],
            "cross_ratio" => vec!["reciprocal", "fixed_point", "geometric"],
            "partial_sum" => vec!["fixed_point", "geometric", "recover_sum"],
            "ratio_product" => vec!["ratio_product", "evaluate_product", "evaluate_sum"],
            _ => vec![],
        }
        .into_iter()
        .map(str::to_string)
        .collect();
        c.parts.r = 1;
        c.reason = "表示された式と初期条件から別の完結する解法を構成した".into();
        // Invalid alternatives never enter the answer or influence difficulty.
        if let Ok(r) = route(problem, c, d, &id) {
            result.push(r)
        }
    }
    if modern {
        result.sort_by_key(|r| (!r.complete, r.score));
    }
    Ok(result)
}

pub fn attach(problem: &mut GeneratedProblem, modern: bool) -> Result<(), String> {
    let routes = build_routes(
        &problem.problem,
        &problem.classification,
        &problem.derivation,
        modern,
    )?;
    if modern {
        let first = &routes[0];
        problem.classification = first.classification.clone();
        problem.derivation = first.derivation.clone();
        problem.solution = first.solution.clone();
        problem.hint = first.hint.clone();
    }
    problem.routes = routes;
    Ok(())
}
