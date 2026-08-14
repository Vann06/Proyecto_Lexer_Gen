// Fachada del pipeline léxico para la capa API: compila un .yal hasta la
// tabla de transición (y, para codegen, hasta los artefactos intermedios que
// necesita), más los pequeños helpers de normalización de kind que usa
// `pipeline` al filtrar el token_map.
use crate::lexico::spec::ast::SpecIR;
use crate::lexico::spec::expand::{expand_definitions, ExpandedRule};
use crate::lexico::spec::parser::parse_yalex;
use crate::lexico::regex::parser::parse_regex;
use crate::lexico::table::transition_table::TransitionTable;
use crate::sintactico::gramatica::grammar::Grammar;

/// Corre el pipeline lexer completo (parse .yal → expandir macros → NFA por
/// regla → NFA maestro → DFA por subconjuntos → DFA minimizado → tabla) y
/// devuelve TODO lo que produce, no solo la tabla — `codegen::rust_codegen`
/// necesita además `spec.header`/`spec.trailer` y las reglas expandidas,
/// que la versión vieja (que solo devolvía la tabla) descartaba.
pub fn build_lexer_artifacts(
    yal_src: &str,
) -> Result<(SpecIR, Vec<ExpandedRule>, TransitionTable), String> {
    let spec = parse_yalex(yal_src).map_err(|e| format!("Error al parsear .yal: {}", e))?;
    let expanded = expand_definitions(&spec);

    let mut id_counter = 0usize;
    let mut nfas = Vec::new();
    for rule in &expanded {
        let ast = parse_regex(&rule.pattern_expanded)
            .map_err(|e| format!("Error en regex '{}': {}", rule.pattern_expanded, e))?;
        let mut nfa = crate::lexico::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        nfas.push(nfa);
    }
    let master = crate::lexico::automata::nfa::combine_nfas(nfas, &mut id_counter);
    let dfa = crate::lexico::automata::subset::build_dfa_from_nfa(&master);
    let min_dfa = crate::lexico::automata::minimize::minimize_dfa(&dfa);
    let table = crate::lexico::table::transition_table::build(&min_dfa);
    Ok((spec, expanded, table))
}

pub(crate) fn build_lexer_table_from_str(yal_src: &str) -> Result<TransitionTable, String> {
    build_lexer_artifacts(yal_src).map(|(_, _, table)| table)
}

pub(crate) fn lex_normalize_kind(kind: &str) -> String {
    kind.to_uppercase()
}

pub(crate) fn lex_is_ignored(kind: &str, grammar: &Grammar) -> bool {
    let lower = kind.to_lowercase();
    lower.contains("whitespace") || lower.contains("comment") || lower == "ignored" || grammar.ignores.contains(kind)
}
