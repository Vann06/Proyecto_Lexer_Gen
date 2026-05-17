// Autómata LR(1) y tablas ACTION/GOTO
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
    ///
    /// El lookahead b se calcula por estado, no globalmente (ahí está la
    /// diferencia con SLR).
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
                    continue; // terminal tras el punto → sólo hay Shift, no Closure
                };

                // β = sufijo del cuerpo DESPUÉS de B; añadimos el lookahead al final
                // para que FIRST(β Terminal(a)) capture el contexto exacto
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
                                // nunca debería ocurrir (beta_plus_a termina en terminal)
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
    /// ya el formato S' / prima (igual que LR0Automaton::build).
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

            // Ordenar items: kernel primero (dot_pos > 0 o cabeza aumentada), luego alfabético
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

            // Recoger los símbolos que aparecen tras el punto (en orden de aparición)
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

                // ¿Ya existe un estado con exactamente estos ítems?
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

// ─────────────────────────────────────────────────────────────────────────────
// Traza de parseo
// ─────────────────────────────────────────────────────────────────────────────

/// Un paso de la traza LR(1).  `stack_states` y `stack_symbols` se intercalan
/// para formar la pila visual: [s0, sym1, s1, sym2, s2, …]
#[derive(Debug, Clone)]
pub struct TraceStep {
    pub stack_states:  Vec<usize>,
    pub stack_symbols: Vec<String>,
    pub remaining:     Vec<String>,
    pub action:        String,
    pub desc:          String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tablas ACTION / GOTO
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LR1Action {
    Shift(usize),
    Reduce { head: String, body: Vec<Symbol> },
    Accept,
}

impl LR1Action {
    pub fn display(&self) -> String {
        match self {
            LR1Action::Shift(s) => format!("d{}", s),
            LR1Action::Accept => "acc".to_string(),
            LR1Action::Reduce { head, body } => {
                let body_str = if body.is_empty() {
                    "ε".to_string()
                } else {
                    body.iter()
                        .map(|s| match s {
                            Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                };
                format!("r({} -> {})", head, body_str)
            }
        }
    }
}

pub struct LR1Tables {
    pub action: HashMap<(usize, String), LR1Action>,
    pub goto: HashMap<(usize, String), usize>,
    pub conflicts: Vec<String>,
}

impl LR1Tables {
    /// Construye las tablas ACTION y GOTO a partir del autómata LR(1).
    ///
    /// Reglas:
    ///   • [A -> α • a β, _]  y  transición por terminal a → ACTION[s, a] = Shift(dest)
    ///   • [A -> α •, a]  (A ≠ S')               → ACTION[s, a] = Reduce(A -> α)
    ///   • [S' -> S •, $]                          → ACTION[s, $] = Accept
    ///   • Transición por No-Terminal B            → GOTO[s, B]   = dest
    pub fn build(automaton: &LR1Automaton) -> Self {
        let mut action: HashMap<(usize, String), LR1Action> = HashMap::new();
        let mut goto_map: HashMap<(usize, String), usize> = HashMap::new();
        let mut conflicts: Vec<String> = Vec::new();

        // Llenar Shift y GOTO desde las transiciones del autómata
        for ((state_id, symbol), dest) in &automaton.transitions {
            match symbol {
                Symbol::NonTerminal(nt) => {
                    goto_map.insert((*state_id, nt.clone()), *dest);
                }
                Symbol::Terminal(t) => {
                    let key = (*state_id, t.clone());
                    Self::try_insert(&mut action, key, LR1Action::Shift(*dest), &mut conflicts);
                }
            }
        }

        // Llenar Reduce y Accept desde ítems con el punto al final
        for state in &automaton.states {
            for item in &state.items {
                if !item.is_reduce_item() {
                    continue;
                }
                let key = (state.id, item.lookahead.clone());
                let new_action = if item.head == automaton.start_head && item.lookahead == EOF {
                    LR1Action::Accept
                } else {
                    LR1Action::Reduce {
                        head: item.head.clone(),
                        body: item.body.clone(),
                    }
                };
                Self::try_insert(&mut action, key, new_action, &mut conflicts);
            }
        }

        LR1Tables { action, goto: goto_map, conflicts }
    }

    fn try_insert(
        action: &mut HashMap<(usize, String), LR1Action>,
        key: (usize, String),
        new_action: LR1Action,
        conflicts: &mut Vec<String>,
    ) {
        if let Some(existing) = action.get(&key) {
            if *existing != new_action {
                conflicts.push(format!(
                    "Conflicto en estado {} con '{}': {} vs {}",
                    key.0,
                    key.1,
                    existing.display(),
                    new_action.display()
                ));
            }
        } else {
            action.insert(key, new_action);
        }
    }

    /// Traza completa del parseo: registra cada paso con la pila visual,
    /// el input restante, la acción tomada y una descripción legible.
    /// La pila visual intercala estados y símbolos: [s0, sym, s1, sym, s2, …]
    pub fn parse_with_trace(&self, tokens: Vec<String>) -> Vec<TraceStep> {
        let mut state_stack: Vec<usize>  = vec![0];
        let mut sym_stack:   Vec<String> = Vec::new();
        let mut input: Vec<String> = tokens;
        input.push(EOF.to_string());
        let mut idx = 0;
        let mut steps: Vec<TraceStep> = Vec::new();

        loop {
            let top   = *state_stack.last().unwrap();
            let token = input[idx].clone();
            let remaining = input[idx..].to_vec();

            match self.action.get(&(top, token.clone())) {
                Some(LR1Action::Shift(next)) => {
                    steps.push(TraceStep {
                        stack_states:  state_stack.clone(),
                        stack_symbols: sym_stack.clone(),
                        remaining,
                        action: format!("s{}", next),
                        desc:   format!("Estado {}, símbolo '{}' → Shift a I{}", top, token, next),
                    });
                    state_stack.push(*next);
                    sym_stack.push(token);
                    idx += 1;
                }
                Some(LR1Action::Reduce { head, body }) => {
                    let body_str = if body.is_empty() {
                        "ε".to_string()
                    } else {
                        body.iter().map(|s| match s {
                            Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
                        }).collect::<Vec<_>>().join(" ")
                    };
                    let n = body.len();
                    let goto_from = state_stack
                        .get(state_stack.len().saturating_sub(n + 1))
                        .copied()
                        .unwrap_or(0);
                    let goto_dest = self.goto
                        .get(&(goto_from, head.clone()))
                        .copied()
                        .unwrap_or(0);
                    steps.push(TraceStep {
                        stack_states:  state_stack.clone(),
                        stack_symbols: sym_stack.clone(),
                        remaining,
                        action: format!("r({} → {})", head, body_str),
                        desc:   format!(
                            "Estado {}, ver '{}' → Reduce ({} → {}), GOTO({},{})={}",
                            top, token, head, body_str, goto_from, head, goto_dest
                        ),
                    });
                    for _ in 0..n {
                        state_stack.pop();
                        sym_stack.pop();
                    }
                    let top_after = *state_stack.last().unwrap();
                    match self.goto.get(&(top_after, head.clone())) {
                        Some(next_state) => {
                            state_stack.push(*next_state);
                            sym_stack.push(head.clone());
                        }
                        None => return steps,
                    }
                }
                Some(LR1Action::Accept) => {
                    steps.push(TraceStep {
                        stack_states:  state_stack.clone(),
                        stack_symbols: sym_stack.clone(),
                        remaining,
                        action: "acc".to_string(),
                        desc:   format!("Estado {}, ver '$' → ACEPTAR ✓", top),
                    });
                    return steps;
                }
                None => {
                    steps.push(TraceStep {
                        stack_states:  state_stack.clone(),
                        stack_symbols: sym_stack.clone(),
                        remaining,
                        action: "error".to_string(),
                        desc:   format!("Error de sintaxis en estado {} con token '{}'", top, token),
                    });
                    return steps;
                }
            }
        }
    }

    /// Simulación del parser LR(1) usando pila de estados.
    ///
    /// Algoritmo:
    ///   1. Leer token actual
    ///   2. ACTION[tope, token] = Shift(s)  → apilar s, avanzar input
    ///   3. ACTION[tope, token] = Reduce(A->α) → desapilar |α|, GOTO[tope, A] → apilar
    ///   4. ACTION[tope, token] = Accept → éxito
    ///   5. Sin entrada → error de sintaxis
    pub fn parse(&self, tokens: Vec<String>) -> Result<(), String> {
        let mut stack: Vec<usize> = vec![0];
        let mut input: Vec<String> = tokens;
        input.push(EOF.to_string());
        let mut idx = 0;

        loop {
            let top = *stack.last().unwrap();
            let token = input[idx].clone();

            match self.action.get(&(top, token.clone())) {
                Some(LR1Action::Shift(next)) => {
                    stack.push(*next);
                    idx += 1;
                }
                Some(LR1Action::Reduce { head, body }) => {
                    let n = body.len();
                    for _ in 0..n {
                        stack.pop();
                    }
                    let top_after = *stack.last().unwrap();
                    match self.goto.get(&(top_after, head.clone())) {
                        Some(next_state) => stack.push(*next_state),
                        None => {
                            return Err(format!(
                                "Error interno: no hay GOTO[{}, {}]",
                                top_after, head
                            ))
                        }
                    }
                }
                Some(LR1Action::Accept) => return Ok(()),
                None => {
                    return Err(format!(
                        "Error de sintaxis en estado {} con token '{}'",
                        top, token
                    ))
                }
            }
        }
    }
}
