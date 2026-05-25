// TOP DOWN PARSER

// L -> Left to Right 
// L -> Left most derivation
// 1 ->
use std::collections::HashMap;

use super::grammar::{Grammar, Symbol, Production};
use super::first::{FirstSets, first_of_sequence, EPSILON};
use super::follow::{FollowSets, EOF};
use super::parse_tree::{ParseNode, ParseToken};

// Tabla de parseo M
// La tabla LL(1) es un diccionario de diccionarios.
// M[NoTerminal][Terminal] = Produccion
pub type LL1Table = HashMap<String, HashMap<String, Production>>;

pub struct LL1Parser {
    pub table: LL1Table,
    pub start_symbol: String,
}

impl LL1Parser {
    /// Construye la tabla predictiva LL(1) 
    pub fn build(
        grammar: &Grammar,
        first_sets: &FirstSets, 
        follow_sets: &FollowSets,
    ) -> Result<Self, String> {
        
        let mut table: LL1Table = HashMap::new();

        // Iteramos sobre cada regla de producción en la gramática
        for prod in &grammar.productions {
            let head = &prod.head; // Este es 'A' en la regla A -> alpha

            // Iteramos sobre cada posible cuerpo (alpha) de la producción 'A'
            for body in &prod.bodies {
                
                // Obtenemos FIRST(alpha) usando la función first_of_sequence
                let first_alpha = first_of_sequence(body, first_sets);

                // -------------------------------------------------------------
                // REGLA 1: Para cada terminal 'a' en FIRST(alpha), agregar A -> alpha a M[A, a]
                // -------------------------------------------------------------
                for terminal in &first_alpha {
                    if terminal != EPSILON { 
                        
                        // Intentamos insertar la producción en la tabla
                        let cell = table.entry(head.clone()).or_insert_with(HashMap::new);
                        
                        // Si ya había una producción en esa celda, tenemos un Conflicto LL(1)
                        if cell.contains_key(terminal) {
                            return Err(format!(
                                "Conflicto LL(1) detectado en la tabla M['{}', '{}']. La gramática no es LL(1) (posible ambigüedad o falta factorización).",
                                head, terminal
                            ));
                        }
                        
                        // Insertamos la producción. Guardamos solo este 'body' específico.
                        let single_prod = Production {
                            head: head.clone(),
                            bodies: vec![body.clone()],
                        };
                        cell.insert(terminal.clone(), single_prod);
                    }
                }

                // -------------------------------------------------------------
                // REGLAS 2 y 3: Si epsilon está en FIRST(alpha), 
                // para cada terminal 'b' en FOLLOW(A), agregar A -> alpha a M[A, b]
                // -------------------------------------------------------------
                if first_alpha.contains(EPSILON) {
                    
                    // Obtenemos el FOLLOW(A)
                    let follow_a = follow_sets.get(head).cloned().unwrap_or_default();
                    
                    for terminal_b in follow_a {
                        let cell = table.entry(head.clone()).or_insert_with(HashMap::new);
                        
                        // Verificamos si hay conflicto
                        if cell.contains_key(&terminal_b) {
                             return Err(format!(
                                "Conflicto LL(1) por Epsilon detectado en la tabla M['{}', '{}'].",
                                head, terminal_b
                            ));
                        }

                        // Insertamos la producción (que deriva en epsilon)
                        let single_prod = Production {
                            head: head.clone(),
                            bodies: vec![body.clone()],
                        };
                        cell.insert(terminal_b.clone(), single_prod);
                    }
                }
            }
        }

        Ok(LL1Parser {
            table,
            start_symbol: grammar.start_symbol.clone(),
        })
    }

    /// Realiza el parseo de una lista de tokens
    pub fn parse(&self, tokens: Vec<String>) -> Result<(), String> {
        let mut stack = Vec::new();
        stack.push(EOF.to_string());
        stack.push(self.start_symbol.clone());

        let mut input = tokens;
        input.push(EOF.to_string());
        
        let mut current_token_idx = 0;

        while !stack.is_empty() {
            let top = stack.pop().unwrap();
            let current_token = &input[current_token_idx];

            if top == EOF && *current_token == EOF {
                println!("¡Parseo exitoso!");
                return Ok(());
            }

            // Si el tope es un terminal o EOF
            if !self.table.contains_key(&top) {
                if top == *current_token {
                    current_token_idx += 1;
                } else {
                    return Err(format!("Error de sintaxis: se esperaba '{}', se encontró '{}'", top, current_token));
                }
            } else {
                // El tope es un no-terminal
                if let Some(row) = self.table.get(&top) {
                    if let Some(prod) = row.get(current_token) {
                        // Empujamos el cuerpo de la producción a la pila en orden inverso
                        // Asumimos que prod.bodies[0] es el cuerpo que debemos usar
                        let body = &prod.bodies[0];
                        for symbol in body.iter().rev() {
                            match symbol {
                                Symbol::Terminal(t) => stack.push(t.clone()),
                                Symbol::NonTerminal(nt) => stack.push(nt.clone()),
                            }
                        }
                    } else {
                        return Err(format!("Error de sintaxis: no hay regla en la tabla para [{}, {}]", top, current_token));
                    }
                } else {
                    return Err(format!("Error interno: no se encontró el no-terminal '{}' en la tabla", top));
                }
            }
        }

        if current_token_idx < input.len() - 1 {
            return Err("Error de sintaxis: tokens sobrantes al final de la entrada".to_string());
        }

        Ok(())
    }

    /// Parsea construyendo el árbol de derivación TOP-DOWN.
    /// Usa una "arena" `Vec<NodeRaw>` para evitar problemas de aliasing/borrow:
    /// los nodos viven en un vector y se referencian por índice.
    pub fn parse_tree(&self, tokens: Vec<ParseToken>) -> Result<ParseNode, String> {
        let mut arena: Vec<NodeRaw> = Vec::new();
        // Crear nodo raíz (símbolo inicial, NT)
        arena.push(NodeRaw {
            symbol: self.start_symbol.clone(),
            lexeme: None,
            children: Vec::new(),
        });
        let root_idx = 0usize;

        // La pila lleva pares (símbolo, índice_en_arena). EOF tiene índice usize::MAX
        // (centinela; nunca se materializa como nodo del árbol).
        let mut stack: Vec<(String, usize)> = Vec::new();
        stack.push((EOF.to_string(), usize::MAX));
        stack.push((self.start_symbol.clone(), root_idx));

        let mut input = tokens;
        input.push(ParseToken { kind: EOF.to_string(), lexeme: String::new() });
        let mut ip = 0usize;

        while let Some((top_sym, top_idx)) = stack.pop() {
            let current = &input[ip];

            if top_sym == EOF && current.kind == EOF {
                break;
            }

            // ¿El tope es terminal o EOF? (no tiene fila en la tabla)
            if !self.table.contains_key(&top_sym) {
                if top_sym == current.kind {
                    // Matchea: asignar el lexema al nodo hoja
                    if top_idx != usize::MAX {
                        arena[top_idx].lexeme = Some(current.lexeme.clone());
                    }
                    ip += 1;
                } else {
                    return Err(format!(
                        "Error de sintaxis: se esperaba '{}', se encontró '{}'",
                        top_sym, current.kind
                    ));
                }
            } else {
                // No-terminal: consultar M[top, current.kind]
                let row = self.table.get(&top_sym).unwrap();
                let prod = row.get(&current.kind).ok_or_else(|| format!(
                    "Error de sintaxis: no hay regla en la tabla para [{}, {}]",
                    top_sym, current.kind
                ))?;

                let body = &prod.bodies[0];

                if body.is_empty() {
                    // Producción ε: registrar nodo ε como único hijo
                    let eps_idx = arena.len();
                    arena.push(NodeRaw {
                        symbol: "ε".to_string(),
                        lexeme: None,
                        children: Vec::new(),
                    });
                    arena[top_idx].children.push(eps_idx);
                } else {
                    // Crear nodos hijos en orden y empujarlos en orden inverso
                    let mut child_indices: Vec<usize> = Vec::with_capacity(body.len());
                    for sym in body {
                        let (name, _is_term) = match sym {
                            Symbol::Terminal(t) => (t.clone(), true),
                            Symbol::NonTerminal(nt) => (nt.clone(), false),
                        };
                        let idx = arena.len();
                        arena.push(NodeRaw {
                            symbol: name,
                            lexeme: None,
                            children: Vec::new(),
                        });
                        child_indices.push(idx);
                    }
                    arena[top_idx].children = child_indices.clone();

                    // Push en orden inverso para que el primero del cuerpo quede arriba
                    for sym in body.iter().rev() {
                        let name = match sym {
                            Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.clone(),
                        };
                        let idx = child_indices.pop().unwrap();
                        stack.push((name, idx));
                    }
                }
            }
        }

        // Convertir la arena en un árbol recursivo de ParseNode
        Ok(arena_to_tree(&arena, root_idx))
    }
}

// Nodo en construcción para la arena de `parse_tree`.
// Aislado a nivel de módulo para que el helper de conversión pueda referenciarlo.
struct NodeRaw {
    symbol: String,
    lexeme: Option<String>,
    children: Vec<usize>,
}

pub struct LL1TraceStep {
    pub stack:     Vec<String>,
    pub remaining: Vec<String>,
    pub action:    String,
    pub desc:      String,
}

impl LL1Parser {
    pub fn parse_with_trace(&self, tokens: Vec<String>) -> Vec<LL1TraceStep> {
        let mut stack: Vec<String> = vec![EOF.to_string(), self.start_symbol.clone()];
        let mut input = tokens;
        input.push(EOF.to_string());
        let mut ip    = 0usize;
        let mut trace: Vec<LL1TraceStep> = Vec::new();

        loop {
            let stack_snap  = stack.clone();
            let remaining: Vec<String> = input[ip..].to_vec();

            let top = match stack.last() {
                Some(s) => s.clone(),
                None    => break,
            };
            let current = input[ip].clone();

            if top == EOF && current == EOF {
                trace.push(LL1TraceStep {
                    stack: stack_snap, remaining,
                    action: "acc".to_string(),
                    desc:   "Cadena aceptada".to_string(),
                });
                break;
            }

            if self.table.contains_key(&top) {
                if let Some(row) = self.table.get(&top) {
                    if let Some(prod) = row.get(&current) {
                        let body = &prod.bodies[0];
                        let body_strs: Vec<String> = if body.is_empty() {
                            vec!["ε".to_string()]
                        } else {
                            body.iter().map(|s| match s {
                                Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.clone(),
                            }).collect()
                        };
                        let desc = format!("Predicción: {} → {}", top, body_strs.join(" "));
                        stack.pop();
                        if !body.is_empty() {
                            for sym in body.iter().rev() {
                                let name = match sym {
                                    Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.clone(),
                                };
                                stack.push(name);
                            }
                        }
                        trace.push(LL1TraceStep {
                            stack: stack_snap, remaining,
                            action: "predict".to_string(),
                            desc,
                        });
                    } else {
                        trace.push(LL1TraceStep {
                            stack: stack_snap, remaining,
                            action: "error".to_string(),
                            desc: format!("Error: no hay regla para [{}, {}]", top, current),
                        });
                        break;
                    }
                } else { break; }
            } else {
                if top == current {
                    let desc = format!("Coincidencia: '{}'", top);
                    stack.pop();
                    ip += 1;
                    trace.push(LL1TraceStep {
                        stack: stack_snap, remaining,
                        action: "match".to_string(),
                        desc,
                    });
                } else {
                    trace.push(LL1TraceStep {
                        stack: stack_snap, remaining,
                        action: "error".to_string(),
                        desc: format!("Error: se esperaba '{}', se encontró '{}'", top, current),
                    });
                    break;
                }
            }
        }
        trace
    }
}

fn arena_to_tree(arena: &[NodeRaw], idx: usize) -> ParseNode {
    let n = &arena[idx];
    let children: Vec<ParseNode> = n.children.iter()
        .map(|&ci| arena_to_tree(arena, ci))
        .collect();
    ParseNode {
        symbol: n.symbol.clone(),
        lexeme: n.lexeme.clone(),
        children,
    }
}
