// Tabla ACTION/GOTO universal para parsers LR.
// Es independiente del método de construcción: la misma estructura sirve para
// LALR(1) (actual), SLR(1)/LR(0)/LR(1) canónico (futuro). Solo cambia el
// constructor `build_from_*` que la rellena. El driver `LRParser` la consume
// indistintamente.

use std::collections::HashMap;
use super::grammar::{Associativity, Grammar, Symbol};
use super::lalr::{LALRAutomaton, LALRItem};
use super::lr0::LR0Automaton;
use super::follow::FollowSets;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Shift(usize),
    Reduce { head: String, body: Vec<Symbol> },
    Accept,
}

#[derive(Debug, Clone)]
pub enum Conflict {
    ShiftReduce {
        state: usize,
        terminal: String,
        shift_to: usize,
        reduce_with: (String, Vec<Symbol>),
    },
    ReduceReduce {
        state: usize,
        terminal: String,
        kept: (String, Vec<Symbol>),
        discarded: (String, Vec<Symbol>),
    },
}

impl Conflict {
    pub fn describe(&self) -> String {
        match self {
            Conflict::ShiftReduce { state, terminal, shift_to, reduce_with: (head, body) } => {
                let body_str = body_to_str(body);
                format!(
                    "SHIFT-REDUCE en estado I{} con '{}': shift→I{} vs reduce ({} → {}). Se conserva SHIFT.",
                    state, terminal, shift_to, head, body_str
                )
            }
            Conflict::ReduceReduce { state, terminal, kept: (h1, b1), discarded: (h2, b2) } => {
                format!(
                    "REDUCE-REDUCE en estado I{} con '{}': ({} → {}) vs ({} → {}). Se conserva la primera.",
                    state, terminal, h1, body_to_str(b1), h2, body_to_str(b2)
                )
            }
        }
    }
}

fn body_to_str(body: &[Symbol]) -> String {
    if body.is_empty() {
        return "ε".to_string();
    }
    body.iter()
        .map(|s| match s {
            Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub struct LRTable {
    pub action: HashMap<(usize, String), Action>,
    pub goto: HashMap<(usize, String), usize>,
    pub start_state: usize,
    pub start_head: String,
    pub conflicts: Vec<Conflict>,
}

impl LRTable {
    /// Construye la tabla ACTION/GOTO a partir del autómata LALR.
    ///   - Shift-Reduce: gana Shift (estilo yacc) o se resuelve por precedencia.
    ///   - Reduce-Reduce: gana la producción de menor índice en la gramática.
    pub fn build_from_lalr(automaton: &LALRAutomaton, grammar: &Grammar) -> Self {
        let mut action: HashMap<(usize, String), Action> = HashMap::new();
        let mut goto: HashMap<(usize, String), usize> = HashMap::new();
        let mut conflicts: Vec<Conflict> = Vec::new();

        let prod_index = build_production_index(grammar);
        let prec_map   = build_prec_map(grammar);

        // 1. Rellenar ACTION y GOTO desde las transiciones
        for ((from, sym), to) in &automaton.transitions {
            match sym {
                Symbol::Terminal(t) => {
                    insert_action(
                        &mut action, &mut conflicts,
                        *from, t.clone(), Action::Shift(*to),
                        &prod_index, &prec_map,
                    );
                }
                Symbol::NonTerminal(nt) => {
                    goto.insert((*from, nt.clone()), *to);
                }
            }
        }

        // 2. Rellenar ACTION desde los ítems completos (reduce / accept)
        for state in &automaton.states {
            for item in &state.items {
                if item.dot_pos != item.body.len() {
                    continue;
                }

                if item.head == automaton.start_head {
                    insert_action(
                        &mut action, &mut conflicts,
                        state.id, "$".to_string(), Action::Accept,
                        &prod_index, &prec_map,
                    );
                } else {
                    for la in &item.lookaheads {
                        insert_action(
                            &mut action, &mut conflicts,
                            state.id, la.clone(),
                            Action::Reduce { head: item.head.clone(), body: item.body.clone() },
                            &prod_index, &prec_map,
                        );
                    }
                }
            }
        }

        LRTable { action, goto, start_state: 0, start_head: automaton.start_head.clone(), conflicts }
    }

    /// Construye la tabla ACTION/GOTO a partir del autómata LR(0) + FOLLOW sets (SLR(1)).

    pub fn build_from_slr(automaton: &LR0Automaton, grammar: &Grammar, follow_sets: &FollowSets) -> Self {
        let mut action: HashMap<(usize, String), Action> = HashMap::new();
        let mut goto:   HashMap<(usize, String), usize>  = HashMap::new();
        let mut conflicts: Vec<Conflict> = Vec::new();

        let prod_index = build_production_index(grammar);
        let prec_map   = build_prec_map(grammar);

        // 1. Shifts y GOTO desde las transiciones del LR(0) — idéntico a LALR
        for ((from, sym), to) in &automaton.transitions {
            match sym {
                Symbol::Terminal(t) => {
                    insert_action(
                        &mut action, &mut conflicts,
                        *from, t.clone(), Action::Shift(*to),
                        &prod_index, &prec_map,
                    );
                }
                Symbol::NonTerminal(nt) => {
                    goto.insert((*from, nt.clone()), *to);
                }
            }
        }

        // 2. Reduces desde ítems completos usando FOLLOW(A) — diferencia clave vs LALR
        for state in &automaton.states {
            for item in &state.items {
                if item.dot_pos != item.body.len() {
                    continue; // ítem no completo
                }

                if item.head == automaton.start_head {
                    // Accept: [S' → S •] solo con $
                    insert_action(
                        &mut action, &mut conflicts,
                        state.id, "$".to_string(), Action::Accept,
                        &prod_index, &prec_map,
                    );
                } else {
                    // Reduce: por cada b ∈ FOLLOW(A)
                    if let Some(follow) = follow_sets.get(&item.head) {
                        for terminal in follow {
                            insert_action(
                                &mut action, &mut conflicts,
                                state.id, terminal.clone(),
                                Action::Reduce { head: item.head.clone(), body: item.body.clone() },
                                &prod_index, &prec_map,
                            );
                        }
                    }
                }
            }
        }

        LRTable { action, goto, start_state: 0, start_head: automaton.start_head.clone(), conflicts }
    }

    /// Imprime la tabla en formato 2D (terminales + no-terminales como columnas).
    pub fn print_table(&self, grammar: &Grammar) {
        // Recopilar columnas
        let mut terminals: Vec<String> = grammar.tokens.iter().cloned().collect();
        terminals.push("$".to_string());
        terminals.sort();

        let mut non_terminals: Vec<String> = grammar.productions.iter()
            .map(|p| p.head.clone())
            .collect();
        non_terminals.dedup();

        let col_w = 8usize;

        // Cabecera
        print!("| {:<6} |", "Estado");
        for t in &terminals {
            print!(" {:<width$} |", t, width = col_w);
        }
        for nt in &non_terminals {
            print!(" {:<width$} |", nt, width = col_w);
        }
        println!();

        // Separador
        print!("|--------|");
        for _ in 0..(terminals.len() + non_terminals.len()) {
            print!("{:-<width$}|", "", width = col_w + 2);
        }
        println!();

        // Filas — ordenar por id para salida estable
        let mut state_ids: Vec<usize> = self.action.keys().map(|(s, _)| *s)
            .chain(self.goto.keys().map(|(s, _)| *s))
            .collect();
        state_ids.sort();
        state_ids.dedup();

        for sid in state_ids {
            print!("| {:<6} |", format!("I{}", sid));
            for t in &terminals {
                let cell = match self.action.get(&(sid, t.clone())) {
                    Some(Action::Shift(n))  => format!("s{}", n),
                    Some(Action::Reduce { head, body }) => {
                        format!("r{}", production_number(grammar, head, body))
                    }
                    Some(Action::Accept)    => "acc".to_string(),
                    None                    => String::new(),
                };
                print!(" {:<width$} |", cell, width = col_w);
            }
            for nt in &non_terminals {
                let cell = match self.goto.get(&(sid, nt.clone())) {
                    Some(n) => format!("{}", n),
                    None    => String::new(),
                };
                print!(" {:<width$} |", cell, width = col_w);
            }
            println!();
        }
    }
}

// ─── Precedencia ─────────────────────────────────────────────────────────────

// pub so tests/regression_tests.rs (a separate integration-test crate) can drive
// insert_action directly for the A7 (%nonassoc error-cell resurrection) regression test.
pub struct PrecInfo {
    pub level: usize,
    pub assoc: Associativity,
}

fn build_prec_map(grammar: &Grammar) -> HashMap<String, PrecInfo> {
    let mut map = HashMap::new();
    for (level, (assoc, tokens)) in grammar.precedence.iter().enumerate() {
        for token in tokens {
            map.insert(token.clone(), PrecInfo { level, assoc: assoc.clone() });
        }
    }
    map
}

enum SR { KeepShift, DoReduce, NonAssocError }

/// Resuelve un conflicto shift-reduce usando precedencia declarada.
/// Devuelve None si alguno de los dos lados no tiene info de precedencia.
fn resolve_shift_reduce(
    terminal: &str,
    body: &[Symbol],
    prec_map: &HashMap<String, PrecInfo>,
) -> Option<SR> {
    let tok_info = prec_map.get(terminal)?;
    // Precedencia de la producción = terminal más a la derecha en el cuerpo
    let prod_info = body.iter().rev().find_map(|s| {
        if let Symbol::Terminal(t) = s { prec_map.get(t.as_str()) } else { None }
    })?;

    Some(match tok_info.level.cmp(&prod_info.level) {
        std::cmp::Ordering::Greater => SR::KeepShift,
        std::cmp::Ordering::Less    => SR::DoReduce,
        std::cmp::Ordering::Equal   => match tok_info.assoc {
            Associativity::Left     => SR::DoReduce,
            Associativity::Right    => SR::KeepShift,
            Associativity::NonAssoc => SR::NonAssocError,
        },
    })
}

/// Intenta insertar una acción en la tabla.  Si ya existe, aplica resolución de conflictos.
/// pub so tests/regression_tests.rs (a separate crate) can exercise it directly (see a7_* test).
pub fn insert_action(
    action: &mut HashMap<(usize, String), Action>,
    conflicts: &mut Vec<Conflict>,
    state: usize,
    terminal: String,
    new_action: Action,
    prod_index: &HashMap<(String, Vec<Symbol>), usize>,
    prec_map: &HashMap<String, PrecInfo>,
) {
    let key = (state, terminal.clone());

    if let Some(existing) = action.get(&key) {
        match (existing.clone(), new_action.clone()) {
            // Shift vs Reduce: intentar resolver por precedencia primero
            (Action::Shift(n), Action::Reduce { head, body }) => {
                match resolve_shift_reduce(&terminal, &body, prec_map) {
                    Some(SR::KeepShift)     => {} // shift ya está, sin conflicto
                    Some(SR::DoReduce)      => { action.insert(key, Action::Reduce { head, body }); }
                    Some(SR::NonAssocError) => { action.remove(&key); } // celda de error
                    None => {
                        // Sin info de precedencia → shift gana (estilo yacc), registrar conflicto
                        conflicts.push(Conflict::ShiftReduce {
                            state, terminal, shift_to: n, reduce_with: (head, body),
                        });
                    }
                }
            }
            (Action::Reduce { head, body }, Action::Shift(n)) => {
                match resolve_shift_reduce(&terminal, &body, prec_map) {
                    Some(SR::KeepShift)     => { action.insert(key, Action::Shift(n)); }
                    Some(SR::DoReduce)      => {} // reduce ya está, sin conflicto
                    Some(SR::NonAssocError) => { action.remove(&key); }
                    None => {
                        conflicts.push(Conflict::ShiftReduce {
                            state, terminal, shift_to: n, reduce_with: (head, body),
                        });
                        action.insert(key, Action::Shift(n));
                    }
                }
            }
            // Reduce-Reduce: gana la producción con menor índice
            (Action::Reduce { head: h1, body: b1 }, Action::Reduce { head: h2, body: b2 }) => {
                let idx1 = prod_index.get(&(h1.clone(), b1.clone())).copied().unwrap_or(usize::MAX);
                let idx2 = prod_index.get(&(h2.clone(), b2.clone())).copied().unwrap_or(usize::MAX);
                if idx2 < idx1 {
                    conflicts.push(Conflict::ReduceReduce {
                        state, terminal,
                        kept: (h2.clone(), b2.clone()),
                        discarded: (h1, b1),
                    });
                    action.insert(key, Action::Reduce { head: h2, body: b2 });
                } else {
                    conflicts.push(Conflict::ReduceReduce {
                        state, terminal,
                        kept: (h1, b1),
                        discarded: (h2, b2),
                    });
                }
            }
            // Accept siempre gana
            (Action::Accept, _) => {}
            (_, Action::Accept) => { action.insert(key, Action::Accept); }
            _ => {}
        }
    } else {
        action.insert(key, new_action);
    }
}

/// Construye un índice (head, body) → número de producción para resolución de conflictos.
fn build_production_index(grammar: &Grammar) -> HashMap<(String, Vec<Symbol>), usize> {
    let mut idx = HashMap::new();
    let mut n = 0usize;
    for prod in &grammar.productions {
        for body in &prod.bodies {
            idx.entry((prod.head.clone(), body.clone())).or_insert(n);
            n += 1;
        }
    }
    idx
}

/// Devuelve el número de producción (para imprimir "r3") dado head + body.
fn production_number(grammar: &Grammar, head: &str, body: &[Symbol]) -> usize {
    let mut n = 0usize;
    for prod in &grammar.productions {
        for b in &prod.bodies {
            if prod.head == head && b.as_slice() == body {
                return n;
            }
            n += 1;
        }
    }
    n
}

/// Lista todas las producciones numeradas (para la leyenda de la tabla).
pub fn print_productions(grammar: &Grammar) {
    println!("Producciones numeradas:");
    let mut n = 0usize;
    for prod in &grammar.productions {
        for body in &prod.bodies {
            let body_str = body_to_str(body);
            println!("  r{}: {} → {}", n, prod.head, body_str);
            n += 1;
        }
    }
}

// Silencia el warning de LALRItem importado pero no usado directamente aquí
#[allow(dead_code)]
fn _use_lalr_item(_: &LALRItem) {}
