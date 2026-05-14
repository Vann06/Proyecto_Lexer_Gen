// TOP DOWN PARSER

// L -> Left to Right 
// L -> Left most derivation
// 1 ->
use std::collections::HashMap;

use super::grammar::{Grammar, Symbol, Production};
use super::first::{FirstSets, first_of_sequence, EPSILON};
use super::follow::{FollowSets, EOF};

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
}
