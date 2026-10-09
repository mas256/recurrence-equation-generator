//! Exact, typed mathematics. Public indices start at 1; Lean's index starts at 0.
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{fmt, str::FromStr};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rational(pub BigRational);

impl Rational {
    pub fn integer(value: i64) -> Self {
        Self(BigRational::from_integer(value.into()))
    }
    pub fn fraction(num: i64, den: i64) -> Self {
        assert_ne!(den, 0);
        Self(BigRational::new(num.into(), den.into()))
    }
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
    pub fn is_one(&self) -> bool {
        self.0.is_one()
    }
    pub fn is_positive(&self) -> bool {
        self.0.is_positive()
    }
    pub fn add(&self, other: &Self) -> Self {
        Self(&self.0 + &other.0)
    }
    pub fn sub(&self, other: &Self) -> Self {
        Self(&self.0 - &other.0)
    }
    pub fn mul(&self, other: &Self) -> Self {
        Self(&self.0 * &other.0)
    }
    pub fn div(&self, other: &Self) -> Result<Self, String> {
        if other.is_zero() {
            Err("零による除算は定義できません。".into())
        } else {
            Ok(Self(&self.0 / &other.0))
        }
    }
    pub fn pow(&self, n: u32) -> Self {
        Self(self.0.pow(n as i32))
    }
    pub fn integer_u32(&self) -> Option<u32> {
        if self.0.is_integer() {
            self.0.to_integer().to_u32()
        } else {
            None
        }
    }
    pub fn latex(&self) -> String {
        if self.0.denom().is_one() {
            self.0.numer().to_string()
        } else if self.0.is_negative() {
            format!("-\\frac{{{}}}{{{}}}", self.0.numer().abs(), self.0.denom())
        } else {
            format!("\\frac{{{}}}{{{}}}", self.0.numer(), self.0.denom())
        }
    }
    pub fn lean(&self) -> String {
        format!("(({} : ℚ) / {})", self.0.numer(), self.0.denom())
    }
}
impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl Serialize for Rational {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Parts {
            num: String,
            den: String,
        }
        Parts {
            num: self.0.numer().to_string(),
            den: self.0.denom().to_string(),
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Rational {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Parts {
            num: String,
            den: String,
        }
        let p = Parts::deserialize(deserializer)?;
        let num = BigInt::from_str(&p.num).map_err(serde::de::Error::custom)?;
        let den = BigInt::from_str(&p.den).map_err(serde::de::Error::custom)?;
        if den.is_zero() {
            return Err(serde::de::Error::custom(
                "rational denominator must be nonzero",
            ));
        }
        Ok(Self(BigRational::new(num, den)))
    }
}

/// Natural expressions are kept separate from rational expressions and exponents.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum NatExpr {
    Index,
    Constant {
        value: u32,
    },
    Add {
        left: Box<NatExpr>,
        right: Box<NatExpr>,
    },
    Sub {
        left: Box<NatExpr>,
        right: Box<NatExpr>,
    },
    Mul {
        left: Box<NatExpr>,
        right: Box<NatExpr>,
    },
    Div {
        numerator: Box<NatExpr>,
        denominator: Box<NatExpr>,
    },
    Pow {
        base: Box<NatExpr>,
        exponent: u32,
    },
    Choose {
        n: Box<NatExpr>,
        k: u32,
    },
    Bound {
        name: String,
    },
}
impl NatExpr {
    pub fn degree(&self) -> u8 {
        match self {
            Self::Index | Self::Bound { .. } => 1,
            Self::Constant { .. } => 0,
            Self::Add { left, right } | Self::Sub { left, right } => {
                left.degree().max(right.degree())
            }
            Self::Mul { left, right } => left.degree().saturating_add(right.degree()),
            Self::Div {
                numerator,
                denominator,
            } => numerator.degree().saturating_sub(denominator.degree()),
            Self::Pow { base, exponent } => {
                base.degree().saturating_mul((*exponent).min(255) as u8)
            }
            Self::Choose { n, k } => n.degree().saturating_mul((*k).min(255) as u8),
        }
    }
    pub fn choose(n: NatExpr, k: u32) -> Self {
        Self::Choose { n: Box::new(n), k }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn mul(left: NatExpr, right: NatExpr) -> Self {
        Self::Mul {
            left: Box::new(left),
            right: Box::new(right),
        }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn add(left: NatExpr, right: NatExpr) -> Self {
        Self::Add {
            left: Box::new(left),
            right: Box::new(right),
        }
    }
    pub fn n() -> Self {
        Self::Index
    }
    pub fn constant(value: u32) -> Self {
        Self::Constant { value }
    }
    pub fn offset(offset: i32) -> Self {
        if offset == 0 {
            Self::Index
        } else if offset > 0 {
            Self::Add {
                left: Box::new(Self::Index),
                right: Box::new(Self::constant(offset as u32)),
            }
        } else {
            Self::Sub {
                left: Box::new(Self::Index),
                right: Box::new(Self::constant(offset.unsigned_abs())),
            }
        }
    }
    pub fn evaluate(&self, n: u32, bound: Option<(&str, u32)>) -> Result<u32, String> {
        Ok(match self {
            Self::Index => n,
            Self::Constant { value } => *value,
            Self::Add { left, right } => left
                .evaluate(n, bound)?
                .checked_add(right.evaluate(n, bound)?)
                .ok_or("添字が大きすぎます。")?,
            Self::Sub { left, right } => left
                .evaluate(n, bound)?
                .saturating_sub(right.evaluate(n, bound)?),
            Self::Mul { left, right } => left
                .evaluate(n, bound)?
                .checked_mul(right.evaluate(n, bound)?)
                .ok_or("添字が大きすぎます。")?,
            Self::Div {
                numerator,
                denominator,
            } => {
                let den = denominator.evaluate(n, bound)?;
                if den == 0 {
                    return Err("自然数の除数が零です。".into());
                }
                numerator.evaluate(n, bound)? / den
            }
            Self::Pow { base, exponent } => base
                .evaluate(n, bound)?
                .checked_pow(*exponent)
                .ok_or("添字が大きすぎます。")?,
            Self::Choose { n: value, k } => {
                let m = value.evaluate(n, bound)?;
                if *k > m {
                    0
                } else {
                    let k = (*k).min(m - k);
                    let mut result = 1u64;
                    for i in 1..=k {
                        result = result
                            .checked_mul(u64::from(m - k + i))
                            .ok_or("二項係数が大きすぎます。")?
                            / u64::from(i);
                    }
                    u32::try_from(result).map_err(|_| "二項係数が大きすぎます。")?
                }
            }
            Self::Bound { name } => {
                bound
                    .filter(|(key, _)| *key == name)
                    .ok_or("束縛変数がありません。")?
                    .1
            }
        })
    }
    pub fn latex(&self) -> String {
        match self {
            Self::Index => "n".into(),
            Self::Constant { value } => value.to_string(),
            Self::Bound { name } => name.clone(),
            Self::Add { left, right } => format!("{}+{}", left.latex(), right.latex()),
            Self::Sub { left, right } => format!("{}-{}", left.latex(), right.latex()),
            Self::Mul { left, right } => format!(
                "\\left({}\\right)\\left({}\\right)",
                left.latex(),
                right.latex()
            ),
            Self::Div {
                numerator,
                denominator,
            } => format!(
                "\\left\\lfloor\\frac{{{}}}{{{}}}\\right\\rfloor",
                numerator.latex(),
                denominator.latex()
            ),
            Self::Pow { base, exponent } => {
                format!("\\left({}\\right)^{{{exponent}}}", base.latex())
            }
            Self::Choose { n, k } => format!("\\binom{{{}}}{{{k}}}", n.latex()),
        }
    }
    pub fn lean(&self) -> String {
        match self {
            Self::Index => "(n + 1)".into(),
            Self::Constant { value } => value.to_string(),
            Self::Bound { name } => name.clone(),
            // Exact simplification avoids natural subtraction in the common n-1 exponent.
            Self::Sub { left, right } if **left == Self::Index && **right == Self::constant(1) => {
                "n".into()
            }
            Self::Add { left, right } => format!("({} + {})", left.lean(), right.lean()),
            Self::Sub { left, right } => format!("({} - {})", left.lean(), right.lean()),
            Self::Mul { left, right } => format!("({} * {})", left.lean(), right.lean()),
            Self::Div {
                numerator,
                denominator,
            } => format!("({} / {})", numerator.lean(), denominator.lean()),
            Self::Pow { base, exponent } => format!("({} ^ {exponent})", base.lean()),
            Self::Choose { n, k } => format!("Nat.choose {} {k}", n.lean()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Expr {
    Rational {
        value: Rational,
    },
    NatCast {
        value: NatExpr,
    },
    Term {
        sequence: String,
        index: NatExpr,
    },
    Add {
        terms: Vec<Expr>,
    },
    Mul {
        factors: Vec<Expr>,
    },
    Div {
        numerator: Box<Expr>,
        denominator: Box<Expr>,
    },
    Pow {
        base: Box<Expr>,
        exponent: NatExpr,
    },
    Factorial {
        index: NatExpr,
    },
    Log {
        base: Box<Expr>,
        argument: Box<Expr>,
    },
    Sum {
        variable: String,
        lower: NatExpr,
        upper: NatExpr,
        body: Box<Expr>,
    },
}
impl Expr {
    pub fn integer(n: i64) -> Self {
        Self::rational(Rational::integer(n))
    }
    pub fn rational(value: Rational) -> Self {
        Self::Rational { value }
    }
    pub fn index() -> Self {
        Self::NatCast {
            value: NatExpr::n(),
        }
    }
    pub fn offset(n: i32) -> Self {
        Self::NatCast {
            value: NatExpr::offset(n),
        }
    }
    pub fn term(sequence: &str, offset: i32) -> Self {
        Self::Term {
            sequence: sequence.into(),
            index: NatExpr::offset(offset),
        }
    }
    pub fn add(items: Vec<Self>) -> Self {
        fn collect(item: Expr, terms: &mut Vec<Expr>, constant: &mut Rational) {
            match item {
                Expr::Add { terms: inner } => {
                    for term in inner {
                        collect(term, terms, constant);
                    }
                }
                Expr::Rational { value } => *constant = constant.add(&value),
                x => terms.push(x),
            }
        }
        let mut terms = Vec::new();
        let mut constant = Rational::integer(0);
        for item in items {
            collect(item, &mut terms, &mut constant);
        }
        if !constant.is_zero() {
            terms.push(Self::rational(constant));
        }
        match terms.len() {
            0 => Self::integer(0),
            1 => terms.remove(0),
            _ => Self::Add { terms },
        }
    }
    pub fn mul(items: Vec<Self>) -> Self {
        fn collect(item: Expr, factors: &mut Vec<Expr>, constant: &mut Rational) {
            match item {
                Expr::Mul { factors: inner } => {
                    for factor in inner {
                        collect(factor, factors, constant);
                    }
                }
                Expr::Rational { value } => *constant = constant.mul(&value),
                x => factors.push(x),
            }
        }
        let mut factors = Vec::new();
        let mut constant = Rational::integer(1);
        for item in items {
            collect(item, &mut factors, &mut constant);
        }
        if constant.is_zero() {
            return Self::integer(0);
        }
        if !constant.is_one() {
            factors.insert(0, Self::rational(constant));
        }
        match factors.len() {
            0 => Self::integer(1),
            1 => factors.remove(0),
            _ => Self::Mul { factors },
        }
    }
    #[allow(clippy::should_implement_trait)] // These constructors normalize symbolic ASTs.
    pub fn sub(lhs: Self, rhs: Self) -> Self {
        Self::add(vec![lhs, Self::mul(vec![Self::integer(-1), rhs])])
    }
    #[allow(clippy::should_implement_trait)] // Division retains the denominator in the AST.
    pub fn div(numerator: Self, denominator: Self) -> Self {
        if let (Self::Rational { value: a }, Self::Rational { value: b }) =
            (&numerator, &denominator)
        {
            if let Ok(c) = a.div(b) {
                return Self::rational(c);
            }
        }
        Self::Div {
            numerator: Box::new(numerator),
            denominator: Box::new(denominator),
        }
    }
    pub fn pow(base: Self, exponent: NatExpr) -> Self {
        Self::Pow {
            base: Box::new(base),
            exponent,
        }
    }
    pub fn evaluate(
        &self,
        n: u32,
        term: &dyn Fn(&str, u32) -> Result<Rational, String>,
    ) -> Result<Rational, String> {
        self.evaluate_bound(n, term, None)
    }
    fn evaluate_bound(
        &self,
        n: u32,
        term: &dyn Fn(&str, u32) -> Result<Rational, String>,
        bound: Option<(&str, u32)>,
    ) -> Result<Rational, String> {
        match self {
            Self::Rational { value } => Ok(value.clone()),
            Self::NatCast { value } => Ok(Rational::integer(value.evaluate(n, bound)? as i64)),
            Self::Term { sequence, index } => term(sequence, index.evaluate(n, bound)?),
            Self::Add { terms } => terms.iter().try_fold(Rational::integer(0), |acc, x| {
                Ok(acc.add(&x.evaluate_bound(n, term, bound)?))
            }),
            Self::Mul { factors } => factors.iter().try_fold(Rational::integer(1), |acc, x| {
                Ok(acc.mul(&x.evaluate_bound(n, term, bound)?))
            }),
            Self::Div {
                numerator,
                denominator,
            } => numerator
                .evaluate_bound(n, term, bound)?
                .div(&denominator.evaluate_bound(n, term, bound)?),
            Self::Pow { base, exponent } => Ok(base
                .evaluate_bound(n, term, bound)?
                .pow(exponent.evaluate(n, bound)?)),
            Self::Factorial { index } => {
                let k = index.evaluate(n, bound)?;
                let value = (1..=k).fold(BigInt::one(), |acc, v| acc * v);
                Ok(Rational(BigRational::from_integer(value)))
            }
            Self::Log { .. } => {
                Err("対数は補助数列の説明用です。有理数の式としては評価できません。".into())
            }
            Self::Sum {
                variable,
                lower,
                upper,
                body,
            } => {
                let lo = lower.evaluate(n, bound)?;
                let hi = upper.evaluate(n, bound)?;
                (lo..=hi).try_fold(Rational::integer(0), |acc, k| {
                    Ok(acc.add(&body.evaluate_bound(n, term, Some((variable, k)))?))
                })
            }
        }
    }
    pub fn latex(&self) -> String {
        match self {
            Self::Rational { value } => value.latex(),
            Self::NatCast { value } => value.latex(),
            Self::Term { sequence, index } => format!("{}_{{{}}}", sequence, index.latex()),
            Self::Add { terms } => {
                let mut s = String::new();
                for (i, x) in terms.iter().enumerate() {
                    let value = x.latex();
                    if i > 0 && !value.starts_with('-') {
                        s.push('+');
                    }
                    s.push_str(&value);
                }
                s
            }
            Self::Mul { factors } => factors
                .iter()
                .map(|x| {
                    let s = x.latex();
                    if matches!(
                        x,
                        Self::Add { .. }
                            | Self::NatCast {
                                value: NatExpr::Add { .. } | NatExpr::Sub { .. }
                            }
                    ) || s.starts_with('-')
                    {
                        format!("\\left({s}\\right)")
                    } else {
                        s
                    }
                })
                .collect::<Vec<_>>()
                .join("\\,"),
            Self::Div {
                numerator,
                denominator,
            } => format!("\\frac{{{}}}{{{}}}", numerator.latex(), denominator.latex()),
            Self::Pow { base, exponent } => {
                let s = base.latex();
                let b = if matches!(**base,Self::Rational{ref value} if value.0.denom().is_one()&&value.is_positive())
                {
                    s
                } else {
                    format!("\\left({s}\\right)")
                };
                format!("{b}^{{{}}}", exponent.latex())
            }
            Self::Factorial { index } => format!("\\left({}\\right)!", index.latex()),
            Self::Log { base, argument } => format!(
                "\\log_{{{}}}\\left({}\\right)",
                base.latex(),
                argument.latex()
            ),
            Self::Sum {
                variable,
                lower,
                upper,
                body,
            } => {
                let summand = if matches!(**body, Self::Add { .. }) {
                    format!("\\left({}\\right)", body.latex())
                } else {
                    body.latex()
                };
                format!(
                    "\\sum_{{{variable}={}}}^{{{}}}{summand}",
                    lower.latex(),
                    upper.latex()
                )
            }
        }
    }
    /// Rendering uses the same mathematical tree as the screen and export.
    pub fn lean(&self) -> String {
        match self {
            Self::Rational { value } => value.lean(),
            Self::NatCast { value } => format!("({} : ℚ)", value.lean()),
            Self::Term { sequence, index } => format!("({sequence} ({} - 1))", index.lean()),
            Self::Add { terms } => format!(
                "({})",
                terms.iter().map(Self::lean).collect::<Vec<_>>().join(" + ")
            ),
            Self::Mul { factors } => format!(
                "({})",
                factors
                    .iter()
                    .map(Self::lean)
                    .collect::<Vec<_>>()
                    .join(" * ")
            ),
            Self::Div {
                numerator,
                denominator,
            } => format!("({} / {})", numerator.lean(), denominator.lean()),
            Self::Pow { base, exponent } => format!("({} ^ {})", base.lean(), exponent.lean()),
            Self::Factorial { index } => format!("(Nat.factorial {} : ℚ)", index.lean()),
            Self::Log { .. } => "-- logarithm is a pedagogical real-valued auxiliary".into(),
            Self::Sum {
                variable,
                lower,
                upper,
                body,
            } => format!(
                "(∑ {variable} ∈ Finset.Icc {} {}, {})",
                lower.lean(),
                upper.lean(),
                body.lean()
            ),
        }
    }
    /// A deterministic representation for content identity. Display/proof
    /// renderers retain the original pedagogical ordering of operands.
    pub fn canonical(&self) -> Self {
        fn sorted(expressions: &[Expr]) -> Vec<Expr> {
            let mut result = expressions.iter().map(Expr::canonical).collect::<Vec<_>>();
            result.sort_by_cached_key(|expr| {
                // Every Expr field is serializable without fallible map keys.
                serde_json::to_string(expr).expect("typed expression serialization")
            });
            result
        }
        match self {
            Self::Add { terms } => Self::Add {
                terms: sorted(terms),
            },
            Self::Mul { factors } => Self::Mul {
                factors: sorted(factors),
            },
            Self::Div {
                numerator,
                denominator,
            } => Self::Div {
                numerator: Box::new(numerator.canonical()),
                denominator: Box::new(denominator.canonical()),
            },
            Self::Pow { base, exponent } => Self::Pow {
                base: Box::new(base.canonical()),
                exponent: exponent.clone(),
            },
            Self::Log { base, argument } => Self::Log {
                base: Box::new(base.canonical()),
                argument: Box::new(argument.canonical()),
            },
            Self::Sum {
                variable,
                lower,
                upper,
                body,
            } => Self::Sum {
                variable: variable.clone(),
                lower: lower.clone(),
                upper: upper.clone(),
                body: Box::new(body.canonical()),
            },
            other => other.clone(),
        }
    }
    pub fn nodes(&self) -> usize {
        1 + match self {
            Self::Add { terms } => terms.iter().map(Self::nodes).sum(),
            Self::Mul { factors } => factors.iter().map(Self::nodes).sum(),
            Self::Div {
                numerator,
                denominator,
            } => numerator.nodes() + denominator.nodes(),
            Self::Pow { base, .. } => base.nodes(),
            Self::Log { base, argument } => base.nodes() + argument.nodes(),
            Self::Sum { body, .. } => body.nodes(),
            _ => 0,
        }
    }
    pub fn exponent_degree(&self) -> u8 {
        match self {
            Self::Pow { base, exponent } => base.exponent_degree().max(exponent.degree()),
            Self::Add { terms } => terms.iter().map(Self::exponent_degree).max().unwrap_or(0),
            Self::Mul { factors } => factors.iter().map(Self::exponent_degree).max().unwrap_or(0),
            Self::Div {
                numerator,
                denominator,
            } => numerator
                .exponent_degree()
                .max(denominator.exponent_degree()),
            Self::Sum { body, .. } => body.exponent_degree(),
            Self::Log { base, argument } => base.exponent_degree().max(argument.exponent_degree()),
            _ => 0,
        }
    }
    pub fn fraction_depth(&self) -> usize {
        match self {
            Self::Rational { value } => usize::from(!value.0.denom().is_one()),
            Self::Div {
                numerator,
                denominator,
            } => 1 + numerator.fraction_depth().max(denominator.fraction_depth()),
            Self::Add { terms } => terms.iter().map(Self::fraction_depth).max().unwrap_or(0),
            Self::Mul { factors } => factors.iter().map(Self::fraction_depth).max().unwrap_or(0),
            Self::Pow { base, .. } => base.fraction_depth(),
            Self::Log { base, argument } => base.fraction_depth().max(argument.fraction_depth()),
            Self::Sum { body, .. } => body.fraction_depth(),
            _ => 0,
        }
    }
    pub fn add_terms(&self) -> usize {
        match self {
            Self::Add { terms } => {
                terms.len().saturating_sub(1) + terms.iter().map(Self::add_terms).sum::<usize>()
            }
            Self::Mul { factors } => factors.iter().map(Self::add_terms).sum(),
            Self::Div {
                numerator,
                denominator,
            } => numerator.add_terms() + denominator.add_terms(),
            Self::Pow { base, .. } => base.add_terms(),
            Self::Log { base, argument } => base.add_terms() + argument.add_terms(),
            Self::Sum { body, .. } => body.add_terms(),
            _ => 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Equation {
    pub lhs: Expr,
    pub rhs: Expr,
}
impl Equation {
    pub fn new(lhs: Expr, rhs: Expr) -> Self {
        Self { lhs, rhs }
    }
    pub fn latex(&self) -> String {
        format!("{}={}", self.lhs.latex(), self.rhs.latex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_large_rational_and_reduction() {
        let a: Rational =
            serde_json::from_str(r#"{"num":"123456789123456789123456789","den":"3"}"#).unwrap();
        assert_eq!(a.to_string(), "41152263041152263041152263");
        assert!(serde_json::from_str::<Rational>(r#"{"num":"1","den":"0"}"#).is_err());
    }
    #[test]
    fn rational_roundtrip() {
        let x = Rational::fraction(-8, 12);
        assert_eq!(
            serde_json::from_str::<Rational>(&serde_json::to_string(&x).unwrap()).unwrap(),
            x
        );
    }
    #[test]
    fn nested_constants_have_one_normalized_tree() {
        let x = Expr::index();
        assert_eq!(
            Expr::add(vec![
                Expr::add(vec![x.clone(), Expr::integer(2)]),
                Expr::integer(3)
            ]),
            Expr::add(vec![x.clone(), Expr::integer(5)])
        );
        assert_eq!(
            Expr::mul(vec![
                Expr::mul(vec![Expr::integer(2), x.clone()]),
                Expr::integer(3)
            ]),
            Expr::mul(vec![Expr::integer(6), x])
        );
    }
    #[test]
    fn additive_summand_preserves_displayed_scope() {
        let expr = Expr::Sum {
            variable: "k".into(),
            lower: NatExpr::constant(1),
            upper: NatExpr::n(),
            body: Box::new(Expr::add(vec![Expr::index(), Expr::integer(1)])),
        };
        assert!(expr.latex().ends_with("\\left(n+1\\right)"));
    }
}
