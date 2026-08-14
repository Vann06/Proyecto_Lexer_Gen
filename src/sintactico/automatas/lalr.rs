use std::collections::{HashMap, HashSet, BTreeMap, BTreeSet};
use crate::sintactico::gramatica::grammar::Symbol;
use crate::sintactico::automatas::lr1::{LR1Automaton, LR1Item};

/// Clave de core: (head, body, dot_pos) — todo excepto el lookahead.
type ItemCore = (String, Vec<Symbol>, usize);

fn item_core(item: &LR1Item) -> ItemCore {
    (item.head.clone(), item.body.clone(), item.dot_pos)
}

#[derive(Debug, Clone)]
pub struct LALRItem {
    pub head: String,
    pub body: Vec<Symbol>,
    pub dot_pos: usize,
    pub lookaheads: HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct LALRState {
    pub id: usize,
    pub items: Vec<LALRItem>,
    pub origin: Option<(usize, Symbol)>,
    pub lr1_ids: Vec<usize>, // estados LR(1) que conforman este estado LALR
}

#[derive(Debug)]
pub struct LALRAutomaton {
    pub states: Vec<LALRState>,
    pub transitions: HashMap<(usize, Symbol), usize>,
    pub start_head: String,
}

/// Fusiona los estados del autómata LR(1) canónico agrupando por core.
/// Estados con el mismo conjunto de cores (ignorando lookaheads) se unifican
/// en un único estado LALR, uniendo los lookaheads de cada ítem.
pub fn merge_by_core(lr1: LR1Automaton) -> LALRAutomaton {
    // Paso 1 — Calcular el core de cada estado LR(1) y agrupar.
    // Recorrer en orden ascendente de id para asignación determinista.
    let mut core_to_lalr_id: BTreeMap<BTreeSet<ItemCore>, usize> = BTreeMap::new();
    let mut lr1_to_lalr: HashMap<usize, usize> = HashMap::new();
    let mut lalr_count = 0usize;

    let mut states_sorted: Vec<_> = lr1.states.iter().collect();
    states_sorted.sort_by_key(|s| s.id);

    for state in &states_sorted {
        let core: BTreeSet<ItemCore> = state.items.iter().map(item_core).collect();
        let lalr_id = *core_to_lalr_id.entry(core).or_insert_with(|| {
            let id = lalr_count;
            lalr_count += 1;
            id
        });
        lr1_to_lalr.insert(state.id, lalr_id);
    }

    // Paso 2 — Construir cada estado LALR uniendo lookaheads por core.
    // `builders[lalr_id]` acumula: core → HashSet<lookahead>
    let mut builders: Vec<BTreeMap<ItemCore, HashSet<String>>> = vec![BTreeMap::new(); lalr_count];
    let mut origins: Vec<Option<(usize, Symbol)>> = vec![None; lalr_count];
    let mut lr1_members: Vec<Vec<usize>> = vec![Vec::new(); lalr_count];

    for state in &states_sorted {
        let lalr_id = lr1_to_lalr[&state.id];
        lr1_members[lalr_id].push(state.id);

        // El origin del estado LALR viene del primer representante LR(1)
        // (el de menor id) cuyo origin haya sido asignado.
        if origins[lalr_id].is_none() {
            if let Some((from_lr1, sym)) = &state.origin {
                origins[lalr_id] = Some((lr1_to_lalr[from_lr1], sym.clone()));
            }
        }

        for item in &state.items {
            let core = item_core(item);
            builders[lalr_id]
                .entry(core)
                .or_default()
                .insert(item.lookahead.clone());
        }
    }

    // Paso 3 — Materializar los estados LALR.
    let mut lalr_states: Vec<LALRState> = (0..lalr_count)
        .map(|id| {
            let items: Vec<LALRItem> = builders[id]
                .iter()
                .map(|((head, body, dot_pos), lookaheads)| LALRItem {
                    head: head.clone(),
                    body: body.clone(),
                    dot_pos: *dot_pos,
                    lookaheads: lookaheads.clone(),
                })
                .collect();
            LALRState {
                id,
                items,
                origin: origins[id].clone(),
                lr1_ids: lr1_members[id].clone(),
            }
        })
        .collect();

    // Ordenar ítems dentro de cada estado para salida determinista
    for state in &mut lalr_states {
        state.items.sort_by(|a, b| {
            a.head.cmp(&b.head)
                .then_with(|| a.body.cmp(&b.body))
                .then_with(|| a.dot_pos.cmp(&b.dot_pos))
        });
    }

    // Paso 4 — Remapear transiciones LR(1) a LALR.
    // Por teorema clásico: si dos estados LR(1) tienen el mismo core,
    // sus transiciones también van a estados con el mismo core — sin colisiones.
    let mut transitions: HashMap<(usize, Symbol), usize> = HashMap::new();
    for ((from_lr1, sym), to_lr1) in &lr1.transitions {
        let from_lalr = lr1_to_lalr[from_lr1];
        let to_lalr   = lr1_to_lalr[to_lr1];
        transitions.insert((from_lalr, sym.clone()), to_lalr);
    }

    LALRAutomaton {
        states: lalr_states,
        transitions,
        start_head: lr1.start_head,
    }
}
