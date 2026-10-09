//! Registered dynamic generators and forward, statement-based solution detection.
use crate::math::{Equation, Expr, NatExpr, Rational};
use num_traits::{Signed, ToPrimitive};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const RULES_VERSION: &str = "rust-registered-1.0/reference-0.7.0";
pub const EXTENSION_RULES_VERSION: &str = "rust-registered-1.1/reference-0.7.0";
pub const RNG_VERSION: &str = "rand_chacha-0.3/ChaCha8Rng-v1";
pub const SCHEMA_VERSION: &str = "1.0";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FamilyInfo {
    pub id: String,
    pub label: String,
    pub levels: Vec<u8>,
}
pub fn family_catalog() -> Vec<FamilyInfo> {
    let mut result: Vec<FamilyInfo> = [
        ("arithmetic", "等差数列", 1),
        ("geometric", "等比数列", 1),
        ("scaled_constant", "単項式の階比", 2),
        ("factorial_ratio", "階乗型の階比", 2),
        ("second_order", "基本的な3項間漸化式", 3),
        ("reciprocal_affine", "一次分数・逆数", 4),
        ("weighted_sum", "正規化と重み付き総和", 5),
    ]
    .into_iter()
    .map(|(id, label, level)| FamilyInfo {
        id: id.into(),
        label: label.into(),
        levels: vec![level],
    })
    .collect();
    result.extend(crate::extensions::catalog());
    result
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeCore {
    pub id: String,
    pub kind: String,
    pub parameters: BTreeMap<String, Rational>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeBlock {
    pub id: String,
    pub input: String,
    pub kind: String,
    pub parameters: BTreeMap<String, Rational>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: String,
    pub rules_version: String,
    pub rng_version: String,
    pub family: String,
    #[serde(deserialize_with = "deserialize_recipe_seed")]
    pub seed: u64,
    pub parameters: BTreeMap<String, Rational>,
    pub core: RecipeCore,
    pub blocks: Vec<RecipeBlock>,
    pub output: String,
    pub index_start: u32,
}
// Browser files store seed as decimal text to preserve every u64 digit in
// JavaScript. Native serialization remains numeric for existing Recipe files.
fn deserialize_recipe_seed<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    struct SeedVisitor;
    impl<'de> serde::de::Visitor<'de> for SeedVisitor {
        type Value = u64;
        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a u64 integer or decimal u64 string")
        }
        fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<u64, E> {
            Ok(value)
        }
        fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<u64, E> {
            u64::try_from(value).map_err(E::custom)
        }
        fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<u64, E> {
            if value.is_empty() || value.len() > 20 || !value.bytes().all(|c| c.is_ascii_digit()) {
                return Err(E::custom("seed must be a decimal u64 integer"));
            }
            value.parse::<u64>().map_err(E::custom)
        }
    }
    deserializer.deserialize_any(SeedVisitor)
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurrenceKind {
    FirstOrder,
    SecondOrder,
    WeightedSum,
    PureSum,
    SumRelation,
    System,
    MultiplicativeSecond,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainCondition {
    pub kind: String,
    pub expression: Expr,
    pub explanation: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProblemIR {
    pub kind: RecurrenceKind,
    pub index_start: u32,
    pub recurrence_start: u32,
    pub sequence_type: String,
    pub recurrence: Equation,
    pub initials: Vec<Equation>,
    pub general_term: Equation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_recurrences: Vec<Equation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secondary_general_terms: Vec<Equation>,
    pub conditions: Vec<DomainCondition>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuxiliarySequence {
    pub name: String,
    pub definition: Equation,
    pub initial: Equation,
    pub recurrence: Equation,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationStep {
    pub text: String,
    pub equation: Option<Equation>,
    pub reason: String,
    pub conditions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DerivationIR {
    pub method_name: String,
    pub hint: String,
    pub auxiliaries: Vec<AuxiliarySequence>,
    pub steps: Vec<DerivationStep>,
    pub alternatives: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolutionStep {
    pub text: String,
    pub latex: Option<String>,
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScoreParts {
    #[serde(rename = "B")]
    pub b: u8,
    #[serde(rename = "R")]
    pub r: u8,
    #[serde(rename = "A")]
    pub a: u8,
    #[serde(rename = "T")]
    pub t: u8,
    #[serde(rename = "P")]
    pub p: u8,
    #[serde(rename = "U")]
    pub u: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpressionMetrics {
    pub nodes: usize,
    pub fraction_depth: usize,
    pub display_terms: usize,
    pub coefficient_degree: u8,
    #[serde(default)]
    pub exponent_degree: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Classification {
    pub level: u8,
    pub score: u8,
    pub total_score: u8,
    pub parts: ScoreParts,
    pub reason: String,
    pub operations: Vec<String>,
    pub numeric_cost: u8,
    pub rules_version: String,
    pub method_name: String,
    pub metrics: ExpressionMetrics,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GeneratedProblem {
    pub id: String,
    pub recipe: Recipe,
    pub problem: ProblemIR,
    pub derivation: DerivationIR,
    pub classification: Classification,
    pub statement_latex: String,
    pub hint: String,
    pub solution: Vec<SolutionStep>,
    pub answer_latex: String,
    pub family_label: String,
}

pub(crate) fn parameters(items: &[(&str, Rational)]) -> BTreeMap<String, Rational> {
    items
        .iter()
        .map(|(key, value)| (key.to_string(), value.clone()))
        .collect()
}
pub(crate) fn block(
    id: &str,
    input: &str,
    kind: &str,
    parameters: BTreeMap<String, Rational>,
) -> RecipeBlock {
    RecipeBlock {
        id: id.into(),
        input: input.into(),
        kind: kind.into(),
        parameters,
    }
}
fn build_recipe(family: &str, seed: u64, p: BTreeMap<String, Rational>) -> Result<Recipe, String> {
    if crate::extensions::supports(family) {
        return crate::extensions::build_recipe(family, seed, p);
    }
    let (core_kind, core_params, mut blocks) = match family {
        "arithmetic" => (
            "constant",
            parameters(&[("initial", p["c"].clone())]),
            vec![block(
                "b1",
                "core",
                "difference_lift",
                parameters(&[("increment", p["d"].clone())]),
            )],
        ),
        "geometric" => (
            "geometric",
            parameters(&[("amplitude", p["c"].clone()), ("ratio", p["r"].clone())]),
            vec![],
        ),
        "scaled_constant" => (
            "constant",
            parameters(&[("initial", p["c"].clone())]),
            vec![block("b1", "core", "index_scale", BTreeMap::new())],
        ),
        "factorial_ratio" => (
            "constant",
            parameters(&[("initial", p["c"].clone())]),
            vec![block(
                "b1",
                "core",
                "factorial_product",
                parameters(&[("ratio", p["r"].clone())]),
            )],
        ),
        "second_order" => (
            "geometric",
            parameters(&[("amplitude", p["c"].clone()), ("ratio", p["r"].clone())]),
            vec![block(
                "b1",
                "core",
                "linear_combination",
                parameters(&[("amplitude", p["d"].clone()), ("ratio", p["s"].clone())]),
            )],
        ),
        "reciprocal_affine" => (
            "constant",
            parameters(&[("initial", Rational::integer(1).div(&p["c"])?)]),
            vec![
                block(
                    "b1",
                    "core",
                    "difference_lift",
                    parameters(&[("increment", p["d"].clone())]),
                ),
                block("b2", "b1", "reciprocal", BTreeMap::new()),
            ],
        ),
        "weighted_sum" => {
            let (a, b) = weighted_amplitudes(&p)?;
            (
                "geometric",
                parameters(&[("amplitude", a), ("ratio", p["r"].clone())]),
                vec![
                    block(
                        "b1",
                        "core",
                        "linear_combination",
                        parameters(&[("amplitude", b), ("ratio", p["s"].clone())]),
                    ),
                    block("b2", "b1", "index_scale", BTreeMap::new()),
                    block("b3", "b2", "sum_encode", BTreeMap::new()),
                ],
            )
        }
        _ => return Err("未登録の問題の型です。".into()),
    };
    // Generated recipes use only nonidentity blocks; replay verifies this canonical DAG.
    let output = blocks
        .last()
        .map(|b| b.id.clone())
        .unwrap_or_else(|| "core".into());
    Ok(Recipe {
        schema_version: SCHEMA_VERSION.into(),
        rules_version: RULES_VERSION.into(),
        rng_version: RNG_VERSION.into(),
        family: family.into(),
        seed,
        parameters: p,
        core: RecipeCore {
            id: "core".into(),
            kind: core_kind.into(),
            parameters: core_params,
        },
        blocks: std::mem::take(&mut blocks),
        output,
        index_start: 1,
    })
}
fn weighted_amplitudes(p: &BTreeMap<String, Rational>) -> Result<(Rational, Rational), String> {
    let denominator = p["r"].sub(&p["s"]);
    Ok((
        p["c"]
            .mul(&p["r"].sub(&Rational::integer(1)))
            .div(&denominator)?,
        p["c"]
            .mul(&Rational::integer(1).sub(&p["s"]))
            .div(&denominator)?,
    ))
}
fn validate_parameters(family: &str, p: &BTreeMap<String, Rational>) -> Result<(), String> {
    if crate::extensions::supports(family) {
        return crate::extensions::validate_parameters(family, p);
    }
    let names: &[&str] = match family {
        "arithmetic" | "reciprocal_affine" => &["c", "d"],
        "geometric" | "factorial_ratio" => &["c", "r"],
        "scaled_constant" => &["c"],
        "second_order" => &["c", "d", "r", "s"],
        "weighted_sum" => &["c", "r", "s"],
        _ => return Err("未登録の型です。".into()),
    };
    if p.keys().map(String::as_str).collect::<BTreeSet<_>>() != names.iter().copied().collect() {
        return Err("Recipeのパラメータ集合が登録規則と一致しません。".into());
    }
    // Educational coefficient budget is exact; no floating point is used.
    for value in p.values() {
        if !value.is_positive() || value.0 > Rational::integer(16).0 {
            return Err("パラメータは正の有理数で16以下にしてください。".into());
        }
    }
    if let Some(r) = p.get("r") {
        if r.integer_u32().is_none() || r.0 < Rational::integer(2).0 || r.0 > Rational::integer(5).0
        {
            return Err("公比は2から5の整数です。".into());
        }
    }
    if let Some(s) = p.get("s") {
        if s.integer_u32().is_none()
            || s.0 < Rational::integer(2).0
            || s.0 > Rational::integer(5).0
            || p["r"] == *s
        {
            return Err("特性根は2から5の異なる整数です。".into());
        }
    }
    Ok(())
}
pub fn generate(level: u8, family: Option<&str>, seed: u64) -> Result<GeneratedProblem, String> {
    generate_limited(level, family, seed, 200)
}
pub fn generate_limited(
    level: u8,
    family: Option<&str>,
    seed: u64,
    max_candidates: usize,
) -> Result<GeneratedProblem, String> {
    if !(1..=5).contains(&level) {
        return Err("難易度はLv1からLv5です。".into());
    }
    let candidates = family_catalog()
        .into_iter()
        .filter(|f| f.levels.contains(&level))
        .collect::<Vec<_>>();
    if family.is_some_and(|id| !candidates.iter().any(|f| f.id == id)) {
        return Err("選択した型は指定Lvに対応していません。".into());
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let chosen = family
        .map(str::to_string)
        .unwrap_or_else(|| candidates[rng.gen_range(0..candidates.len())].id.clone());
    let preferred = [1, 2, 3, 4, 5, 6, 8, 9];
    // Sample reduced values uniformly rather than overweighting fractions with
    // several representations. The 43 distinct amplitudes exceed the 40-item
    // recent-history window even for the one-parameter scaled_constant family.
    let amplitudes = [1, 2, 3, 4, 5, 6, 8, 9, 16, 25]
        .into_iter()
        .flat_map(|num| {
            [1, 2, 3, 5, 8, 9]
                .into_iter()
                .map(move |den| Rational::fraction(num, den))
        })
        .filter(|value| value.0 <= Rational::integer(16).0)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut last_error = String::new();
    for _ in 0..max_candidates {
        let c = amplitudes[rng.gen_range(0..amplitudes.len())].clone();
        let d = Rational::integer(preferred[rng.gen_range(0..preferred.len())]);
        let r = Rational::integer(rng.gen_range(2..=5));
        let mut s = Rational::integer(rng.gen_range(2..=5));
        while s == r {
            s = Rational::integer(rng.gen_range(2..=5));
        }
        let p = match chosen.as_str() {
            "arithmetic" | "reciprocal_affine" => parameters(&[("c", c), ("d", d)]),
            "geometric" | "factorial_ratio" => parameters(&[("c", c), ("r", r)]),
            "scaled_constant" => parameters(&[("c", c)]),
            "second_order" => parameters(&[("c", c), ("d", d), ("r", r), ("s", s)]),
            "weighted_sum" => parameters(&[("c", c), ("r", r), ("s", s)]),
            _ => crate::extensions::sample(&chosen, level, &mut rng)?,
        };
        let recipe = build_recipe(&chosen, seed, p)?;
        match replay(&recipe) {
            Ok(problem)
                if problem.classification.level == level
                    && problem.classification.numeric_cost <= 8 =>
            {
                return Ok(problem)
            }
            Ok(_) => last_error = "短い解法により再分類されました。".into(),
            Err(e) => last_error = e,
        }
    }
    Err(format!(
        "品質条件を満たす候補が{max_candidates}件以内に見つかりませんでした。{last_error}"
    ))
}
pub fn replay(recipe: &Recipe) -> Result<GeneratedProblem, String> {
    if recipe.schema_version != SCHEMA_VERSION
        || recipe.rules_version
            != if crate::extensions::supports(&recipe.family) {
                EXTENSION_RULES_VERSION
            } else {
                RULES_VERSION
            }
        || recipe.rng_version != RNG_VERSION
    {
        return Err("Recipeの版がこのアプリと一致しません。".into());
    }
    validate_parameters(&recipe.family, &recipe.parameters)?;
    if *recipe != build_recipe(&recipe.family, recipe.seed, recipe.parameters.clone())? {
        return Err("Recipeの接続・ブロック・正規化が登録規則と一致しません。".into());
    }
    let problem = compile(recipe)?;
    let classification = classify(&problem)?;
    let derivation = explain(&problem, &classification)?;
    let statement_latex = format!(
        "\\begin{{gathered}}{}\\quad(n\\ge1)\\\\{}\\end{{gathered}}",
        std::iter::once(&problem.recurrence)
            .chain(&problem.secondary_recurrences)
            .map(Equation::latex)
            .collect::<Vec<_>>()
            .join("\\\\"),
        problem
            .initials
            .iter()
            .map(Equation::latex)
            .collect::<Vec<_>>()
            .join("\\quad ")
    );
    let answer_latex = format!(
        "\\begin{{gathered}}{}\\end{{gathered}}\\quad(n\\ge1)",
        std::iter::once(&problem.general_term)
            .chain(&problem.secondary_general_terms)
            .map(Equation::latex)
            .collect::<Vec<_>>()
            .join("\\\\")
    );
    let solution = derivation
        .steps
        .iter()
        .map(|step| SolutionStep {
            text: step.text.clone(),
            latex: step.equation.as_ref().map(Equation::latex),
            reason: step.reason.clone(),
        })
        .collect();
    let id = content_hash_for_version(&problem, &recipe.rules_version)?;
    let family_label = family_catalog()
        .into_iter()
        .find(|f| f.id == recipe.family)
        .ok_or("未登録の型です。")?
        .label;
    Ok(GeneratedProblem {
        id,
        recipe: recipe.clone(),
        problem,
        classification,
        hint: derivation.hint.clone(),
        derivation,
        statement_latex,
        solution,
        answer_latex,
        family_label,
    })
}
pub fn content_hash(problem: &ProblemIR) -> Result<String, String> {
    content_hash_for_version(problem, RULES_VERSION)
}
pub fn content_hash_for_version(
    problem: &ProblemIR,
    rules_version: &str,
) -> Result<String, String> {
    fn canonical_equation(equation: &mut Equation) {
        equation.lhs = equation.lhs.canonical();
        equation.rhs = equation.rhs.canonical();
    }
    let mut canonical = problem.clone();
    canonical_equation(&mut canonical.recurrence);
    canonical_equation(&mut canonical.general_term);
    canonical.initials.iter_mut().for_each(canonical_equation);
    canonical
        .secondary_recurrences
        .iter_mut()
        .for_each(canonical_equation);
    canonical
        .secondary_general_terms
        .iter_mut()
        .for_each(canonical_equation);
    for condition in &mut canonical.conditions {
        condition.expression = condition.expression.canonical();
    }
    let bytes = serde_json::to_vec(&(rules_version, canonical)).map_err(|e| e.to_string())?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
pub(crate) fn initial(index: u32, value: Rational) -> Equation {
    Equation::new(
        Expr::Term {
            sequence: "a".into(),
            index: NatExpr::constant(index),
        },
        Expr::rational(value),
    )
}
fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    if crate::extensions::supports(&recipe.family) {
        return crate::extensions::compile(recipe);
    }
    let p = &recipe.parameters;
    let c = Expr::rational(p["c"].clone());
    let a = Expr::term("a", 0);
    let n = Expr::index();
    let next = Expr::offset(1);
    let nm1 = Expr::offset(-1);
    let exponent = NatExpr::offset(-1);
    let mut kind = RecurrenceKind::FirstOrder;
    let mut conditions = Vec::new();
    let (rhs, formula, initials) = match recipe.family.as_str() {
        "arithmetic" => (
            Expr::add(vec![a, Expr::rational(p["d"].clone())]),
            Expr::add(vec![
                c,
                Expr::mul(vec![Expr::rational(p["d"].clone()), nm1]),
            ]),
            vec![initial(1, p["c"].clone())],
        ),
        "geometric" => {
            let r = Expr::rational(p["r"].clone());
            (
                Expr::mul(vec![r.clone(), a]),
                Expr::mul(vec![c, Expr::pow(r, exponent)]),
                vec![initial(1, p["c"].clone())],
            )
        }
        "scaled_constant" => {
            conditions.push(nonzero(
                n.clone(),
                "自然数nは1以上なので、係数の分母nは零にならない。",
            ));
            (
                Expr::mul(vec![Expr::div(next, n.clone()), a]),
                Expr::mul(vec![c, n]),
                vec![initial(1, p["c"].clone())],
            )
        }
        "factorial_ratio" => {
            let r = Expr::rational(p["r"].clone());
            (
                Expr::mul(vec![r.clone(), n, a]),
                Expr::mul(vec![
                    c,
                    Expr::pow(r, exponent.clone()),
                    Expr::Factorial { index: exponent },
                ]),
                vec![initial(1, p["c"].clone())],
            )
        }
        "second_order" => {
            kind = RecurrenceKind::SecondOrder;
            let r = p["r"].clone();
            let s = p["s"].clone();
            let rhs = Expr::add(vec![
                Expr::mul(vec![Expr::rational(r.add(&s)), Expr::term("a", 1)]),
                Expr::mul(vec![
                    Expr::rational(Rational::integer(-1).mul(&r).mul(&s)),
                    a,
                ]),
            ]);
            let formula = Expr::add(vec![
                Expr::mul(vec![
                    c,
                    Expr::pow(Expr::rational(r.clone()), exponent.clone()),
                ]),
                Expr::mul(vec![
                    Expr::rational(p["d"].clone()),
                    Expr::pow(Expr::rational(s.clone()), exponent),
                ]),
            ]);
            (
                rhs,
                formula,
                vec![
                    initial(1, p["c"].add(&p["d"])),
                    initial(2, p["c"].mul(&r).add(&p["d"].mul(&s))),
                ],
            )
        }
        "reciprocal_affine" => {
            let d = Expr::rational(p["d"].clone());
            let denominator = Expr::add(vec![
                Expr::integer(1),
                Expr::mul(vec![d.clone(), a.clone()]),
            ]);
            conditions.push(nonzero(
                denominator.clone(),
                "一般項は正なので、元の分母1+d a_nは正であり零にならない。",
            ));
            conditions.push(nonzero(
                a.clone(),
                "正の初項と分母からすべての項が正となり、逆数が定義できる。",
            ));
            let formula = Expr::div(
                c.clone(),
                Expr::add(vec![Expr::integer(1), Expr::mul(vec![d, c, nm1])]),
            );
            (
                Expr::div(a, denominator),
                formula,
                vec![initial(1, p["c"].clone())],
            )
        }
        "weighted_sum" => {
            kind = RecurrenceKind::WeightedSum;
            let (amplitude_a, amplitude_b) = weighted_amplitudes(p)?;
            let alpha = p["r"].mul(&p["s"]);
            let beta = p["r"].add(&p["s"]).sub(&Rational::integer(1)).sub(&alpha);
            let k = NatExpr::Bound { name: "k".into() };
            let sum = Expr::Sum {
                variable: "k".into(),
                lower: NatExpr::constant(1),
                upper: NatExpr::n(),
                body: Box::new(Expr::div(
                    Expr::Term {
                        sequence: "a".into(),
                        index: k.clone(),
                    },
                    Expr::NatCast { value: k },
                )),
            };
            let rhs = Expr::mul(vec![
                next,
                Expr::add(vec![
                    Expr::mul(vec![Expr::rational(alpha), Expr::div(a, n.clone())]),
                    Expr::mul(vec![Expr::rational(beta), sum]),
                ]),
            ]);
            let formula = Expr::mul(vec![
                n.clone(),
                Expr::add(vec![
                    Expr::mul(vec![
                        Expr::rational(amplitude_a),
                        Expr::pow(Expr::rational(p["r"].clone()), exponent.clone()),
                    ]),
                    Expr::mul(vec![
                        Expr::rational(amplitude_b),
                        Expr::pow(Expr::rational(p["s"].clone()), exponent),
                    ]),
                ]),
            ]);
            conditions.push(nonzero(
                n,
                "nと総和の添字kは1以上なので、正規化の分母は零にならない。",
            ));
            (rhs, formula, vec![initial(1, p["c"].clone())])
        }
        _ => return Err("未登録の型です。".into()),
    };
    let lhs = Expr::term(
        "a",
        if kind == RecurrenceKind::SecondOrder {
            2
        } else {
            1
        },
    );
    Ok(ProblemIR {
        kind,
        index_start: 1,
        recurrence_start: 1,
        sequence_type: "rational".into(),
        recurrence: Equation::new(lhs, rhs),
        initials,
        general_term: Equation::new(Expr::term("a", 0), formula),
        secondary_recurrences: Vec::new(),
        secondary_general_terms: Vec::new(),
        conditions,
    })
}
pub(crate) fn nonzero(expression: Expr, explanation: &str) -> DomainCondition {
    DomainCondition {
        kind: "nonzero".into(),
        expression,
        explanation: explanation.into(),
    }
}

pub(crate) fn contains_term(expr: &Expr, target: &Expr) -> bool {
    if expr == target {
        return true;
    }
    match expr {
        Expr::Add { terms } => terms.iter().any(|x| contains_term(x, target)),
        Expr::Mul { factors } => factors.iter().any(|x| contains_term(x, target)),
        Expr::Div {
            numerator,
            denominator,
        } => contains_term(numerator, target) || contains_term(denominator, target),
        Expr::Pow { base, .. } => contains_term(base, target),
        Expr::Sum { body, .. } => contains_term(body, target),
        _ => false,
    }
}
/// Extract coefficient and constant solely from the displayed recurrence tree.
pub(crate) fn linear_split(expr: &Expr, target: &Expr) -> Result<(Expr, Expr), String> {
    if expr == target {
        return Ok((Expr::integer(1), Expr::integer(0)));
    }
    if !contains_term(expr, target) {
        return Ok((Expr::integer(0), expr.clone()));
    }
    match expr {
        Expr::Add { terms } => {
            let pairs = terms
                .iter()
                .map(|x| linear_split(x, target))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((
                Expr::add(pairs.iter().map(|p| p.0.clone()).collect()),
                Expr::add(pairs.into_iter().map(|p| p.1).collect()),
            ))
        }
        Expr::Mul { factors } => {
            let mut coefficient = Expr::integer(1);
            let mut constant = Expr::integer(1);
            let mut dependent = false;
            for factor in factors {
                let (a, b) = linear_split(factor, target)?;
                if a != Expr::integer(0) {
                    if dependent {
                        return Err("非線形の項です。".into());
                    }
                    dependent = true;
                    coefficient = Expr::mul(vec![coefficient, a]);
                    constant = Expr::mul(vec![constant, b]);
                } else {
                    coefficient = Expr::mul(vec![coefficient, b.clone()]);
                    constant = Expr::mul(vec![constant, b]);
                }
            }
            Ok((coefficient, constant))
        }
        Expr::Div {
            numerator,
            denominator,
        } if !contains_term(denominator, target) => {
            let (a, b) = linear_split(numerator, target)?;
            Ok((
                Expr::div(a, *denominator.clone()),
                Expr::div(b, *denominator.clone()),
            ))
        }
        _ => Err("登録済みの一次変形では処理できません。".into()),
    }
}
pub(crate) fn literal(expr: &Expr) -> Option<Rational> {
    if let Expr::Rational { value } = expr {
        Some(value.clone())
    } else {
        None
    }
}
pub(crate) fn has_sum(expr: &Expr) -> bool {
    match expr {
        Expr::Sum { .. } => true,
        Expr::Add { terms } => terms.iter().any(has_sum),
        Expr::Mul { factors } => factors.iter().any(has_sum),
        Expr::Div {
            numerator,
            denominator,
        } => has_sum(numerator) || has_sum(denominator),
        _ => false,
    }
}
pub(crate) fn initial_value(problem: &ProblemIR, index: usize) -> Result<Rational, String> {
    literal(
        &problem
            .initials
            .get(index)
            .ok_or("初期条件が不足しています。")?
            .rhs,
    )
    .ok_or_else(|| "初期条件が数値ではありません。".into())
}

pub fn classify(problem: &ProblemIR) -> Result<Classification, String> {
    if let Some(result) = crate::extensions::classify(problem)? {
        return Ok(result);
    }
    let rhs = &problem.recurrence.rhs;
    let a = Expr::term("a", 0);
    let (score, reason, method, operations, discovery, domain, degree) = match problem.kind {
        RecurrenceKind::WeightedSum if has_sum(rhs) => (
            20,
            "重みをそろえ、総和を消し、3項間を解く",
            "正規化して総和を消す",
            vec!["index_scale", "eliminate_sum", "characteristic_distinct"],
            3,
            1,
            1,
        ),
        RecurrenceKind::SecondOrder => {
            let (p, rest) = linear_split(rhs, &Expr::term("a", 1))?;
            let (q, constant) = linear_split(&rest, &a)?;
            let p = literal(&p).ok_or("3項間の係数が定数ではありません。")?;
            let q = literal(&q).ok_or("3項間の係数が定数ではありません。")?;
            if constant != Expr::integer(0) {
                return Err("定数項付き3項間の解法は未登録です。".into());
            }
            let first = initial_value(problem, 0)?;
            let second = initial_value(problem, 1)?;
            let ratio = second.div(&first)?;
            if ratio.mul(&ratio) == p.mul(&ratio).add(&q) {
                (
                    2,
                    "初期条件から短い等比数列の解法が成立する",
                    "等比数列として解く",
                    vec!["geometric"],
                    0,
                    0,
                    0,
                )
            } else {
                (
                    9,
                    "基本的な3項間を特性方程式の異なる2根で解く",
                    "特性方程式を解く",
                    vec!["characteristic_distinct"],
                    2,
                    0,
                    0,
                )
            }
        }
        RecurrenceKind::FirstOrder => {
            if let Expr::Div {
                numerator,
                denominator,
            } = rhs
            {
                if **numerator == a {
                    let (d, one) = linear_split(denominator, &a)?;
                    if literal(&d).is_some_and(|x| x.is_positive()) && one == Expr::integer(1) {
                        (
                            14,
                            "定数係数の一次分数を逆数に帰着させる",
                            "逆数を等差数列にする",
                            vec!["reciprocal", "difference_sum", "evaluate_sum"],
                            1,
                            1,
                            0,
                        )
                    } else {
                        return Err("一次分数の前向き解法が見つかりません。".into());
                    }
                } else {
                    return Err("分数の前向き解法が見つかりません。".into());
                }
            } else {
                let (coefficient, forcing) = linear_split(rhs, &a)?;
                if coefficient == Expr::integer(1)
                    && literal(&forcing).is_some_and(|x| !x.is_zero())
                {
                    (
                        3,
                        "等差数列の基本形",
                        "等差数列として解く",
                        vec!["difference_sum", "evaluate_sum"],
                        0,
                        0,
                        0,
                    )
                } else if literal(&coefficient).is_some_and(|x| !x.is_zero() && !x.is_one())
                    && forcing == Expr::integer(0)
                {
                    (
                        2,
                        "等比数列の基本形",
                        "等比数列として解く",
                        vec!["geometric"],
                        0,
                        0,
                        0,
                    )
                } else if coefficient == Expr::div(Expr::offset(1), Expr::index())
                    && forcing == Expr::integer(0)
                {
                    (
                        6,
                        "単項式の倍率の階比を相殺する",
                        "nで割って正規化する",
                        vec!["ratio_product", "evaluate_product"],
                        1,
                        1,
                        1,
                    )
                } else if matches!(&coefficient,Expr::Mul{factors} if factors.contains(&Expr::index()) && factors.iter().all(|x|matches!(x,Expr::Rational{..}|Expr::NatCast{value:NatExpr::Index})))
                    && forcing == Expr::integer(0)
                {
                    (
                        6,
                        "単項式の階比を総積で処理する",
                        "階比を階乗にまとめる",
                        vec!["ratio_product", "evaluate_product"],
                        1,
                        0,
                        1,
                    )
                } else {
                    return Err("問題の係数から完結する登録解法を検出できません。".into());
                }
            }
        }
        _ => return Err("総和の解法を検出できません。".into()),
    };
    let metrics = ExpressionMetrics {
        nodes: problem.recurrence.lhs.nodes() + rhs.nodes(),
        fraction_depth: rhs.fraction_depth(),
        display_terms: 1 + rhs.add_terms(),
        coefficient_degree: degree,
        exponent_degree: problem
            .recurrence
            .lhs
            .exponent_degree()
            .max(rhs.exponent_degree()),
    };
    if metrics.nodes > 44
        || metrics.fraction_depth > 2
        || metrics.display_terms > 8
        || metrics.coefficient_degree > 2
    {
        return Err("問題の数式の複雑さが上限を超えています。".into());
    }
    let mut values = BTreeSet::new();
    collect_rationals(rhs, &mut values);
    for eq in &problem.initials {
        collect_rationals(&eq.rhs, &mut values);
    }
    if problem.kind == RecurrenceKind::WeightedSum {
        let (alpha, beta) = weighted_coefficients(rhs)?;
        let p = alpha.add(&beta).add(&Rational::integer(1));
        let q = Rational::integer(-1).mul(&alpha);
        let (r, s) = characteristic_roots(&p, &q)?;
        let first = initial_value(problem, 0)?;
        let second = alpha.add(&beta).mul(&first);
        let x = second.sub(&s.mul(&first)).div(&r.sub(&s))?;
        let y = first.sub(&x);
        values.extend([alpha, beta, p, q, r, s, first, second, x, y]);
    }
    let numeric_cost = values.iter().map(numeric_cost).sum::<u16>().min(255) as u8;
    // Version 1.0 saved weighted-sum recipes retain replay compatibility.
    // New generation rejects their enlarged, accurately measured budget.
    if numeric_cost > 8 && problem.kind != RecurrenceKind::WeightedSum {
        return Err("数値の計算コストが上限8を超えています。".into());
    }
    let b = operations
        .iter()
        .map(|operation| match *operation {
            "constant" => 1,
            "geometric" => 2,
            "reciprocal" => 3,
            "difference_sum" => 2,
            "evaluate_sum" => 1,
            "ratio_product" => 2,
            "evaluate_product" => 1,
            "index_scale" => 3,
            "eliminate_sum" => 3,
            "characteristic_distinct" => 5,
            _ => 0,
        })
        .sum();
    let expression = (metrics.nodes.saturating_sub(18).div_ceil(10)
        + metrics.fraction_depth.saturating_sub(1))
    .min(6) as u8;
    let parts = ScoreParts {
        b,
        r: discovery,
        a: numeric_cost.min(6),
        t: domain,
        p: expression,
        u: 0,
    };
    let total_score = parts.b + parts.r + parts.a + parts.t + parts.p;
    let level = match score {
        0..=3 => 1,
        4..=7 => 2,
        8..=12 => 3,
        13..=18 => 4,
        19..=25 => 5,
        _ => return Err("スコアがLvの範囲外です。".into()),
    };
    Ok(Classification {
        level,
        score,
        total_score,
        parts,
        reason: reason.into(),
        operations: operations.into_iter().map(str::to_string).collect(),
        numeric_cost,
        rules_version: RULES_VERSION.into(),
        method_name: method.into(),
        metrics,
    })
}
pub(crate) fn collect_rationals(expr: &Expr, result: &mut BTreeSet<Rational>) {
    match expr {
        Expr::Rational { value } => {
            result.insert(value.clone());
        }
        Expr::Add { terms } => terms.iter().for_each(|x| collect_rationals(x, result)),
        Expr::Mul { factors } => factors.iter().for_each(|x| collect_rationals(x, result)),
        Expr::Div {
            numerator,
            denominator,
        } => {
            collect_rationals(numerator, result);
            collect_rationals(denominator, result);
        }
        Expr::Pow { base, .. } => collect_rationals(base, result),
        Expr::Log { base, argument } => {
            collect_rationals(base, result);
            collect_rationals(argument, result);
        }
        Expr::Sum { body, .. } => collect_rationals(body, result),
        _ => {}
    }
}
pub(crate) fn numeric_cost(value: &Rational) -> u16 {
    fn component(n: u64, preferred: &[u64]) -> u16 {
        if preferred.contains(&n) {
            return 0;
        }
        let mut cost = if n <= 10 {
            1
        } else if n <= 30 {
            2
        } else if n <= 100 {
            3
        } else if n <= 1000 {
            4
        } else {
            (5 + n.to_string().len().saturating_sub(4)).min(8) as u16
        };
        let square = (0..=10).any(|k| k * k == n);
        let power = [2u64, 3, 5]
            .iter()
            .any(|&b| (1..8).any(|e| b.pow(e) == n && n <= 125));
        if n <= 100 && square || power {
            cost = cost.saturating_sub(1);
        }
        cost
    }
    let num = value.0.numer().abs().to_u64().unwrap_or(u64::MAX);
    let den = value.0.denom().to_u64().unwrap_or(u64::MAX);
    component(num, &[0, 1, 2, 3, 4, 5, 6, 8, 9, 16, 25]) + component(den, &[1, 2, 3, 5, 8, 9])
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn make_classification(
    problem: &ProblemIR,
    score: u8,
    method: &str,
    reason: &str,
    operations: &[&str],
    discovery: u8,
    domain: u8,
    coefficient_degree: u8,
) -> Result<Classification, String> {
    let equations = std::iter::once(&problem.recurrence).chain(&problem.secondary_recurrences);
    let mut metrics = ExpressionMetrics {
        nodes: 0,
        fraction_depth: 0,
        display_terms: 0,
        coefficient_degree,
        exponent_degree: 0,
    };
    for eq in equations {
        metrics.nodes = metrics.nodes.max(eq.lhs.nodes() + eq.rhs.nodes());
        metrics.fraction_depth = metrics
            .fraction_depth
            .max(eq.lhs.fraction_depth().max(eq.rhs.fraction_depth()));
        metrics.display_terms = metrics
            .display_terms
            .max(1 + eq.lhs.add_terms() + eq.rhs.add_terms());
        metrics.exponent_degree = metrics
            .exponent_degree
            .max(eq.lhs.exponent_degree().max(eq.rhs.exponent_degree()));
    }
    if metrics.nodes > 44
        || metrics.fraction_depth > 2
        || metrics.display_terms > 8
        || coefficient_degree > 2
        || discovery > 3
    {
        return Err("問題の数式の複雑さが上限を超えています。".into());
    }
    let mut values = BTreeSet::new();
    // Classification is a function of the question. The supplied answer is
    // verified separately and cannot influence the detected route or budget.
    for eq in std::iter::once(&problem.recurrence)
        .chain(&problem.secondary_recurrences)
        .chain(&problem.initials)
    {
        collect_rationals(&eq.lhs, &mut values);
        collect_rationals(&eq.rhs, &mut values);
    }
    let numeric_cost = values.iter().map(numeric_cost).sum::<u16>().min(255) as u8;
    if numeric_cost > 8 {
        return Err("数値の計算コストが上限8を超えています。".into());
    }
    let b = operations
        .iter()
        .map(|op| match *op {
            "constant" => 1,
            "geometric"
            | "fixed_point"
            | "shift"
            | "ratio_product"
            | "difference_sum"
            | "difference"
            | "evaluate_quadratic_sum"
            | "split_pair"
            | "recover_pair"
            | "recover_sum" => 2,
            "index_scale" | "reciprocal" | "logarithm" | "eliminate_sum" => 3,
            "evaluate_product" | "evaluate_sum" => 1,
            "characteristic_distinct" => 5,
            "characteristic_repeated" => 6,
            "polynomial_shift" | "geometric_sum" => 4,
            _ => 0,
        })
        .sum();
    let expression = (metrics.nodes.saturating_sub(18).div_ceil(10)
        + metrics.fraction_depth.saturating_sub(1)
        + usize::from(coefficient_degree.saturating_sub(1))
        + usize::from(metrics.exponent_degree.saturating_sub(1)))
    .min(6) as u8;
    let parts = ScoreParts {
        b,
        r: discovery,
        a: numeric_cost.min(6),
        t: domain,
        p: expression,
        u: 0,
    };
    let total_score = parts.b + parts.r + parts.a + parts.t + parts.p;
    let level = match score {
        0..=3 => 1,
        4..=7 => 2,
        8..=12 => 3,
        13..=18 => 4,
        19..=25 => 5,
        _ => return Err("スコアがLvの範囲外です。".into()),
    };
    Ok(Classification {
        level,
        score,
        total_score,
        parts,
        reason: reason.into(),
        operations: operations.iter().map(|s| s.to_string()).collect(),
        numeric_cost,
        rules_version: EXTENSION_RULES_VERSION.into(),
        method_name: method.into(),
        metrics,
    })
}

pub(crate) fn include_intermediate_values(
    problem: &ProblemIR,
    mut classification: Classification,
    intermediate: &[Rational],
) -> Result<Classification, String> {
    let mut values = intermediate.iter().cloned().collect::<BTreeSet<_>>();
    for eq in std::iter::once(&problem.recurrence)
        .chain(&problem.secondary_recurrences)
        .chain(&problem.initials)
    {
        collect_rationals(&eq.lhs, &mut values);
        collect_rationals(&eq.rhs, &mut values);
    }
    classification.numeric_cost = values.iter().map(numeric_cost).sum::<u16>().min(255) as u8;
    if classification.numeric_cost > 8 {
        return Err("数値の計算コストが上限8を超えています。".into());
    }
    classification.parts.a = classification.numeric_cost.min(6);
    let parts = &classification.parts;
    classification.total_score = parts.b + parts.r + parts.a + parts.t + parts.p + parts.u;
    Ok(classification)
}

pub(crate) fn step(text: &str, equation: Option<Equation>, reason: &str) -> DerivationStep {
    DerivationStep {
        text: text.into(),
        equation,
        reason: reason.into(),
        conditions: Vec::new(),
    }
}
fn explain(problem: &ProblemIR, classification: &Classification) -> Result<DerivationIR, String> {
    if let Some(result) = crate::extensions::explain(problem, classification)? {
        return Ok(result);
    }
    let rhs = &problem.recurrence.rhs;
    let a = Expr::term("a", 0);
    let mut auxiliaries = Vec::new();
    let (hint, mut steps) = match classification.method_name.as_str() {
        "等差数列として解く" => {
            let (_, d) = linear_split(rhs, &a)?;
            let first = initial_value(problem, 0)?;
            ("次の項と現在の項の差が一定であることに着目する。",vec![step("与式では隣接する項の差が一定なので、等差数列である。",Some(Equation::new(Expr::sub(Expr::term("a",1),a.clone()),d.clone())),"漸化式の両辺からa_nを引く"),step("初項から第n項まではn−1回だけ同じ差を加える。したがって一般項は次のとおりである。",Some(Equation::new(a.clone(),Expr::add(vec![Expr::rational(first),Expr::mul(vec![d,Expr::offset(-1)])]))),"等差数列の一般項")])
        }
        "等比数列として解く" => {
            let (r, _) = linear_split(rhs, &a)?;
            (
                "次の項が現在の項の何倍かを調べる。",
                vec![
                    step(
                        "与式は等比数列の形である。初項からn−1回公比を掛け合わせる。",
                        Some(Equation::new(Expr::div(Expr::term("a", 1), a.clone()), r)),
                        "初項が正で公比が非零なので各項は非零",
                    ),
                    step(
                        "初項を代入すると、一般項が得られる。",
                        Some(problem.general_term.clone()),
                        "等比数列の一般項",
                    ),
                ],
            )
        }
        "nで割って正規化する" => {
            let definition = Equation::new(Expr::term("b", 0), Expr::div(a.clone(), Expr::index()));
            let recurrence = Equation::new(Expr::term("b", 1), Expr::term("b", 0));
            let initial = Equation::new(
                Expr::Term {
                    sequence: "b".into(),
                    index: NatExpr::constant(1),
                },
                Expr::rational(initial_value(problem, 0)?),
            );
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: definition.clone(),
                initial: initial.clone(),
                recurrence: recurrence.clone(),
            });
            (
                "係数の(n+1)/nを見て、a_nをnで割った数列を考える。",
                vec![
                    step(
                        "nは1以上なので、nで割る置換を定義できる。",
                        Some(definition),
                        "分母nが正",
                    ),
                    step(
                        "与式の両辺をn+1で割ると、補助数列は一定となる。",
                        Some(recurrence),
                        "共通の倍率を相殺する",
                    ),
                    step("補助数列の初項は次の値である。", Some(initial), "n=1を代入"),
                    step(
                        "a_n=n b_nに戻すと、一般項を得る。",
                        Some(problem.general_term.clone()),
                        "逆変換",
                    ),
                ],
            )
        }
        "階比を階乗にまとめる" => {
            let (ratio, _) = linear_split(rhs, &a)?;
            ("隣接する項の比を掛け合わせると、連続する整数の積が現れる。",vec![step("初項とすべての倍率は正なので、各項は正であり階比をとることができる。",Some(Equation::new(Expr::div(Expr::term("a",1),a.clone()),ratio)),"初項が正かつ倍率が正"),step("この比を初項から第n項まで掛け合わせる。途中の数列の項は相殺され、整数1からn−1までの積は(n−1)!となる。",Some(problem.general_term.clone()),"階比の総積と階乗の定義"),step("n=1のときも0!=1より初期条件と一致する。",None,"空積は1")])
        }
        "特性方程式を解く" => {
            let (p, rest) = linear_split(rhs, &Expr::term("a", 1))?;
            let (q, _) = linear_split(&rest, &a)?;
            let pr = literal(&p).ok_or("係数が定数でありません。")?;
            let qr = literal(&q).ok_or("係数が定数でありません。")?;
            let roots = characteristic_roots(&pr, &qr)?;
            let first = initial_value(problem, 0)?;
            let second = initial_value(problem, 1)?;
            let x = second
                .sub(&first.mul(&roots.1))
                .div(&roots.0.sub(&roots.1))?;
            let y = first.sub(&x);
            let root_eq = Equation::new(
                Expr::integer(0),
                Expr::add(vec![
                    Expr::pow(
                        Expr::Term {
                            sequence: "t".into(),
                            index: NatExpr::constant(0),
                        },
                        NatExpr::constant(2),
                    ),
                    Expr::mul(vec![
                        Expr::rational(Rational::integer(-1).mul(&pr)),
                        Expr::Term {
                            sequence: "t".into(),
                            index: NatExpr::constant(0),
                        },
                    ]),
                    Expr::rational(Rational::integer(-1).mul(&qr)),
                ]),
            );
            let formula = Expr::add(vec![
                Expr::mul(vec![
                    Expr::rational(x),
                    Expr::pow(Expr::rational(roots.0.clone()), NatExpr::offset(-1)),
                ]),
                Expr::mul(vec![
                    Expr::rational(y),
                    Expr::pow(Expr::rational(roots.1.clone()), NatExpr::offset(-1)),
                ]),
            ]);
            ("3項間の定数係数から特性方程式を立て、異なる2根を求める。",vec![step("等比数列の形を代入して特性方程式を立てる。",Some(root_eq),"a_n=t^(n−1)を代入して係数を比較"),step(&format!("特性方程式の異なる2根は{}と{}である。一般解はこの2つの等比数列の一次結合となる。",roots.0,roots.1),None,"異なる特性根の解法"),step("2つの初期条件を代入して係数を定めると、次の一般項が得られる。",Some(Equation::new(a.clone(),formula)),"2元一次連立方程式を解く")])
        }
        "逆数を等差数列にする" => {
            let denominator = if let Expr::Div { denominator, .. } = rhs {
                denominator
            } else {
                return Err("一次分数ではありません。".into());
            };
            let (d, _) = linear_split(denominator, &a)?;
            let first = Rational::integer(1).div(&initial_value(problem, 0)?)?;
            let definition =
                Equation::new(Expr::term("b", 0), Expr::div(Expr::integer(1), a.clone()));
            let recurrence = Equation::new(
                Expr::term("b", 1),
                Expr::add(vec![Expr::term("b", 0), d.clone()]),
            );
            let initial = Equation::new(
                Expr::Term {
                    sequence: "b".into(),
                    index: NatExpr::constant(1),
                },
                Expr::rational(first.clone()),
            );
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: definition.clone(),
                initial: initial.clone(),
                recurrence: recurrence.clone(),
            });
            ("分子にa_nがある一次分数なので、両辺の逆数をとると加法の漸化式になる。",vec![step("初項は正で、正の項を代入したとき元の分母は正である。帰納的に各項は正となるので、次の置換を定義できる。",Some(definition),"初期条件と正値の帰納法"),step("両辺の逆数をとって整理する。補助数列は等差数列となる。",Some(recurrence),"元の分母と各項が非零"),step("補助数列の初期条件は次のとおりである。",Some(initial),"初項の逆数"),step("等差数列の一般項を用いる。",Some(Equation::new(Expr::term("b",0),Expr::add(vec![Expr::rational(first),Expr::mul(vec![d,Expr::offset(-1)])]))),"等差数列の一般項"),step("a_n=1/b_nに戻す。この補助数列は全てのnで正なので、逆変換と元の分母は全て定義できる。",Some(problem.general_term.clone()),"正値・非零を確認して逆変換")])
        }
        "正規化して総和を消す" => {
            let (alpha, beta) = weighted_coefficients(rhs)?;
            let p = alpha.add(&beta).add(&Rational::integer(1));
            let q = Rational::integer(-1).mul(&alpha);
            let roots = characteristic_roots(&p, &q)?;
            let definition = Equation::new(Expr::term("b", 0), Expr::div(a.clone(), Expr::index()));
            let k = NatExpr::Bound { name: "k".into() };
            let sum = Expr::Sum {
                variable: "k".into(),
                lower: NatExpr::constant(1),
                upper: NatExpr::n(),
                body: Box::new(Expr::Term {
                    sequence: "b".into(),
                    index: k,
                }),
            };
            let recurrence = Equation::new(
                Expr::term("b", 1),
                Expr::add(vec![
                    Expr::mul(vec![Expr::rational(alpha.clone()), Expr::term("b", 0)]),
                    Expr::mul(vec![Expr::rational(beta.clone()), sum]),
                ]),
            );
            let first = initial_value(problem, 0)?;
            let second = alpha.add(&beta).mul(&first);
            let initial = Equation::new(
                Expr::Term {
                    sequence: "b".into(),
                    index: NatExpr::constant(1),
                },
                Expr::rational(first.clone()),
            );
            auxiliaries.push(AuxiliarySequence {
                name: "b".into(),
                definition: definition.clone(),
                initial: initial.clone(),
                recurrence: recurrence.clone(),
            });
            let second_eq = Equation::new(
                Expr::term("b", 2),
                Expr::add(vec![
                    Expr::mul(vec![Expr::rational(p), Expr::term("b", 1)]),
                    Expr::mul(vec![Expr::rational(q), Expr::term("b", 0)]),
                ]),
            );
            let x = second
                .sub(&first.mul(&roots.1))
                .div(&roots.0.sub(&roots.1))?;
            let y = first.sub(&x);
            let b_formula = Expr::add(vec![
                Expr::mul(vec![
                    Expr::rational(x),
                    Expr::pow(Expr::rational(roots.0.clone()), NatExpr::offset(-1)),
                ]),
                Expr::mul(vec![
                    Expr::rational(y),
                    Expr::pow(Expr::rational(roots.1.clone()), NatExpr::offset(-1)),
                ]),
            ]);
            ("a_n/nで重みをそろえ、隣接する2式の差をとって総和を消す。",vec![step("分母nは正なので、重みをそろえる補助数列を定義する。",Some(definition),"n≥1"),step("与式をn+1で割って補助数列の漸化式を得る。",Some(recurrence),"正規化"),step("この式の添字を1つ進めた式から元の式を引くと、総和が消える。整理して次の3項間漸化式となる。",Some(second_eq),"隣接する総和の差はb_(n+1)"),step(&format!("正規化した初項は{}である。総和の式にn=1を代入するとb_2={}となる。特性方程式の異なる2根は{}と{}である。",first,second,roots.0,roots.1),Some(initial),"総和の境界と初期条件を対応させる"),step("3項間漸化式の一般解に2つの初期条件を代入して係数を定める。",Some(Equation::new(Expr::term("b",0),b_formula)),"異なる特性根と初期条件"),step("a_n=n b_nに戻す。n=1の初期条件を満たし、隣接する総和の差と初期境界から元の総和の式も全てのnで成立する。",Some(problem.general_term.clone()),"正規化の逆変換と総和の初期境界")])
        }
        _ => return Err("教材出力の解法が未登録です。".into()),
    };
    steps.push(step("得られた一般項は初期条件と漸化式をすべての添字で満たす。初期条件から順に項が決まるため、この数列は一意である。",None,"この内容は各問題のLean証明の検証対象"));
    Ok(DerivationIR {
        method_name: classification.method_name.clone(),
        hint: hint.into(),
        auxiliaries,
        steps,
        alternatives: vec![],
    })
}
fn characteristic_roots(p: &Rational, q: &Rational) -> Result<(Rational, Rational), String> {
    // Roots of t²-p t-q=0. Registered coefficients have bounded rational roots.
    for r in 1..=16 {
        for s in (r + 1)..=16 {
            let a = Rational::integer(r);
            let b = Rational::integer(s);
            if a.add(&b) == *p && Rational::integer(-1).mul(&a).mul(&b) == *q {
                return Ok((a, b));
            }
        }
    }
    Err("登録範囲の異なる有理特性根を検出できません。".into())
}
fn weighted_coefficients(rhs: &Expr) -> Result<(Rational, Rational), String> {
    let terms = if let Expr::Mul { factors } = rhs {
        factors
            .iter()
            .find_map(|x| {
                if let Expr::Add { terms } = x {
                    Some(terms)
                } else {
                    None
                }
            })
            .ok_or("総和の正規化形ではありません。")?
    } else {
        return Err("総和の正規化形ではありません。".into());
    };
    let mut alpha = None;
    let mut beta = None;
    for term in terms {
        if let Expr::Mul { factors } = term {
            let coefficient = factors
                .iter()
                .find_map(literal)
                .unwrap_or_else(|| Rational::integer(1));
            if factors.iter().any(has_sum) {
                beta = Some(coefficient);
            } else {
                alpha = Some(coefficient);
            }
        } else if has_sum(term) {
            beta = Some(Rational::integer(1));
        }
    }
    Ok((
        alpha.ok_or("総和係数が不足しています。")?,
        beta.ok_or("総和係数が不足しています。")?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_family_replays_and_satisfies_exact_recurrence() {
        for family in family_catalog() {
            for seed in 0..8 {
                let generated = generate(family.levels[0], Some(&family.id), seed).unwrap();
                let replayed = replay(&generated.recipe).unwrap();
                assert_eq!(generated.id, replayed.id);
                assert_eq!(generated.problem, replayed.problem);
                let formula = &generated.problem.general_term.rhs;
                let term = |sequence: &str, n: u32| {
                    let expression = match sequence {
                        "a" => formula,
                        "b" => &generated.problem.secondary_general_terms[0].rhs,
                        _ => return Err("unregistered sequence".into()),
                    };
                    expression.evaluate(n, &|_, _| Err("formula must be closed".into()))
                };
                for n in 1..12 {
                    for equation in std::iter::once(&generated.problem.recurrence)
                        .chain(&generated.problem.secondary_recurrences)
                    {
                        let lhs = equation.lhs.evaluate(n, &term).unwrap();
                        let rhs = equation.rhs.evaluate(n, &term).unwrap();
                        assert_eq!(lhs, rhs, "{} n={n}", family.id);
                    }
                }
                for initial in &generated.problem.initials {
                    assert_eq!(
                        initial.lhs.evaluate(1, &term).unwrap(),
                        initial.rhs.evaluate(1, &term).unwrap()
                    );
                }
            }
        }
    }
    #[test]
    fn recipe_changes_invalidate_identity_and_version() {
        let generated = generate(1, Some("arithmetic"), 7).unwrap();
        let mut altered = generated.recipe.clone();
        altered.rules_version = "other".into();
        assert!(replay(&altered).is_err());
        altered = generated.recipe.clone();
        altered.blocks[0].input = "b1".into();
        assert!(replay(&altered).is_err());
        altered = generated.recipe.clone();
        altered.seed += 1;
        assert_eq!(replay(&altered).unwrap().id, generated.id);
    }
    #[test]
    fn browser_and_native_recipe_seeds_preserve_u64_and_content_identity() {
        for seed in [0, 9_007_199_254_740_992, u64::MAX] {
            let generated = generate(1, Some("arithmetic"), seed).unwrap();
            let native = serde_json::to_value(&generated.recipe).unwrap();
            assert_eq!(native["seed"].as_u64(), Some(seed));
            let numeric: Recipe = serde_json::from_value(native.clone()).unwrap();
            let mut browser = native.clone();
            browser["seed"] = seed.to_string().into();
            let text: Recipe = serde_json::from_value(browser).unwrap();
            assert_eq!(numeric, text);
            assert_eq!(text, generated.recipe);
            assert_eq!(serde_json::to_value(&text).unwrap(), native);
            assert_eq!(text.schema_version, SCHEMA_VERSION);
            assert_eq!(replay(&text).unwrap().id, generated.id);
            assert_eq!(replay(&text).unwrap().problem, generated.problem);
        }
        let template =
            serde_json::to_value(generate(1, Some("arithmetic"), 0).unwrap().recipe).unwrap();
        for seed in [
            serde_json::json!(-1),
            serde_json::json!(1.0),
            serde_json::json!(1.5),
            serde_json::json!("-1"),
            serde_json::json!("1.0"),
            serde_json::json!(""),
            serde_json::json!(" 1"),
            serde_json::json!("18446744073709551616"),
        ] {
            let mut invalid = template.clone();
            invalid["seed"] = seed;
            assert!(serde_json::from_value::<Recipe>(invalid).is_err());
        }
        let overflow = serde_json::to_string(&template)
            .unwrap()
            .replace("\"seed\":0", "\"seed\":18446744073709551616");
        assert!(serde_json::from_str::<Recipe>(&overflow).is_err());
    }
    #[test]
    fn forward_classification_does_not_read_general_term() {
        let mut generated = generate(5, Some("weighted_sum"), 42).unwrap();
        generated.problem.general_term.rhs = Expr::integer(999);
        assert_eq!(classify(&generated.problem).unwrap().level, 5);
    }
    #[test]
    fn one_parameter_family_exceeds_recent_history_window() {
        let problems = (0..1024)
            .map(|seed| generate_limited(2, Some("scaled_constant"), seed, 1).unwrap())
            .collect::<Vec<_>>();
        let unique = problems.iter().map(|p| &p.id).collect::<BTreeSet<_>>();
        assert_eq!(unique.len(), 43);
        assert!(problems
            .iter()
            .any(|p| p.recipe.parameters["c"].integer_u32().is_none()));
    }
    #[test]
    fn equivalent_root_orders_share_content_identity() {
        for family in ["second_order", "weighted_sum"] {
            let mut original = parameters(&[
                ("c", Rational::integer(1)),
                ("r", Rational::integer(2)),
                ("s", Rational::integer(3)),
            ]);
            if family == "second_order" {
                original.insert("d".into(), Rational::integer(2));
            }
            let first = replay(&build_recipe(family, 10, original.clone()).unwrap()).unwrap();
            let mut swapped = original.clone();
            swapped.insert("r".into(), original["s"].clone());
            swapped.insert("s".into(), original["r"].clone());
            if family == "second_order" {
                swapped.insert("c".into(), original["d"].clone());
                swapped.insert("d".into(), original["c"].clone());
            }
            let second = replay(&build_recipe(family, 20, swapped).unwrap()).unwrap();
            assert_ne!(
                first.problem.general_term, second.problem.general_term,
                "raw pedagogical ordering is retained"
            );
            assert_eq!(first.id, second.id, "{family} equivalent roots");
            original.insert("c".into(), Rational::integer(2));
            let changed = replay(&build_recipe(family, 10, original).unwrap()).unwrap();
            assert_ne!(first.id, changed.id, "{family} different amplitude");
        }
    }
    #[test]
    fn displayed_index_factors_preserve_arithmetic_and_sum_formulas() {
        let arithmetic = replay(
            &build_recipe(
                "arithmetic",
                0,
                parameters(&[("c", Rational::integer(1)), ("d", Rational::integer(2))]),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            arithmetic.problem.general_term.latex(),
            "a_{n}=2\\,\\left(n-1\\right)+1"
        );
        let weighted = replay(
            &build_recipe(
                "weighted_sum",
                0,
                parameters(&[
                    ("c", Rational::integer(1)),
                    ("r", Rational::integer(2)),
                    ("s", Rational::integer(3)),
                ]),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(weighted
            .problem
            .recurrence
            .latex()
            .starts_with("a_{n+1}=\\left(n+1\\right)\\,\\left("));
        assert!(weighted
            .statement_latex
            .contains("\\left(n+1\\right)\\,\\left("));
    }
}
