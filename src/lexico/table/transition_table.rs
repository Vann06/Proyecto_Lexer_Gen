// Fase 11: Convertir AFD en tabla de transiciones delta[state][symbol].
// Núcleo del lexer en tiempo de ejecución (acceso O(1)).

use std::collections::HashSet;
use crate::lexico::automata::dfa::Dfa;

pub const DEAD: i32 = -1;

#[derive(Debug, Clone)]
pub struct TransitionTable {
    /// delta[state][char as usize] = next_state o DEAD (solo ASCII 0..127)
    pub delta: Vec<Vec<i32>>,
    /// accept[state] = Some(acción/token) o None
    pub accept: Vec<Option<String>>,
    pub start: usize,
    pub n_states: usize,
    pub alphabet: Vec<char>,
}

impl TransitionTable {
    pub fn next(&self, state: usize, c: char) -> i32 {
        if state >= self.n_states || (c as usize) >= 128 {
            return DEAD;
        }
        self.delta[state][c as usize]
    }

    pub fn is_accepting(&self, state: usize) -> bool {
        state < self.n_states && self.accept[state].is_some()
    }

    pub fn token_at(&self, state: usize) -> Option<&str> {
        self.accept.get(state).and_then(|o| o.as_deref())
    }
}

/// Deriva el nombre del token a partir del código de acción de una regla
/// del `.yal` (el string crudo guardado en `accept[state]`, ej. `return NUM`,
/// `Token::Id`, `{ "PLUS" }`, `skip`, o vacío). Única fuente de verdad para
/// esto: antes vivía completa en `Simulator::next_token` y, por separado,
/// una versión empobrecida en `codegen::rust_codegen` que solo entendía
/// `Token::X` (con fallback a `"Unknown"`) — 5 de los 8 `.yal` de ejemplo
/// del repo (hardtest, hardtest_sim, miniprog, python_hard, python) usan el
/// estilo `{ "X" }`, que esa copia nunca reconocía.
pub fn kind_from_action(action: &str) -> String {
    let clean_act = action.trim();
    if clean_act.is_empty()
        || clean_act.eq_ignore_ascii_case("skip")
        || clean_act.eq_ignore_ascii_case("ignore")
    {
        // Reglas con acciones vacías, `skip` o `ignore` descartan el token
        // (ej. ws { }, ws { skip }).
        "Ignored".to_string()
    } else if clean_act.starts_with("return") {
        // Manejar acciones del estilo `return NUM` o `return Token::NUM`
        let tail = clean_act.trim_start_matches("return").trim();
        if let Some(idx) = tail.find("Token::") {
            let tail = &tail[idx + 7..];
            let end = tail
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            tail[..end].to_string()
        } else if tail.starts_with('"') {
            if let Some(second_quote) = tail[1..].find('"') {
                tail[1..1 + second_quote].to_string()
            } else {
                tail.to_string()
            }
        } else {
            let end = tail
                .find(|c: char| !c.is_alphanumeric() && c != '_')
                .unwrap_or(tail.len());
            tail[..end].to_string()
        }
    } else if let Some(idx) = action.find("Token::") {
        let tail = &action[idx + 7..];
        let end = tail
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(tail.len());
        tail[..end].to_string()
    } else if action.contains("None") {
        "Ignored".to_string()
    } else if let Some(first_quote) = action.find('"') {
        if let Some(second_quote) = action[first_quote + 1..].find('"') {
            action[first_quote + 1..first_quote + 1 + second_quote].to_string()
        } else {
            clean_act.to_string()
        }
    } else {
        clean_act.to_string()
    }
}

/// Construye la tabla de transición a partir del DFA minimizado.
/// Se asume que los IDs de estado del DFA son contiguos 0..n-1.
pub fn build(dfa: &Dfa) -> TransitionTable {
    let n = dfa.states.len();
    let mut delta: Vec<Vec<i32>> = vec![vec![DEAD; 128]; n];
    let mut accept: Vec<Option<String>> = vec![None; n];
    let mut alphabet_set: HashSet<char> = HashSet::new();

    for (_id, state) in &dfa.states {
        for (c, to_id) in &state.transitions {
            if (*c as usize) < 128 {
                delta[state.id][*c as usize] = *to_id as i32;
                alphabet_set.insert(*c);
            }
        }
        accept[state.id] = state
            .accept_action
            .as_ref()
            .map(|(_, s)| s.clone());
    }

    let mut alphabet: Vec<char> = alphabet_set.into_iter().collect();
    alphabet.sort_unstable();

    TransitionTable {
        delta,
        accept,
        start: dfa.start_state,
        n_states: n,
        alphabet,
    }
}

/// Imprime la tabla para depuración (consola).
#[allow(dead_code)]
pub fn print_table(tt: &TransitionTable) {
    print!("{:>8}", "Estado");
    for &c in &tt.alphabet {
        print!("{:>6}", c);
    }
    println!();
    for s in 0..tt.n_states {
        let marker = if tt.is_accepting(s) {
            format!("*{}", tt.token_at(s).unwrap_or(""))
        } else {
            s.to_string()
        };
        print!("{:>8}", marker);
        for &c in &tt.alphabet {
            let v = tt.next(s, c);
            if v == DEAD {
                print!("{:>6}", "-");
            } else {
                print!("{:>6}", v);
            }
        }
        println!();
    }
}

/// DFA dummy para tests: acepta uno o más dígitos como "NUM".
#[cfg(test)]
pub(crate) fn make_dummy_dfa_num() -> Dfa {
    use std::collections::HashMap;
    use crate::lexico::automata::dfa::DfaState;
    let mut states = HashMap::new();
    let mut trans0 = HashMap::new();
    for c in '0'..='9' {
        trans0.insert(c, 1);
    }
    states.insert(
        0,
        DfaState {
            id: 0,
            transitions: trans0,
            accept_action: None,
        },
    );
    let mut trans1 = HashMap::new();
    for c in '0'..='9' {
        trans1.insert(c, 1);
    }
    states.insert(
        1,
        DfaState {
            id: 1,
            transitions: trans1,
            accept_action: Some((0, "NUM".to_string())),
        },
    );
    let mut dfa = Dfa::new(0);
    dfa.states = states;
    dfa
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dfa_num() {
        let dfa = make_dummy_dfa_num();
        let tt = build(&dfa);
        assert_eq!(tt.n_states, 2);
        assert_eq!(tt.start, 0);
        assert!(!tt.is_accepting(0));
        assert!(tt.is_accepting(1));
        assert_eq!(tt.token_at(0), None);
        assert_eq!(tt.token_at(1), Some("NUM"));
        assert_eq!(tt.next(0, '5'), 1);
        assert_eq!(tt.next(0, 'a'), DEAD);
        assert_eq!(tt.next(1, '9'), 1);
    }

    #[test]
    fn kind_from_action_handles_token_colon_style() {
        assert_eq!(kind_from_action("return Token::NUM;"), "NUM");
        assert_eq!(kind_from_action("Token::Id"), "Id");
    }

    #[test]
    fn kind_from_action_handles_return_bare_ident() {
        assert_eq!(kind_from_action("return NUM"), "NUM");
    }

    #[test]
    fn kind_from_action_handles_return_quoted_string() {
        assert_eq!(kind_from_action(r#"return "NUM""#), "NUM");
    }

    #[test]
    fn kind_from_action_handles_bare_quoted_string() {
        // Estilo usado por 5 de los 8 .yal de ejemplo del repo (python_hard,
        // hardtest, miniprog, ...) — el bug que este cambio arregla: la copia
        // vieja en rust_codegen no reconocía este caso y devolvía "Unknown".
        assert_eq!(kind_from_action(r#"{ "PLUS" }"#), "PLUS");
    }

    #[test]
    fn kind_from_action_handles_skip_and_ignore() {
        assert_eq!(kind_from_action("skip"), "Ignored");
        assert_eq!(kind_from_action("  Ignore  "), "Ignored");
        assert_eq!(kind_from_action(""), "Ignored");
        assert_eq!(kind_from_action("   "), "Ignored");
    }

    #[test]
    fn kind_from_action_handles_none() {
        assert_eq!(kind_from_action("None"), "Ignored");
    }

    #[test]
    fn kind_from_action_falls_back_to_trimmed_action() {
        assert_eq!(kind_from_action("  PLUS  "), "PLUS");
    }
}
