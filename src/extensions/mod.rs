use crate::generator::{Classification, DerivationIR, FamilyInfo, ProblemIR, Recipe};
use crate::math::Rational;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
pub mod first_order;
pub mod powers;
pub mod second_order;
pub mod sums;
pub mod systems;

macro_rules! route {
    ($family:expr, $call:ident $(, $arg:expr)*) => {
        if first_order::supports($family) { first_order::$call($family $(, $arg)*) }
        else if second_order::supports($family) { second_order::$call($family $(, $arg)*) }
        else if sums::supports($family) { sums::$call($family $(, $arg)*) }
        else if powers::supports($family) { powers::$call($family $(, $arg)*) }
        else if systems::supports($family) { systems::$call($family $(, $arg)*) }
        else { Err("未登録の拡張型です。".into()) }
    }
}
pub fn catalog() -> Vec<FamilyInfo> {
    [
        first_order::catalog(),
        second_order::catalog(),
        sums::catalog(),
        powers::catalog(),
        systems::catalog(),
    ]
    .concat()
}
pub fn supports(family: &str) -> bool {
    first_order::supports(family)
        || second_order::supports(family)
        || sums::supports(family)
        || powers::supports(family)
        || systems::supports(family)
}
pub fn sample(
    family: &str,
    level: u8,
    rng: &mut ChaCha8Rng,
) -> Result<BTreeMap<String, Rational>, String> {
    route!(family, sample, level, rng)
}
pub fn validate_parameters(family: &str, p: &BTreeMap<String, Rational>) -> Result<(), String> {
    route!(family, validate_parameters, p)
}
pub fn build_recipe(
    family: &str,
    seed: u64,
    p: BTreeMap<String, Rational>,
) -> Result<Recipe, String> {
    route!(family, build_recipe, seed, p)
}
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String> {
    let family = recipe.family.as_str();
    if first_order::supports(family) {
        first_order::compile(recipe)
    } else if second_order::supports(family) {
        second_order::compile(recipe)
    } else if sums::supports(family) {
        sums::compile(recipe)
    } else if powers::supports(family) {
        powers::compile(recipe)
    } else if systems::supports(family) {
        systems::compile(recipe)
    } else {
        Err("未登録の拡張型です。".into())
    }
}
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String> {
    for classifier in [
        systems::classify,
        sums::classify,
        powers::classify,
        second_order::classify,
        first_order::classify,
    ] {
        if let Some(result) = classifier(problem)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}
pub fn explain(
    problem: &ProblemIR,
    classification: &Classification,
) -> Result<Option<DerivationIR>, String> {
    for explain in [
        systems::explain,
        sums::explain,
        powers::explain,
        second_order::explain,
        first_order::explain,
    ] {
        if let Some(result) = explain(problem, classification)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}
