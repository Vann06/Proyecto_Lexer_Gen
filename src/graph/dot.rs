// Fase 6: Exportar AST de regex y DFA a formato Graphviz DOT.

use std::fs;
use crate::regex::ast::RegexAst;
use crate::automata::dfa::Dfa;

/// Genera un archivo .dot con el árbol de la expresión regular.
#[allow(dead_code)]
pub fn write_ast_dot(path: &str, root: &RegexAst) -> std::io::Result<()> {
    let mut out = String::from("digraph AST {\n  node [shape=box];\n");
    let mut id = 0usize;
    ast_node(&mut out, root, &mut id);
    out.push('}');
    fs::write(path, out)
}

#[allow(dead_code)]
fn ast_node(out: &mut String, node: &RegexAst, id: &mut usize) -> usize {
    let my_id = *id;
    *id += 1;
    let label = match node {
        RegexAst::Literal(c) => format!("'{}'", c),
        RegexAst::Concat(_, _) => "·".into(),
        RegexAst::Union(_, _) => "|".into(),
        RegexAst::Star(_) => "*".into(),
        RegexAst::Plus(_) => "+".into(),
        RegexAst::Optional(_) => "?".into(),
        RegexAst::Group(_) => "()".into(),
        RegexAst::CharClass(s) => format!("[{}]", s),
        RegexAst::Empty => "ε".into(),
    };
    out.push_str(&format!("  n{} [label=\"{}\"];\n", my_id, escape_dot_label(&label)));

    match node {
        RegexAst::Literal(_) | RegexAst::CharClass(_) | RegexAst::Empty => {}
        RegexAst::Star(inner) | RegexAst::Plus(inner) | RegexAst::Optional(inner) | RegexAst::Group(inner) => {
            let child = ast_node(out, inner, id);
            out.push_str(&format!("  n{} -> n{};\n", my_id, child));
        }
        RegexAst::Union(a, b) | RegexAst::Concat(a, b) => {
            let la = ast_node(out, a, id);
            let lb = ast_node(out, b, id);
            out.push_str(&format!("  n{} -> n{};\n  n{} -> n{};\n", my_id, la, my_id, lb));
        }
    }
    my_id
}

fn escape_dot_label(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Genera un archivo .dot con el grafo del DFA.
pub fn write_dfa_dot(path: &str, dfa: &Dfa) -> std::io::Result<()> {
    let mut out = String::from("digraph DFA {\n  rankdir=LR;\n  node [shape=circle];\n");
    out.push_str("  __start__ [shape=none label=\"\"];\n");
    out.push_str(&format!("  __start__ -> {};\n", dfa.start_state));

    for (_id, state) in &dfa.states {
        let label = if state.accept_action.is_some() {
            let action = state
                .accept_action
                .as_ref()
                .map(|(_, s)| s.as_str())
                .unwrap_or("");
            format!("{}\\n{}", state.id, action)
        } else {
            state.id.to_string()
        };
        let shape = if state.accept_action.is_some() {
            "doublecircle"
        } else {
            "circle"
        };
        out.push_str(&format!(
            "  {} [shape={} label=\"{}\"];\n",
            state.id,
            shape,
            escape_dot_label(&label)
        ));
    }

    for (_id, state) in &dfa.states {
        for (c, to_id) in &state.transitions {
            let label = if *c == '\0' {
                "ε".to_string()
            } else {
                c.to_string()
            };
            out.push_str(&format!(
                "  {} -> {} [label=\"{}\"];\n",
                state.id,
                to_id,
                escape_dot_label(&label)
            ));
        }
    }
    out.push('}');
    fs::write(path, out)
}
