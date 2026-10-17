//! Textbook output is rendered from the exact same IR as the web view.
use crate::generator::{DerivationIR, GeneratedProblem};

fn prose(value: &str) -> String {
    let mut result = String::new();
    for c in value.chars() {
        result.push_str(match c {
            '\\' => "\\textbackslash{}",
            '&' => "\\&",
            '%' => "\\%",
            '$' => "\\$",
            '#' => "\\#",
            '_' => "\\_",
            '{' => "\\{",
            '}' => "\\}",
            '~' => "\\textasciitilde{}",
            '^' => "\\textasciicircum{}",
            '−' => "-",
            _ => {
                result.push(c);
                continue;
            }
        });
    }
    result
}
fn display(latex: &str) -> String {
    // Fit long equations to a column; short equations keep their natural size.
    format!("\\begin{{center}}\\fitmath{{{latex}}}\\end{{center}}\n")
}
fn write_derivation(output: &mut String, derivation: &DerivationIR) {
    for step in &derivation.steps {
        output.push_str(&prose(&step.text));
        output.push_str("\n\n");
        if let Some(equation) = &step.equation {
            output.push_str(&display(&equation.latex()));
        }
        for condition in &step.conditions {
            output.push_str(&prose(condition));
            output.push_str("\n\n");
        }
    }
}
pub fn tex(problem: &GeneratedProblem, include_solution: bool) -> String {
    let mut output = format!(
        r#"% UTF-8 / pLaTeX + dvipdfmx
% Problem content SHA-256: {}
% Rules: {}
% Solution routes use finite equation checks, not a Lean proof of every route.
% A native Lean certificate concerns the original problem.
\documentclass[a4paper,11pt,twocolumn]{{jsbook}}
\usepackage[dvipdfmx]{{graphicx}}
\usepackage{{amsmath,amssymb}}
\usepackage[margin=20mm]{{geometry}}
\setlength{{\columnsep}}{{12pt}}
\setlength{{\columnseprule}}{{0.4pt}}
\newsavebox{{\equationbox}}
\newcommand{{\fitmath}}[1]{{\sbox{{\equationbox}}{{$\displaystyle #1$}}\ifdim\wd\equationbox>\columnwidth\resizebox{{\columnwidth}}{{!}}{{\usebox{{\equationbox}}}}\else\usebox{{\equationbox}}\fi}}
\pagestyle{{plain}}
\begin{{document}}
\section*{{漸化式演習 Lv{}}}
次の条件を満たす数列の一般項を求めよ。数列の項は有理数とし、添字は $n\ge1$ とする。
"#,
        problem.id, problem.recipe.rules_version, problem.classification.level
    );
    output.push_str(&display(&problem.problem.recurrence.latex()));
    for equation in &problem.problem.secondary_recurrences {
        output.push_str(&display(&equation.latex()));
    }
    output.push_str(&display(
        &problem
            .problem
            .initials
            .iter()
            .map(|i| i.latex())
            .collect::<Vec<_>>()
            .join("\\qquad "),
    ));
    if include_solution {
        output.push_str("\\subsection*{方針}\n");
        output.push_str(&prose(&problem.hint));
        output.push('\n');
        output.push_str("\\subsection*{解答}\n");
        write_derivation(&mut output, &problem.derivation);
        output.push_str("\\subsection*{一般項}\n");
        output.push_str(&display(&problem.problem.general_term.latex()));
        for equation in &problem.problem.secondary_general_terms {
            output.push_str(&display(&equation.latex()));
        }
        for (i, route) in problem.routes.iter().enumerate() {
            output.push_str(&format!("% Route {}: checked terms {}, checked equations {}, unresolved {}, formal proof {}\n",i+1,route.validation.checked_terms,route.validation.equations_checked,route.validation.unresolved_equations,route.validation.formal_verification));
            if i == 0 {
                continue;
            }
            output.push_str(&format!(
                "\\subsection*{{別解{}：{}}}\n",
                i,
                prose(&route.method_name)
            ));
            output.push_str(&prose(&route.hint));
            output.push_str("\n\n");
            write_derivation(&mut output, &route.derivation);
        }
        if problem.routes.is_empty() {
            for alternative in &problem.derivation.alternatives {
                output.push_str("\\subsection*{別の考え方（概要）}\n");
                output.push_str(&prose(alternative));
                output.push('\n');
            }
        }
    }
    output.push_str("\\end{document}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_solution_export_does_not_reveal_answer() {
        let p = crate::generator::generate(3, Some("reciprocal_affine"), 32).unwrap();
        let question = tex(&p, false);
        let full = tex(&p, true);
        assert!(question.contains(&p.problem.recurrence.latex()));
        assert!(!question.contains(&p.problem.general_term.latex()));
        assert!(full.contains(&p.problem.general_term.latex()));
        assert!(full.contains("\\columnsep}{12pt}"));
    }
    #[test]
    fn export_includes_complete_alternative_equations() {
        let p = crate::generator::generate(2, Some("affine"), 32).unwrap();
        assert!(p.routes.len() >= 2);
        let full = tex(&p, true);
        let question = tex(&p, false);
        for route in p.routes.iter().skip(1) {
            assert!(full.contains(&prose(&route.method_name)));
            assert!(!question.contains(&prose(&route.method_name)));
            assert!(route
                .derivation
                .steps
                .iter()
                .filter_map(|s| s.equation.as_ref())
                .all(|e| full.contains(&e.latex())));
        }
    }
}
