// Lógica para el cálculo de los conjuntos matemáticos FIRST y FOLLOW.

use std::collections::{HashMap, HashSet};
use super::grammar::{Grammar, Symbol};

pub const EPSILON: &str = "ε";
pub const EOF: &str = "$";

pub type FirstSets = HashMap<String, HashSet<String>>;
pub type FollowSets = HashMap<String, HashSet<String>>;

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

/// Calcula el conjunto FIRST para una secuencia de símbolos (útil para calcular FOLLOW y predecir tablas de parsing).
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

/// Calcula el conjunto FOLLOW para todos los no-terminales de la gramática.
/// FOLLOW(A) contiene todos los símbolos terminales que pueden aparecer inmediatamente a la derecha
/// del no-terminal A en alguna forma sentencial.
pub fn calculate_follow(grammar: &Grammar, first_sets: &FirstSets) -> FollowSets {
    let mut follow_sets: FollowSets = HashMap::new();
    let mut changed = true;

    // Inicializar los conjuntos vacíos para cada no-terminal
    for prod in &grammar.productions {
        follow_sets.insert(prod.head.clone(), HashSet::new());
    }

    // Regla 1: Colocar EOF en FOLLOW(S) donde S es el símbolo inicial
    if !grammar.start_symbol.is_empty() {
        if let Some(set) = follow_sets.get_mut(&grammar.start_symbol) {
            set.insert(EOF.to_string());
        }
    }

    // Iterar hasta llegar a un punto fijo
    while changed {
        changed = false;

        for prod in &grammar.productions {
            for body in &prod.bodies {
                let head = &prod.head;

                for (i, symbol) in body.iter().enumerate() {
                    if let Symbol::NonTerminal(nt) = symbol {
                        // Calcular FIRST de la subsecuencia (beta) que sigue a este no-terminal
                        let beta = &body[i + 1..];
                        let beta_first = first_of_sequence(beta, first_sets);

                        let mut has_epsilon = false;
                        let mut to_add = Vec::new();

                        // Regla 2: Todo en FIRST(beta) excepto epsilon se añade a FOLLOW(nt)
                        for f in beta_first {
                            if f == EPSILON {
                                has_epsilon = true;
                            } else {
                                to_add.push(f);
                            }
                        }

                        // Regla 3: Si FIRST(beta) contiene epsilon (o si beta es vacío),
                        // entonces todo lo de FOLLOW(head) se añade a FOLLOW(nt)
                        if has_epsilon || beta.is_empty() {
                            if let Some(head_follow) = follow_sets.get(head) {
                                for f in head_follow {
                                    to_add.push(f.clone());
                                }
                            }
                        }

                        // Insertar lo recolectado en FOLLOW(nt)
                        if let Some(set) = follow_sets.get_mut(nt) {
                            for f in to_add {
                                if set.insert(f) {
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    follow_sets
}