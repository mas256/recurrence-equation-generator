//! Textbook output is rendered from the exact same IR as the web view.
use crate::generator::GeneratedProblem;

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
pub fn tex(problem: &GeneratedProblem, include_solution: bool) -> String {
    let mut output = format!(
        r#"% UTF-8 / pLaTeX + dvipdfmx
% Problem content SHA-256: {}
% Rules: {}
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
        for step in &problem.derivation.steps {
            output.push_str(&prose(&step.text));
            output.push_str("\n\n");
            if let Some(equation) = &step.equation {
                output.push_str(&display(&equation.latex()));
            }
        }
        output.push_str("\\subsection*{一般項}\n");
        output.push_str(&display(&problem.problem.general_term.latex()));
        for equation in &problem.problem.secondary_general_terms {
            output.push_str(&display(&equation.latex()));
        }
        for alternative in &problem.derivation.alternatives {
            output.push_str("\\subsection*{別解}\n");
            output.push_str(&prose(alternative));
            output.push('\n');
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
        let p = crate::generator::generate(4, Some("reciprocal_affine"), 32).unwrap();
        let question = tex(&p, false);
        let full = tex(&p, true);
        assert!(question.contains(&p.problem.recurrence.latex()));
        assert!(!question.contains(&p.problem.general_term.latex()));
        assert!(full.contains(&p.problem.general_term.latex()));
        assert!(full.contains("\\columnsep}{12pt}"));
    }
}
