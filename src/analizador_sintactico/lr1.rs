// Autómata LR(1) canónico.
//
// Cada ítem LR(1) lleva un lookahead: el terminal que DEBE seguir después de
// reducir la regla para que la reducción sea válida.  Eso permite al parser
// distinguir contextos que SLR confunde (usa FOLLOW global).
use std::collections::{HashMap, HashSet};
use super::grammar::{Grammar, Symbol};
use super::first::{FirstSets, first_of_sequence, EPSILON};

pub const EOF: &str = "$";

// ─────────────────────────────────────────────────────────────────────────────
// Ítem LR(1): [A -> α • β, a]
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LR1Item {
    pub head: String,
    pub body: Vec<Symbol>,
    pub dot_pos: usize,
    pub lookahead: String, // terminal o "$"
}

impl LR1Item {
    pub fn is_reduce_item(&self) -> bool {
        self.dot_pos == self.body.len()
    }

    /// Devuelve una cadena legible: [A -> α • β, a]
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        for (i, sym) in self.body.iter().enumerate() {
            if i == self.dot_pos {
                parts.push("•".to_string());
            }
            parts.push(match sym {
                Symbol::Terminal(t) => t.clone(),
                Symbol::NonTerminal(nt) => nt.clone(),
            });
        }
        if self.dot_pos == self.body.len() {
            parts.push("•".to_string());
        }
        let body_str = if parts.is_empty() {
            "•".to_string()
        } else {
            parts.join(" ")
        };
        format!("[{} -> {}, {}]", self.head, body_str, self.lookahead)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Estado del autómata
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LR1State {
    pub id: usize,
    pub items: HashSet<LR1Item>,
    pub origin: Option<(usize, Symbol)>, // (estado_origen, símbolo_leído)
}

// ─────────────────────────────────────────────────────────────────────────────
// Autómata LR(1)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LR1Automaton {
    pub states: Vec<LR1State>,
    pub transitions: HashMap<(usize, Symbol), usize>,
    pub start_head: String, // cabeza de la regla aumentada (ej. "S'")
}

impl LR1Automaton {
    /// CERRADURA LR(1)
    ///
    /// Para cada ítem [A -> α • B β, a]:
    ///   - β_plus_a = β ++ [Terminal(a)]
    ///   - Para cada producción B -> γ y cada b ∈ FIRST(β_plus_a):
    ///       agregar [B -> • γ, b]
    pub fn closure(items: &HashSet<LR1Item>, grammar: &Grammar, first_sets: &FirstSets) -> HashSet<LR1Item> {
        let mut closure_set = items.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let mut new_items: HashSet<LR1Item> = HashSet::new();

            for item in &closure_set {
                if item.is_reduce_item() {
                    continue;
                }

                let Symbol::NonTerminal(nt_name) = &item.body[item.dot_pos] else {
                    continue; // terminal tras el punto → solo hay Shift, no Closure
                };

                // β = sufijo del cuerpo DESPUÉS de B; añadimos el lookahead al final
                let beta_plus_a: Vec<Symbol> = item.body[item.dot_pos + 1..]
                    .iter()
                    .cloned()
                    .chain(std::iter::once(Symbol::Terminal(item.lookahead.clone())))
                    .collect();

                let lookaheads = first_of_sequence(&beta_plus_a, first_sets);

                for production in &grammar.productions {
                    if production.head != *nt_name {
                        continue;
                    }
                    for body in &production.bodies {
                        for la in &lookaheads {
                            if la == EPSILON {
                                continue;
                            }
                            let candidate = LR1Item {
                                head: production.head.clone(),
                                body: body.clone(),
                                dot_pos: 0,
                                lookahead: la.clone(),
                            };
                            if !closure_set.contains(&candidate) {
                                new_items.insert(candidate);
                            }
                        }
                    }
                }
            }

            if !new_items.is_empty() {
                closure_set.extend(new_items);
                changed = true;
            }
        }

        closure_set
    }

    /// GOTO(estado, símbolo): avanza el punto sobre `symbol` y aplica CERRADURA
    pub fn goto(
        state_items: &HashSet<LR1Item>,
        symbol: &Symbol,
        grammar: &Grammar,
        first_sets: &FirstSets,
    ) -> HashSet<LR1Item> {
        let mut moved: HashSet<LR1Item> = HashSet::new();

        for item in state_items {
            if !item.is_reduce_item() && &item.body[item.dot_pos] == symbol {
                moved.insert(LR1Item {
                    head: item.head.clone(),
                    body: item.body.clone(),
                    dot_pos: item.dot_pos + 1,
                    lookahead: item.lookahead.clone(),
                });
            }
        }

        Self::closure(&moved, grammar, first_sets)
    }

    /// Construye el autómata completo por BFS sobre los estados.
    ///
    /// La gramática se AUMENTA automáticamente si el símbolo inicial no tiene
    /// ya el formato S' / prima.
    pub fn build(grammar: &Grammar, first_sets: &FirstSets) -> Self {
        let mut states: Vec<LR1State> = Vec::new();
        let mut transitions: HashMap<(usize, Symbol), usize> = HashMap::new();

        // Gramática aumentada: S' -> S  con lookahead $
        let (start_head, start_body) = if grammar.start_symbol.ends_with('\'')
            || grammar.start_symbol.contains("prima")
        {
            (
                grammar.start_symbol.clone(),
                grammar.productions[0].bodies[0].clone(),
            )
        } else {
            (
                format!("{}'", grammar.start_symbol),
                vec![Symbol::NonTerminal(grammar.start_symbol.clone())],
            )
        };

        let seed = LR1Item {
            head: start_head.clone(),
            body: start_body,
            dot_pos: 0,
            lookahead: EOF.to_string(),
        };

        let mut seed_set = HashSet::new();
        seed_set.insert(seed);
        let state0_items = Self::closure(&seed_set, grammar, first_sets);
        states.push(LR1State { id: 0, items: state0_items, origin: None });

        let mut unprocessed: Vec<usize> = vec![0];

        while !unprocessed.is_empty() {
            let current_id = unprocessed.remove(0);

            // Ordenar items: kernel primero, luego alfabético
            let mut sorted_items: Vec<&LR1Item> = states[current_id].items.iter().collect();
            sorted_items.sort_by(|a, b| {
                let ak = a.dot_pos > 0 || a.head == start_head;
                let bk = b.dot_pos > 0 || b.head == start_head;
                match (ak, bk) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.cmp(b),
                }
            });

            // Recoger los símbolos que aparecen tras el punto
            let mut symbols_to_visit: Vec<Symbol> = Vec::new();
            for item in &sorted_items {
                if !item.is_reduce_item() {
                    let sym = item.body[item.dot_pos].clone();
                    if !symbols_to_visit.contains(&sym) {
                        symbols_to_visit.push(sym);
                    }
                }
            }

            for symbol in &symbols_to_visit {
                let next_items = Self::goto(&states[current_id].items, symbol, grammar, first_sets);
                if next_items.is_empty() {
                    continue;
                }

                let existing_id = states.iter().find(|s| s.items == next_items).map(|s| s.id);
                let dest_id = match existing_id {
                    Some(id) => id,
                    None => {
                        let new_id = states.len();
                        states.push(LR1State {
                            id: new_id,
                            items: next_items,
                            origin: Some((current_id, symbol.clone())),
                        });
                        unprocessed.push(new_id);
                        new_id
                    }
                };
                transitions.insert((current_id, symbol.clone()), dest_id);
            }
        }

        LR1Automaton { states, transitions, start_head }
    }
}
