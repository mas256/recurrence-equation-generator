//! Curriculum map: supported subsets and explicitly unimplemented branches.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageEntry {
    pub id: String,
    pub label: String,
    pub pattern: String,
    pub status: String,
    pub detail: String,
    pub families: Vec<String>,
}
pub fn catalog() -> Vec<CoverageEntry> {
    let mut rows = Vec::new();
    let mut add =
        |id: &str, label: &str, pattern: &str, status: &str, detail: &str, families: &[&str]| {
            rows.push(CoverageEntry {
                id: id.into(),
                label: label.into(),
                pattern: pattern.into(),
                status: status.into(),
                detail: detail.into(),
                families: families.iter().map(|s| s.to_string()).collect(),
            })
        };
    for (id, label, pattern, families) in [
        (
            "first_order_constant",
            "初項から一定の定数数列",
            "基本：特性方程式",
            vec!["cases_constant", "cases_affine_fixed"],
        ),
        (
            "first_order_p_zero",
            "倍率0：初項だけ異なる定数尾部",
            "基本：特性方程式",
            vec!["cases_reset"],
        ),
        (
            "first_order_p_one",
            "倍率1：等差数列",
            "基本：特性方程式",
            vec!["arithmetic", "cases_arithmetic"],
        ),
        (
            "first_order_signed_ratio",
            "正・負・分数の倍率",
            "基本：特性方程式",
            vec!["geometric", "affine", "cases_geometric", "cases_affine"],
        ),
        (
            "difference_polynomial",
            "登録した多項式階差・一定の二階差",
            "基本：階差",
            vec!["polynomial_difference", "arithmetic_difference"],
        ),
        (
            "ratio_registered",
            "登録倍率・階乗・指数係数",
            "基本：階比",
            vec![
                "scaled_constant",
                "factorial_ratio",
                "shifted_scaled",
                "scaled_affine",
                "ratio_power",
            ],
        ),
        (
            "polynomial_forcing_linear",
            "一次式の付加項と特解",
            "応用：特解",
            vec!["polynomial_forcing", "reciprocal_forcing", "coupled_forced"],
        ),
        (
            "exponential_forcing",
            "指数付加項：共鳴・非共鳴・倍率0・1",
            "応用：指数付加項",
            vec!["cases_exponential_forcing"],
        ),
        (
            "logarithmic_direct",
            "直接・平行移動後の対数型",
            "応用：対数",
            vec!["cases_logarithmic", "cases_shifted_logarithmic"],
        ),
        (
            "logarithmic_second",
            "対数をとって二階差を求める",
            "応用：対数",
            vec!["multiplicative_second"],
        ),
        (
            "reciprocal_registered",
            "逆数＋等差・階比・付加項",
            "応用：逆数",
            vec![
                "reciprocal_affine",
                "reciprocal_scaled",
                "reciprocal_forcing",
            ],
        ),
        (
            "fractional_two_fixed_points",
            "異なる二つの有理不動点",
            "応用：分数",
            vec!["mobius"],
        ),
        (
            "fractional_double_fixed_point",
            "重不動点・逆数が等差数列",
            "応用：分数",
            vec!["reciprocal_affine"],
        ),
        (
            "second_order_distinct",
            "正・負・分数の異なる有理2根",
            "応用：3項間",
            vec!["second_order", "cases_second_distinct"],
        ),
        (
            "second_order_repeated",
            "同次の重根1・1以外",
            "応用：3項間",
            vec!["cases_second_repeated"],
        ),
        (
            "second_order_zero_root",
            "零根：次数低下と初期項の例外",
            "応用：3項間",
            vec!["cases_second_zero"],
        ),
        (
            "second_order_single_mode",
            "初期条件で一つの成分が消える",
            "応用：3項間",
            vec!["cases_second_single_mode"],
        ),
        (
            "second_order_forced",
            "定数項と異根・重根の分岐",
            "応用：3項間",
            vec![
                "forced_second",
                "arithmetic_difference",
                "cases_second_forced",
            ],
        ),
        (
            "sums_registered",
            "部分和・重み付き総和と正規化",
            "応用：総和",
            vec![
                "weighted_sum",
                "pure_sum",
                "pure_sum_scaled",
                "sum_relation",
            ],
        ),
        (
            "systems_distinct",
            "異なる2実固有値の連立と付加項",
            "応用：連立",
            vec![
                "coupled_symmetric",
                "coupled_weighted",
                "coupled_scaled",
                "coupled_forced",
            ],
        ),
    ] {
        add(
            id,
            label,
            pattern,
            "supported",
            "登録された係数・初期値・倍率の範囲で生成します。任意の係数の全組合せを意味しません。",
            &families,
        );
    }
    for (id, label, pattern, families) in [
        (
            "first_order",
            "定数係数の1次漸化式",
            "基本：特性方程式",
            vec![
                "arithmetic",
                "geometric",
                "affine",
                "cases_constant",
                "cases_reset",
            ],
        ),
        (
            "difference",
            "階差数列型",
            "基本：階差",
            vec!["polynomial_difference", "difference_scaled"],
        ),
        (
            "ratio",
            "階比・変数係数型",
            "基本：階比",
            vec!["shifted_scaled", "scaled_affine"],
        ),
        (
            "polynomial_forcing",
            "多項式付加項型",
            "応用：特解",
            vec!["polynomial_forcing", "forced_second"],
        ),
        (
            "logarithmic",
            "対数を取る型",
            "応用：対数",
            vec!["cases_logarithmic", "multiplicative_second"],
        ),
        (
            "fractional",
            "一次分数型",
            "応用：分数",
            vec!["mobius", "reciprocal_affine"],
        ),
        (
            "second_order",
            "定数係数3項間の全分類",
            "応用：3項間",
            vec!["cases_second_repeated", "cases_second_distinct"],
        ),
        (
            "scaled_second_order",
            "変数係数の3項間・正規化",
            "応用：変数係数",
            vec!["scaled_second_order", "difference_scaled"],
        ),
        (
            "sums",
            "和を含む漸化式",
            "応用：総和",
            vec!["pure_sum", "sum_relation"],
        ),
        (
            "systems",
            "連立漸化式の全分類",
            "応用：連立",
            vec!["coupled_symmetric", "coupled_weighted"],
        ),
        (
            "initial_condition_branches",
            "初期条件による退化・解法の変化",
            "横断：初期条件",
            vec!["cases_constant", "cases_reset", "cases_second_single_mode"],
        ),
        (
            "periodic",
            "周期数列・符号交代",
            "発展：周期",
            vec!["cases_geometric", "cases_affine"],
        ),
        (
            "symmetric_expressions",
            "対称式による分離",
            "発展：対称式",
            vec!["coupled_symmetric"],
        ),
    ] {
        add(
            id,
            label,
            pattern,
            "partial",
            "登録形の一部に対応しています。未実装の細分岐は別行で表示します。",
            &families,
        );
    }
    for (id, label, pattern) in [
        (
            "first_order_real_coefficients",
            "任意の実係数・実数値",
            "基本：特性方程式",
        ),
        (
            "difference_general",
            "任意の可和な階差・高次多項式",
            "基本：階差",
        ),
        (
            "ratio_zero_coefficient",
            "変数係数が途中で0になる場合",
            "基本：階比",
        ),
        (
            "polynomial_forcing_general",
            "任意次数の付加項・共鳴次数",
            "応用：特解",
        ),
        (
            "logarithmic_sign_cases",
            "負値・零・任意の実数冪を含む対数型",
            "応用：対数",
        ),
        (
            "reciprocal_zero_solution",
            "逆数置換から除外される零解",
            "応用：逆数",
        ),
        (
            "fractional_irrational_fixed_points",
            "無理数の実不動点",
            "応用：分数",
        ),
        (
            "fractional_no_real_fixed_point",
            "実不動点を持たない分数型",
            "応用：分数",
        ),
        (
            "fractional_degenerate",
            "定数写像・不動点の初項・特異点",
            "応用：分数",
        ),
        (
            "second_order_irrational_roots",
            "異なる無理数の実特性根",
            "応用：3項間",
        ),
        (
            "second_order_nonreal_roots",
            "非実共役特性根",
            "応用：3項間",
        ),
        (
            "sums_special_coefficients",
            "総和型の特別係数0・1と次数低下",
            "応用：総和",
        ),
        (
            "systems_repeated",
            "重複固有値・Jordan型の連立",
            "応用：連立",
        ),
        ("systems_nonreal", "非実固有値・回転型の連立", "応用：連立"),
        (
            "periodic_general",
            "一般の周期写像・周期3以上",
            "発展：周期",
        ),
        (
            "symmetric_nonlinear",
            "和と積による非線形対称式",
            "発展：対称式",
        ),
        ("floor", "ガウス記号を含む型", "発展：ガウス記号"),
        ("higher_order", "4項間以上", "発展：高階"),
        ("nonadjacent", "非隣接・添字が飛ぶ型", "発展：非隣接"),
        ("trigonometric", "三角関数による線形化", "発展：三角関数"),
        ("newton", "ニュートン法型", "発展：ニュートン法"),
        ("invariants", "非線形の保存量を発見する型", "発展：保存量"),
        ("double_recurrences", "二重数列・二重漸化式", "発展：二重"),
        (
            "generating_functions",
            "母関数による独立した解法",
            "発展：母関数",
        ),
        (
            "free_composition",
            "任意の可逆変換・解法の自由な合成",
            "横断：合成",
        ),
    ] {
        add(
            id,
            label,
            pattern,
            "planned",
            "専用の生成条件・値表現・導出・検証は未実装です。",
            &[],
        );
    }
    rows
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    #[test]
    fn scope_matches_registered_generators() {
        let families: BTreeSet<_> = crate::generator::family_catalog()
            .into_iter()
            .map(|f| f.id)
            .collect();
        let mut ids = BTreeSet::new();
        for e in catalog() {
            assert!(ids.insert(e.id));
            assert!(["supported", "partial", "planned"].contains(&e.status.as_str()));
            assert_eq!(e.status == "planned", e.families.is_empty());
            for f in e.families {
                assert!(families.contains(&f), "{f}");
            }
        }
    }
}
