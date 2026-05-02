// logica general para los conjuntos FIRST y FOLLOW.

use std::collections::{HashMap, HashSet};
use super::grammar::{Grammar, Symbol};

pub const EPSILON: &str = "ε";

pub type FirstSets = HashMap<String, HashSet<String>>;

/// Calcula el conjunto FIRST para todos los no-terminales de la gramática.
/// FIRST(A) contiene todos los símbolos terminales que pueden comenzar una cadena
/// derivada del no-terminal A. Si A puede derivar a la cadena vacía, EPSILON pertenece a FIRST(A).
pub fn calculate_first(grammar: &Grammar) -> FirstSets {
    let mut first_sets: FirstSets = HashMap::new();
    let mut changed = true;

    // Inicializar los conjuntos vacíos para cada no-terminal
    for prod in &grammar.productions {
        first_sets.insert(prod.head.clone(), HashSet::new());
    }

    // Iterar hasta que los conjuntos dejen de crecer (Punto Fijo)
    while changed {
        changed = false;

        for prod in &grammar.productions {
            for body in &prod.bodies {
                let head = &prod.head;
                
                // 1. Si el cuerpo está vacío (Epsilon), agregar Epsilon a FIRST(head)
                if body.is_empty() {
                    let set = first_sets.get_mut(head).unwrap();
                    if set.insert(EPSILON.to_string()) {
                        changed = true;
                    }
                    continue;
                }

                let mut all_can_be_epsilon = true;
                
                for symbol in body {
                    match symbol {
                        Symbol::Terminal(t) => {
                            // 2. Si empieza con un Terminal, agregar ese Terminal a FIRST(head) y detener evaluación de este cuerpo
                            let set = first_sets.get_mut(head).unwrap();
                            if set.insert(t.clone()) {
                                changed = true;
                            }
                            all_can_be_epsilon = false;
                            break;
                        }
                        Symbol::NonTerminal(nt) => {
                            // 3. Si es un No-Terminal, agregar su FIRST actual a FIRST(head) (excepto epsilon)
                            let mut has_epsilon = false;
                            let mut to_add = Vec::new();
                            
                            if let Some(nt_first) = first_sets.get(nt) {
                                for f in nt_first {
                                    if f == EPSILON {
                                        has_epsilon = true;
                                    } else {
                                        to_add.push(f.clone());
                                    }
                                }
                            }
                            
                            let set = first_sets.get_mut(head).unwrap();
                            for f in to_add {
                                if set.insert(f) {
                                    changed = true;
                                }
                            }

                            // Si este no-terminal no deriva en epsilon, detenemos la búsqueda en los siguientes símbolos
                            if !has_epsilon {
                                all_can_be_epsilon = false;
                                break; 
                            }
                        }
                    }
                }

                // 4. Si TODOS los símbolos en el cuerpo pueden derivar en epsilon, entonces head también puede
                if all_can_be_epsilon {
                    let set = first_sets.get_mut(head).unwrap();
                    if set.insert(EPSILON.to_string()) {
                        changed = true;
                    }
                }
            }
        }
    }

    first_sets
}



/// Calcula el conjunto FIRST para una secuencia de símbolos 
pub fn first_of_sequence(sequence: &[Symbol], first_sets: &FirstSets) -> HashSet<String> {
    let mut result = HashSet::new();

    if sequence.is_empty() {
        result.insert(EPSILON.to_string());
        return result;
    }

    let mut all_can_be_epsilon = true;

    for symbol in sequence {
        match symbol {
            Symbol::Terminal(t) => {
                result.insert(t.clone());
                all_can_be_epsilon = false;
                break;
            }
            Symbol::NonTerminal(nt) => {
                let mut has_epsilon = false;
                if let Some(nt_first) = first_sets.get(nt) {
                    for f in nt_first {
                        if f == EPSILON {
                            has_epsilon = true;
                        } else {
                            result.insert(f.clone());
                        }
                    }
                }
                
                if !has_epsilon {
                    all_can_be_epsilon = false;
                    break;
                }
            }
        }
    }

    if all_can_be_epsilon {
        result.insert(EPSILON.to_string());
    }

    result
}