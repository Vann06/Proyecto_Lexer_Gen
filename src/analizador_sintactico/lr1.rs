use std::collections::{HashSet, HashMap};
use super::grammar::{Grammar, Symbol};
use super::first::{FirstSets, first_of_sequence, EPSILON};
use super::follow::EOF;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LR1Item {
    pub head: String,
    pub body: Vec<Symbol>,
    pub dot_pos: usize,
    pub lookahead: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LR1State {
    pub id: usize,
    pub items: HashSet<LR1Item>,
    pub origin: Option<(usize, Symbol)>,
}

#[derive(Debug)]
pub struct LR1Automaton {
    pub states: Vec<LR1State>,
    pub transitions: HashMap<(usize, Symbol), usize>,
    pub start_head: String,
}

impl LR1Automaton {
    /// Calcula el closure LR(1) de un conjunto de ítems.
    /// Para cada ítem [A → α . B β, a], agrega [B → . γ, b]
    /// para cada producción B → γ y cada b ∈ FIRST(β a).
    pub fn closure(items: &HashSet<LR1Item>, grammar: &Grammar, first_sets: &FirstSets) -> HashSet<LR1Item> {
        let mut closure_set = items.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let mut new_items: HashSet<LR1Item> = HashSet::new();

            for item in &closure_set {
                if item.dot_pos >= item.body.len() {
                    continue;
                }
                let symbol_after_dot = &item.body[item.dot_pos];
                let nt_name = match symbol_after_dot {
                    Symbol::NonTerminal(nt) => nt,
                    Symbol::Terminal(_) => continue,
                };

                // Calcular FIRST(β a) donde β = cuerpo después del punto, a = lookahead
                let mut seq: Vec<Symbol> = item.body[item.dot_pos + 1..].to_vec();
                seq.push(Symbol::Terminal(item.lookahead.clone()));
                let lookaheads = first_of_sequence(&seq, first_sets);

                for production in &grammar.productions {
                    if production.head != *nt_name {
                        continue;
                    }
                    for body in &production.bodies {
                        for la in &lookaheads {
                            if la == EPSILON {
                                continue; // siempre termina en terminal, no debería ocurrir
                            }
                            let new_item = LR1Item {
                                head: production.head.clone(),
                                body: body.clone(),
                                dot_pos: 0,
                                lookahead: la.clone(),
                            };
                            if !closure_set.contains(&new_item) {
                                new_items.insert(new_item);
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

    /// GOTO(I, X): mueve el punto sobre X y aplica closure.
    pub fn goto(state_items: &HashSet<LR1Item>, symbol: &Symbol, grammar: &Grammar, first_sets: &FirstSets) -> HashSet<LR1Item> {
        let mut moved_items = HashSet::new();

        for item in state_items {
            if item.dot_pos < item.body.len() && item.body[item.dot_pos] == *symbol {
                moved_items.insert(LR1Item {
                    head: item.head.clone(),
                    body: item.body.clone(),
                    dot_pos: item.dot_pos + 1,
                    lookahead: item.lookahead.clone(),
                });
            }
        }
        Self::closure(&moved_items, grammar, first_sets)
    }

    /// Construye el autómata LR(1) canónico completo.
    /// Auto-aumenta la gramática con S' → S si es necesario.
    pub fn build(grammar: &Grammar, first_sets: &FirstSets) -> Self {
        let mut states: Vec<LR1State> = Vec::new();
        let mut transitions: HashMap<(usize, Symbol), usize> = HashMap::new();

        // Auto-aumentación: S' → S  (igual que en lr0::build)
        let (start_head, start_body) = if grammar.start_symbol.ends_with('\'') || grammar.start_symbol.contains("prima") {
            (grammar.start_symbol.clone(), grammar.productions[0].bodies[0].clone())
        } else {
            (format!("{}'", grammar.start_symbol), vec![Symbol::NonTerminal(grammar.start_symbol.clone())])
        };

        let start_item = LR1Item {
            head: start_head.clone(),
            body: start_body,
            dot_pos: 0,
            lookahead: EOF.to_string(),
        };

        let mut initial_set = HashSet::new();
        initial_set.insert(start_item);

        let state0_items = Self::closure(&initial_set, grammar, first_sets);
        states.push(LR1State { id: 0, items: state0_items, origin: None });

        let mut unprocessed = vec![0usize];

        while !unprocessed.is_empty() {
            let current_id = unprocessed.remove(0);

            // Ordenar ítems kernel-primero para numeración determinista de estados
            let mut current_items: Vec<_> = states[current_id].items.iter().collect();
            current_items.sort_by(|a, b| {
                let a_kernel = a.dot_pos > 0 || a.head == start_head;
                let b_kernel = b.dot_pos > 0 || b.head == start_head;
                if a_kernel == b_kernel { a.cmp(b) }
                else if a_kernel { std::cmp::Ordering::Less }
                else { std::cmp::Ordering::Greater }
            });

            // Recolectar símbolos en el orden en que aparecen (determinismo)
            let mut symbols_to_process: Vec<Symbol> = Vec::new();
            for item in &current_items {
                if item.dot_pos < item.body.len() {
                    let sym = item.body[item.dot_pos].clone();
                    if !symbols_to_process.contains(&sym) {
                        symbols_to_process.push(sym);
                    }
                }
            }

            for symbol in &symbols_to_process {
                let next_items = Self::goto(&states[current_id].items, symbol, grammar, first_sets);
                if next_items.is_empty() {
                    continue;
                }

                let dest_id = match states.iter().find(|s| s.items == next_items) {
                    Some(s) => s.id,
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
