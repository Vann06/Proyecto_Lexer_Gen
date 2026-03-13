// Convierte AFN global a AFD con subconjuntos
// ε-closure
// move 

use std::collections::{HashMap, BTreeSet, VecDeque};
use crate::automata::nfa::{Nfa, Transition};
use crate::automata::dfa::{Dfa, DfaState};

// === 1. Algoritmo Epsilon-Closure (Clausura-ε) ===
// "Desde estas bolitas iniciales, ¿a qué otras bolitas puedo llegar mágicamente gratis?"
pub fn epsilon_closure(nfa: &Nfa, start_states: &BTreeSet<usize>) -> BTreeSet<usize> {
    let mut closure = start_states.clone();
    let mut stack: Vec<usize> = closure.iter().copied().collect();

    while let Some(state_id) = stack.pop() {
        if let Some(state) = nfa.states.get(&state_id) {
            for (trans, to_id) in &state.transitions {
                // Si la flecha es un pase mágico gratis (Epsilon)
                if *trans == Transition::Epsilon {
                    if closure.insert(*to_id) {
                        stack.push(*to_id); // Lo analizamos también a ver si ese tiene más pases gratis
                    }
                }
            }
        }
    }
    closure
}

// === 2. Algoritmo Move (Transición real de letras) ===
// "De este conjunto de bolitas en el que estoy, ¿hacia cuáles salto si escucho el caracter 'c'?"
pub fn move_to(nfa: &Nfa, current_states: &BTreeSet<usize>, c: char) -> BTreeSet<usize> {
    let mut result = BTreeSet::new();
    for &state_id in current_states {
        if let Some(state) = nfa.states.get(&state_id) {
            for (trans, to_id) in &state.transitions {
                // Si la flecha me pide pagar el peaje del caracter exacto 'c' que tengo en mano
                if *trans == Transition::Literal(c) {
                    result.insert(*to_id);
                }
            }
        }
    }
    result
}

// === 3. Conversión Maestra: Subconjuntos de AFN a AFD ===
pub fn build_dfa_from_nfa(nfa: &Nfa) -> Dfa {
    let mut next_dfa_id = 0;
    
    // Cada "Super Bolita del AFD" guarda en su panza un costal de IDs del AFN (un subconjunto ordenado BTreeSet)
    let mut dfa_states_map: HashMap<BTreeSet<usize>, usize> = HashMap::new();
    let mut dfa = Dfa::new(next_dfa_id);
    let mut unmarked_states: VecDeque<BTreeSet<usize>> = VecDeque::new();

    // Paso 1: Inicializamos sacándole el epsilon-closure a nuestro Mega-Start (328)
    let mut start_set = BTreeSet::new();
    start_set.insert(nfa.start_state);
    let dfa_start_set = epsilon_closure(nfa, &start_set);

    // Guardamos la primera Súper Bolita: la ID 0
    dfa_states_map.insert(dfa_start_set.clone(), next_dfa_id);
    unmarked_states.push_back(dfa_start_set.clone());
    next_dfa_id += 1;

    // Nuestro Alfabeto Total (Sacamos todas las letrecitas que pedían TODOS nuestros 330 estados viejos)
    let mut alphabet: BTreeSet<char> = BTreeSet::new();
    for state in nfa.states.values() {
        for (trans, _) in &state.transitions {
            if let Transition::Literal(c) = trans {
                alphabet.insert(*c);
            }
        }
    }

    // Paso 2: El Ciclo de Subconjuntos
    while let Some(current_set) = unmarked_states.pop_front() {
        let current_dfa_id = dfa_states_map[&current_set];

        // Revisar si esta "Súper Bolita" actual contiene alguna bolita ganadora del AFN viejo
        let mut best_action: Option<(usize, String)> = None;
        for &nfa_state_id in &current_set {
            if let Some(n_state) = nfa.states.get(&nfa_state_id) {
                if let Some((prio, act)) = &n_state.accept_action {
                    // Si hay conflicto (varios ganan), gana el de menor número de prioridad (ej. Regla 1 > Regla 13)
                    match &best_action {
                        Some((best_prio, _)) if best_prio <= prio => {},
                        _ => best_action = Some((*prio, act.clone())),
                    }
                }
            }
        }

        // Creamos nuestra nueva Super Bolita segura (sin flechas borrosas)
        let mut dfa_state = DfaState {
            id: current_dfa_id,
            transitions: HashMap::new(),
            accept_action: best_action,
        };

        // Paso 3. Por cada letra del abecedario, vemos a qué nueva Súper Bolita brinca
        for &c in &alphabet {
            let move_res = move_to(nfa, &current_set, c);
            if !move_res.is_empty() {
                // Siempre hay que sacar el epsilon-closure del destino a donde saltó
                let dest_set = epsilon_closure(nfa, &move_res);

                let dest_dfa_id = if let Some(&id) = dfa_states_map.get(&dest_set) {
                    id
                } else {
                    // Oh! Encontramos un Grupo de bolitas nunca antes visto. Le damos nueva placa de ID
                    let new_id = next_dfa_id;
                    next_dfa_id += 1;
                    dfa_states_map.insert(dest_set.clone(), new_id);
                    unmarked_states.push_back(dest_set); // Lo metemos a analizar en el futuro ciclo
                    new_id
                };

                // Guardamos el camino en la Super Bolita
                dfa_state.transitions.insert(c, dest_dfa_id);
            }
        }

        // Registramos en el Autómata Determinista final
        dfa.states.insert(current_dfa_id, dfa_state);
    }

    dfa
}
