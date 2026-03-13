// minimizar AFD 

use std::collections::{HashMap, HashSet, BTreeSet};
use crate::automata::dfa::{Dfa, DfaState};

/// Fase 10: Algoritmo de minimización de particiones de Hopcroft Modificado
pub fn minimize_dfa(dfa: &Dfa) -> Dfa {
    if dfa.states.is_empty() { return dfa.clone(); }

    // Obtenemos el alfabeto completo (todas las letras de todas las transiciones del AFD actual)
    let mut alphabet = HashSet::new();
    for state in dfa.states.values() {
        for c in state.transitions.keys() {
            alphabet.insert(*c);
        }
    }

    // 1. Partición Inicial. Agruparemos los Minions en un "Diccionario de Prioridades"
    // Usamos el "accept_action" como bandera divisoria.
    let mut partition_map: HashMap<Option<usize>, BTreeSet<usize>> = HashMap::new();
    
    for state in dfa.states.values() {
        // La bandera será: None si no es final. Y Some(ID_REGLA) si es un final ganador de una regla.
        let group_key = state.accept_action.as_ref().map(|(prio, _)| *prio);
        partition_map.entry(group_key).or_insert_with(BTreeSet::new).insert(state.id);
    }

    // Vector maestro de Grupos. Cada grupo se representará por las bolitas que posee.
    let mut partitions: Vec<BTreeSet<usize>> = partition_map.into_values().collect();

    // 2. Refinamiento Activo (¡A separar a los Minions!)
    let mut changed = true;
    while changed {
        changed = false;
        let mut new_partitions = Vec::new();

        for group in &partitions {
            // Un sub-diccionario que va a agruparlos según "A qué Partición Mayor salten leyendo X letra"
            // Signature es como el ADN del estado.
            let mut sub_groups: HashMap<Vec<Option<usize>>, BTreeSet<usize>> = HashMap::new();

            for &state_id in group {
                let state = &dfa.states[&state_id];
                let mut signature = Vec::new();

                for &c in &alphabet {
                    let dest_id = state.transitions.get(&c);
                    // ¿En cuál de nuestras particiones actuales vive esa bolita destino?
                    let dest_group_idx = dest_id.and_then(|id| {
                        partitions.iter().position(|p| p.contains(id))
                    });
                    signature.push(dest_group_idx);
                }

                sub_groups.entry(signature).or_insert_with(BTreeSet::new).insert(state_id);
            }

            // Si el ADN separó el grupo en dos o más pedacitos, significa que hubo división (Changed = true!)
            if sub_groups.len() > 1 {
                changed = true;
            }
            new_partitions.extend(sub_groups.into_values());
        }
        partitions = new_partitions;
    }

    // 3. Crear el AFD Minimizado Final
    let mut min_dfa = Dfa::new(0); // Empezamos a llenar un Tablero en Blanco
    
    // Obtenemos la ID de la super-partición que contiene a nuestro inicio real viejo
    let new_start_id = partitions.iter().position(|p| p.contains(&dfa.start_state)).unwrap();
    min_dfa.start_state = new_start_id;

    for (new_id, group) in partitions.iter().enumerate() {
        let first_minion_id = *group.iter().next().unwrap();
        let old_state = &dfa.states[&first_minion_id]; // Agarramos a un representante del grupo y copiamos su lógica

        let mut next_transitions = HashMap::new();
        // Recableamos las flechas: Del minion destino viejo, buscamos en qué nuevo Subgrupo quedó
        for (c, old_dest_id) in &old_state.transitions {
            let new_dest_id = partitions.iter().position(|p| p.contains(old_dest_id)).unwrap();
            next_transitions.insert(*c, new_dest_id);
        }

        min_dfa.states.insert(
            new_id,
            DfaState {
                id: new_id,
                transitions: next_transitions,
                accept_action: old_state.accept_action.clone(),
            },
        );
    }

    min_dfa
}
