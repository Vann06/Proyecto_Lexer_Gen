// Driver shift-reduce GENÉRICO para cualquier parser LR.
//
// Esta implementación es independiente del método de construcción de la tabla:
// funciona idéntico para LALR(1) (actual), SLR(1), LR(0) y LR(1) canónico.
// Lo único que cambia entre variantes es CÓMO se construye `LRTable`; el
// algoritmo de parseo (shift / reduce / accept) es el mismo.

use super::grammar::Symbol;
use super::parse_tree::{ParseNode, ParseToken};
use super::tables::{Action, LRTable};

#[derive(Debug, Clone)]
pub enum ParseStep {
    Shift { state: usize, token: String },
    Reduce { head: String, body: Vec<Symbol> },
    Accept,
}

pub struct LRParser<'a> {
    pub table: &'a LRTable,
}

impl<'a> LRParser<'a> {
    pub fn new(table: &'a LRTable) -> Self {
        LRParser { table }
    }

    /// Ejecuta shift-reduce. `tokens` debe ser una lista de terminales SIN el $ final.
    /// Devuelve la traza de pasos o un mensaje de error sintáctico.
    pub fn parse(&self, tokens: Vec<String>) -> Result<Vec<ParseStep>, String> {
        let mut state_stack: Vec<usize> = vec![self.table.start_state];
        let mut symbol_stack: Vec<Symbol> = Vec::new();

        let mut input: Vec<String> = tokens;
        input.push("$".to_string());
        let mut ip = 0usize;

        let mut trace: Vec<ParseStep> = Vec::new();

        loop {
            let s = *state_stack.last().unwrap();
            let a = &input[ip];

            match self.table.action.get(&(s, a.clone())) {
                Some(Action::Shift(t)) => {
                    let t = *t;
                    trace.push(ParseStep::Shift { state: t, token: a.clone() });
                    state_stack.push(t);
                    symbol_stack.push(Symbol::Terminal(a.clone()));
                    ip += 1;
                }
                Some(Action::Reduce { head, body }) => {
                    let head = head.clone();
                    let body = body.clone();

                    trace.push(ParseStep::Reduce { head: head.clone(), body: body.clone() });

                    for _ in 0..body.len() {
                        state_stack.pop();
                        symbol_stack.pop();
                    }

                    let top = *state_stack.last().unwrap();
                    let next_state = self.table.goto.get(&(top, head.clone())).copied()
                        .ok_or_else(|| format!(
                            "Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head
                        ))?;

                    state_stack.push(next_state);
                    symbol_stack.push(Symbol::NonTerminal(head));
                }
                Some(Action::Accept) => {
                    trace.push(ParseStep::Accept);
                    return Ok(trace);
                }
                None => {
                    return Err(format_error(s, a, &self.table));
                }
            }
        }
    }

    /// Ejecuta shift-reduce y construye el árbol de derivación BOTTOM-UP.
    /// Mantiene una pila paralela de `ParseNode`:
    ///   - Shift token t        → push hoja(t)
    ///   - Reduce A → α (|α|=k) → pop k nodos, push interno(A, esos k nodos)
    ///   - Accept               → la pila contiene la raíz
    pub fn parse_tree(&self, tokens: Vec<ParseToken>) -> Result<ParseNode, String> {
        let mut state_stack: Vec<usize> = vec![self.table.start_state];
        let mut node_stack: Vec<ParseNode> = Vec::new();

        let mut input = tokens;
        // Centinela $ con lexema vacío (no aparece en el árbol porque Accept no lo consume).
        input.push(ParseToken { kind: "$".to_string(), lexeme: String::new() });
        let mut ip = 0usize;

        loop {
            let s = *state_stack.last().unwrap();
            let a = &input[ip].kind;

            match self.table.action.get(&(s, a.clone())) {
                Some(Action::Shift(t)) => {
                    let t = *t;
                    node_stack.push(ParseNode::leaf(&input[ip]));
                    state_stack.push(t);
                    ip += 1;
                }
                Some(Action::Reduce { head, body }) => {
                    let head = head.clone();
                    let k = body.len();

                    // Pop k nodos hijos en el orden en que están en el cuerpo.
                    let split_at = node_stack.len().saturating_sub(k);
                    let children: Vec<ParseNode> = node_stack.drain(split_at..).collect();

                    for _ in 0..k {
                        state_stack.pop();
                    }

                    let top = *state_stack.last().unwrap();
                    let next_state = self.table.goto.get(&(top, head.clone())).copied()
                        .ok_or_else(|| format!(
                            "Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head
                        ))?;

                    // Si el cuerpo era ε, añadir un nodo ε visible en el árbol.
                    let children = if children.is_empty() {
                        vec![ParseNode::epsilon_leaf()]
                    } else {
                        children
                    };

                    node_stack.push(ParseNode::internal(head, children));
                    state_stack.push(next_state);
                }
                Some(Action::Accept) => {
                    // La pila debe contener exactamente el árbol del símbolo inicial.
                    return node_stack.pop()
                        .ok_or_else(|| "Error interno: Accept con pila de nodos vacía.".to_string());
                }
                None => {
                    return Err(format_error(s, a, &self.table));
                }
            }
        }
    }
}

fn format_error(state: usize, token: &str, table: &LRTable) -> String {
    let expected: Vec<String> = table.action.keys()
        .filter(|(st, _)| *st == state)
        .map(|(_, t)| format!("'{}'", t))
        .collect();
    let expected_str = if expected.is_empty() {
        "ninguno (estado de error)".to_string()
    } else {
        expected.join(", ")
    };
    format!("Error sintáctico: estado I{}, token '{}'. Esperado: {}",
            state, token, expected_str)
}

/// Imprime la traza del parseo en formato columnar.
pub fn print_trace(trace: &[ParseStep]) {
    println!("{:<40} Acción", "Pila de estados");
    println!("{}", "-".repeat(60));
    for step in trace {
        match step {
            ParseStep::Shift { state, token } => {
                println!("  push I{}  ←  shift '{}'", state, token);
            }
            ParseStep::Reduce { head, body } => {
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
                println!("  reduce   :  {} → {}", head, body_str);
            }
            ParseStep::Accept => {
                println!("  accept   ✓");
            }
        }
    }
}
