use std::collections::HashMap;

// --- 1. Estado Determinista (Casilla Segura) ---
#[derive(Debug, Clone)]
pub struct DfaState {
    pub id: usize, // Identificador de la nueva "Super Bolita"
    
    // Lista de flechas salientes. Nota: Ahora es un mapa directo: "Si leo X -> voy a ID"
    pub transitions: HashMap<char, usize>, 
    
    // Si la super bolita tiene premio, guardamos [prioridad, acción]
    pub accept_action: Option<(usize, String)>, 
}

// --- 2. Autómata Finito Determinista (El Tablero Compacto) ---
#[derive(Debug, Clone)]
pub struct Dfa {
    pub states: HashMap<usize, DfaState>, // Diccionario de todas las Super Bolitas
    pub start_state: usize,               // Solo 1 entrada (El mega-start inicial)
}

impl Dfa {
    pub fn new(start_state: usize) -> Self {
        Dfa {
            states: HashMap::new(),
            start_state,
        }
    }
}