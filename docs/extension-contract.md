# Extension contract (0.2)

Root owns the shared types and dispatchers. Each extension module owns its registered families and exports these functions:

```rust
pub fn catalog() -> Vec<FamilyInfo>;
pub fn supports(family: &str) -> bool;
pub fn sample(family: &str, level: u8, rng: &mut ChaCha8Rng)
    -> Result<BTreeMap<String, Rational>, String>;
pub fn validate_parameters(family: &str, p: &BTreeMap<String, Rational>) -> Result<(), String>;
pub fn build_recipe(family: &str, seed: u64, p: BTreeMap<String, Rational>) -> Result<Recipe, String>;
pub fn compile(recipe: &Recipe) -> Result<ProblemIR, String>;
pub fn classify(problem: &ProblemIR) -> Result<Option<Classification>, String>;
pub fn explain(problem: &ProblemIR, classification: &Classification)
    -> Result<Option<DerivationIR>, String>;
```

Classification must derive the route from the displayed equation and initials, without using Recipe or trusting the supplied general term. Return `Ok(None)` for unrelated shapes. Explain dispatch can use the unique method name produced by classification.

Each proof module exports:

```rust
pub fn source(problem: &GeneratedProblem) -> Result<Option<String>, String>;
```

Return a complete Lean module for owned families and None otherwise. Import `Recurrence.Theory`, print Lean.versionString and audit the four theorem names `GeneratedProblem.problem_valid`, `problem_recurrence`, `problem_unique`, `problem_conditions`. All recurrence statements must match the IR. Prove initials, recurrence for every natural index, uniqueness, and all declared domain conditions.

New types added by root:

- RecurrenceKind: FirstOrder, SecondOrder, WeightedSum, PureSum, SumRelation, System, MultiplicativeSecond.
- ProblemIR adds `secondary_recurrences: Vec<Equation>` and `secondary_general_terms: Vec<Equation>` (empty for scalar families). System initials contain both a and b. Primary equation is a; secondary is b.
- NatExpr adds `Mul { left, right }`, `Div { numerator, denominator }`, `Pow { base, exponent: u32 }`, `Choose { n: Box<NatExpr>, k: u32 }`.
- Expr adds `Log { base: Box<Expr>, argument: Box<Expr> }` for pedagogical derivations. It is not supported in rational recurrence verification.
- Existing RULES_VERSION remains 1.0; new Recipe constructors use EXTENSION_RULES_VERSION (`rust-registered-1.1/reference-0.7.0`). SCHEMA_VERSION and RNG_VERSION are unchanged.

Root exposes pub(crate) generator helpers: parameters, block, initial, nonzero, step, linear_split, contains_term, literal, has_sum, initial_value, collect_rationals, numeric_cost, and:

```rust
pub(crate) fn make_classification(problem: &ProblemIR, score: u8,
    method: &str, reason: &str, operations: &[&str], discovery: u8,
    domain: u8, coefficient_degree: u8) -> Result<Classification, String>;
```

This computes actual complexity and numeric budget, score parts and level, using reference operation weights. Module-specific signed and zero profile parameters are validated in the module. Recipes must contain only validated bounded coefficients and canonical blocks (at most four).

Root exposes `verifier::render(expr, sequence, bound, step)` and `verifier::natural(value, bound)` as pub(crate). Sequence="f" maps original a to f and b to g; sequence="a" maps a to a and b to b. In step mode current a maps to x and b to y. Sum lower1 and upper n, n+1 or n+2 is translated to Finset.range with the matching cardinality and public k=j+1.

Agents may use local profile helpers; root will not impose a shared scale encoding. State exact profile coverage and reference scores in the completion report. Representative accepted profiles for all 28 families are the immediate scope; do not advertise full coverage of all 110 reference profile entries unless verified.
