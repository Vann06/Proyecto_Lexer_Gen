// esturcutra de datos para los items LR(0) 
// generación del autómata 
use std::collections::{HashSet, HashMap};
use super::grammar::{Grammar, Symbol};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LR0Item {
    pub head: String,
    pub body: Vec<Symbol>,
    pub dot_pos: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub id: usize,
    pub items: HashSet<LR0Item>,
    pub origin: Option<(usize, Symbol)>, // NUEVO: Guarda de dónde vino Goto(I, X)
}

#[derive(Debug)]
pub struct LR0Automaton {
    pub states: Vec<State>,
    pub transitions: HashMap<(usize, Symbol), usize>,
    pub start_head: String, // Para identificar la regla aumentada fácilmente
}

impl LR0Automaton {
    pub fn closure(items: &HashSet<LR0Item>, grammar: &Grammar) -> HashSet<LR0Item> {
        let mut closure_set = items.clone();
        let mut changed = true;

        while changed {
            changed = false;
            let mut new_items = HashSet::new();

            for item in &closure_set {
                if item.dot_pos < item.body.len() {
                    let symbol_after_dot = &item.body[item.dot_pos];

                    if let Symbol::NonTerminal(nt_name) = symbol_after_dot {
                        for production in &grammar.productions {
                            if production.head == *nt_name {
                                for body in &production.bodies {
                                    let new_item = LR0Item {
                                        head: production.head.clone(),
                                        body: body.clone(),
                                        dot_pos: 0,
                                    };
                                    if !closure_set.contains(&new_item) && !new_items.contains(&new_item) {
                                        new_items.insert(new_item);
                                    }
                                }
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

    pub fn goto(state_items: &HashSet<LR0Item>, symbol: &Symbol, grammar: &Grammar) -> HashSet<LR0Item> {
        let mut moved_items = HashSet::new();

        for item in state_items {
            if item.dot_pos < item.body.len() && item.body[item.dot_pos] == *symbol {
                moved_items.insert(LR0Item {
                    head: item.head.clone(),
                    body: item.body.clone(),
                    dot_pos: item.dot_pos + 1,
                });
            }
        }
        Self::closure(&moved_items, grammar)
    }

    pub fn build(grammar: &Grammar) -> Self {
        let mut states: Vec<State> = Vec::new();
        let mut transitions: HashMap<(usize, Symbol), usize> = HashMap::new();

        // 1. AUTO-AUMENTAR LA GRAMÁTICA
        let (start_head, start_body) = if grammar.start_symbol.ends_with('\'') || grammar.start_symbol.contains("prima") {
            (grammar.start_symbol.clone(), grammar.productions[0].bodies[0].clone())
        } else {
            (format!("{}'", grammar.start_symbol), vec![Symbol::NonTerminal(grammar.start_symbol.clone())])
        };

        let start_item = LR0Item {
            head: start_head.clone(),
            body: start_body,
            dot_pos: 0,
        };

        let mut initial_set = HashSet::new();
        initial_set.insert(start_item);
        
        let state0_items = Self::closure(&initial_set, grammar);
        states.push(State { id: 0, items: state0_items, origin: None });

        let mut unprocessed_states = vec![0];

        // 2. PROCESAR EN ORDEN DE ITEMS KERNEL (Como se hace a mano)
        while !unprocessed_states.is_empty() {
            let current_state_id = unprocessed_states.remove(0);
            
            // Ordenar los items del estado actual: Kernel primero, luego alfabético
            let mut current_items: Vec<_> = states[current_state_id].items.iter().collect();
            current_items.sort_by(|a, b| {
                let a_is_kernel = a.dot_pos > 0 || a.head == start_head;
                let b_is_kernel = b.dot_pos > 0 || b.head == start_head;
                if a_is_kernel == b_is_kernel { a.cmp(b) } 
                else if a_is_kernel { std::cmp::Ordering::Less } 
                else { std::cmp::Ordering::Greater }
            });

            // Extraer símbolos a procesar respetando el orden de los items!
            let mut symbols_to_process = Vec::new();
            for item in current_items {
                if item.dot_pos < item.body.len() {
                    let sym = item.body[item.dot_pos].clone();
                    if !symbols_to_process.contains(&sym) {
                        symbols_to_process.push(sym);
                    }
                }
            }

            // Para cada símbolo en orden, calculamos GOTO
            for symbol in &symbols_to_process {
                let next_items = Self::goto(&states[current_state_id].items, symbol, grammar);

                if !next_items.is_empty() {
                    let existing_state_id = states.iter().find(|s| s.items == next_items).map(|s| s.id);

                    let destination_id = match existing_state_id {
                        Some(id) => id,
                        None => {
                            let new_id = states.len();
                            states.push(State { 
                                id: new_id, 
                                items: next_items,
                                origin: Some((current_state_id, symbol.clone())) 
                            });
                            unprocessed_states.push(new_id);
                            new_id
                        }
                    };
                    transitions.insert((current_state_id, symbol.clone()), destination_id);
                }
            }
        }
        LR0Automaton { states, transitions, start_head }
    }
}