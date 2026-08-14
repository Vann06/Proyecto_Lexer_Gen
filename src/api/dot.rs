// Exporta el autómata LR(0) a formato DOT para la vista de grafo del IDE.
// `format_lr0_item` vive en `sintactico` porque también lo usa la conversión
// del autómata a JSON (`lr0_states_to_data`); este módulo solo lo consume.
use crate::sintactico::automatas::lr0::LR0Automaton;

use super::sintactico::format_lr0_item;

pub(crate) fn lr0_to_dot(automaton: &LR0Automaton) -> String {
    let mut dot = String::new();
    dot.push_str("digraph LR0 {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  bgcolor=\"#0d0613\";\n");
    dot.push_str("  node [shape=box fontname=\"Courier\" fontsize=9 color=\"#c026d3\" fontcolor=\"#e8d6f0\" style=filled fillcolor=\"#100817\"];\n");
    dot.push_str("  edge [fontname=\"Courier\" fontsize=9 color=\"#c026d3\" fontcolor=\"#f9a8d4\" arrowsize=0.7];\n");

    let mut sorted_states: Vec<_> = automaton.states.iter().collect();
    sorted_states.sort_by_key(|s| s.id);

    for state in sorted_states {
        let mut items: Vec<String> = state.items.iter().map(|it| format_lr0_item(it)).collect();
        items.sort();
        let label_body = items.join("\\l");
        let label = format!("I{}\\l{}\\l", state.id, label_body).replace('"', "\\\"");

        if state.id == 0 {
            dot.push_str(&format!(
                "  {} [label=\"{}\" color=\"#22d3ee\" fillcolor=\"#0a2530\"];\n",
                state.id, label
            ));
        } else {
            dot.push_str(&format!("  {} [label=\"{}\"];\n", state.id, label));
        }
    }

    let mut transitions: Vec<_> = automaton.transitions.iter().collect();
    transitions.sort_by(|a, b| {
        let (af, _) = a.0;
        let (bf, _) = b.0;
        af.cmp(bf).then(a.1.cmp(b.1))
    });

    for ((from, sym), to) in transitions {
        let sym_label = sym.to_string().replace('"', "\\\"");
        dot.push_str(&format!("  {} -> {} [label=\"{}\"];\n", from, to, sym_label));
    }

    dot.push_str("}\n");
    dot
}
