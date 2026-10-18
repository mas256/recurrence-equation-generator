use crate::generator::GeneratedProblem;
pub mod elementary;
pub mod exponential;
pub mod first_order;
pub mod powers;
pub mod second_order;
pub mod sums;
pub mod systems;
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String> {
    for source in [
        elementary::source,
        exponential::source,
        first_order::source,
        second_order::source,
        sums::source,
        powers::source,
        systems::source,
    ] {
        if let Some(result) = source(problem)? {
            return Ok(Some(result));
        }
    }
    Ok(None)
}
