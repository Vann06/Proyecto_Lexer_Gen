use std::collections::{HashMap, HashSet};
use super::grammar::{Grammar, Symbol};
use super::first::{FirstSets, EPSILON}; // Importamos FirstSets y EPSILON
use crate::analizador_sintactico::first::first_of_sequence;

pub const EOF: &str = "$";
pub type FollowSets = HashMap<String, HashSet<String>>;



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