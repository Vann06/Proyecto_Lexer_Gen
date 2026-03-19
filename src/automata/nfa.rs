// Convertir cada AST a un AFN
// un NFA por regla
// luego super_start para unir todos

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)] 

pub enum Transition{
    Literal(char), //Leemos el caracter
    Epsilon, //Transición epsilon
}

#[derive(Debug, Clone)]
pub struct State{
    #[allow(dead_code)]
    pub id: usize, //Identificador único
    pub is_accept: bool, //Ver si es un estado de aceptacion
    pub accept_action: Option<(usize, String)>, //Acción a realizar
    pub transitions: Vec<(Transition, usize)>, //Transiciones
}

impl State {
    pub fn new(id: usize) -> Self
    {
        State{
            id,
            is_accept: false,
            accept_action: None,
            transitions: Vec::new(),
        }
    }
}

//El automata

#[derive(Debug, Clone)]
pub struct Nfa {
    pub states: HashMap<usize, State>, //Mapa de estados
    pub start_state: usize, //El estado inical del automata no determinista
    pub end_state: usize, //El estado final del automata no determinista
}

impl Nfa {
    pub fn new(id_counter: &mut usize) -> Self {
        let start = *id_counter;
        *id_counter += 1;
        let end = *id_counter;
        *id_counter += 1;
        
        let mut states = HashMap::new();
        states.insert(start, State::new(start));
        states.insert(end, State::new(end));
        
        Nfa{
            states,
            start_state: start,
            end_state: end,
        }
    }

    pub fn add_transition(&mut self, from: usize, to: usize, trans: Transition){
        if let Some(state) = self.states.get_mut(&from) {
            state.transitions.push((trans, to));
        }
    }
}

// ---  Algoritmo de Thompson ---

// Función principal. Recibe el Arbol, y construye la estructura Nfa
pub fn build_nfa_from_ast(ast: &crate::regex::ast::RegexAst, id_counter: &mut usize) -> Nfa {
    use crate::regex::ast::RegexAst; // Para no escribir tanto
    
    match ast {
        
        // --- Caso Base 1: Una letra simple 'X' ---
        RegexAst::Literal(c) => {
            let mut nfa = Nfa::new(id_counter); // Crea el tablero de 2 bolitas
            // Trazamos una flecha con 'c' desde el inicio hasta el fin
            nfa.add_transition(nfa.start_state, nfa.end_state, Transition::Literal(*c));
            nfa
        }

        // --- Caso Base 2: Vacío ---
        RegexAst::Empty => {
            let mut nfa = Nfa::new(id_counter); // Crea el tablero
            // Trazamos un pase gratis Epsilon desde el inicio hasta el fin
            nfa.add_transition(nfa.start_state, nfa.end_state, Transition::Epsilon);
            nfa
        }

        // --- Regla 3: Concatenación (Tren AB) ---
        RegexAst::Concat(left, right) => {
            // Evaluamos la rama izquierda y derecha para que se conviertan en autómatas chiquitos primero
            let mut left_nfa = build_nfa_from_ast(left, id_counter);
            let right_nfa = build_nfa_from_ast(right, id_counter);
            // Trazamos el puente: Flecha épsilon desde el final de A, al inicio de B
            left_nfa.add_transition(left_nfa.end_state, right_nfa.start_state, Transition::Epsilon);
            // Mudamos toda la memoria de casillas (estados) para absorber a B dentro de A
            left_nfa.states.extend(right_nfa.states);
            
            // Reasignamos la placa de salida, porque ahora este tren es más largo
            left_nfa.end_state = right_nfa.end_state;
            
            left_nfa
        }

        // --- Regla 4: Unión (A | B) ---
        RegexAst::Union(left, right) => {
            let mut left_nfa = build_nfa_from_ast(left, id_counter);
            let mut right_nfa = build_nfa_from_ast(right, id_counter);
            // Pides un tablero nuevo base que nos dará nuestro propio Start Maestro y End Maestro
            let mut nfa = Nfa::new(id_counter);
            // Trazamos bifurcación desde el start_state nuevo a los inicios de cada lado
            nfa.add_transition(nfa.start_state, left_nfa.start_state, Transition::Epsilon);
            nfa.add_transition(nfa.start_state, right_nfa.start_state, Transition::Epsilon);
            // Rutamos los cabos sueltos de A y de B devuelta a un solo embudo y fin
            left_nfa.add_transition(left_nfa.end_state, nfa.end_state, Transition::Epsilon);
            right_nfa.add_transition(right_nfa.end_state, nfa.end_state, Transition::Epsilon);
            // Absorbemos todas sus casillas y tableritos al nuestro principal
            nfa.states.extend(left_nfa.states);
            nfa.states.extend(right_nfa.states);
            nfa
        }

        // --- Regla 5: Clausura de Kleene (A*) ---
        RegexAst::Star(inner) => {
            let mut inner_nfa = build_nfa_from_ast(inner, id_counter);
            let mut nfa = Nfa::new(id_counter); // Tablero Maestro
            // 1. Escapar sin hacer nada (El CERO del Asterisco): De Start Maestro a End Maestro
            nfa.add_transition(nfa.start_state, nfa.end_state, Transition::Epsilon);
            
            // 2. Entrar al ciclo: De Start Maestro a Start Interno
            nfa.add_transition(nfa.start_state, inner_nfa.start_state, Transition::Epsilon);
            
            // 3. Salir del ciclo: Del End Interno a End Maestro
            inner_nfa.add_transition(inner_nfa.end_state, nfa.end_state, Transition::Epsilon);
            
            // 4. EL LOOP: Del End Interno, regresamos en el tiempo al Start Interno
            inner_nfa.add_transition(inner_nfa.end_state, inner_nfa.start_state, Transition::Epsilon);
            nfa.states.extend(inner_nfa.states);
            nfa
        }

        // --- Regla 6: Plus (A+) ---
        RegexAst::Plus(inner) => {
            let mut inner_nfa = build_nfa_from_ast(inner, id_counter);
            let mut nfa = Nfa::new(id_counter);

            // Entrar al ciclo
            nfa.add_transition(nfa.start_state, inner_nfa.start_state, Transition::Epsilon);
            // Salir del ciclo
            inner_nfa.add_transition(inner_nfa.end_state, nfa.end_state, Transition::Epsilon);
            // Hacer el Loop
            inner_nfa.add_transition(inner_nfa.end_state, inner_nfa.start_state, Transition::Epsilon);

            nfa.states.extend(inner_nfa.states);
            nfa
        }

        // --- Regla 7: Opcional (A?) ---
        RegexAst::Optional(inner) => {
            let mut inner_nfa = build_nfa_from_ast(inner, id_counter);
            let mut nfa = Nfa::new(id_counter);

            // Entrar a la regla
            nfa.add_transition(nfa.start_state, inner_nfa.start_state, Transition::Epsilon);
            // Escapar mágico por si NO quisimos la regla (El Cero del Opcional)
            nfa.add_transition(nfa.start_state, nfa.end_state, Transition::Epsilon);
            
            // Salir de la regla normal
            inner_nfa.add_transition(inner_nfa.end_state, nfa.end_state, Transition::Epsilon);
            
            nfa.states.extend(inner_nfa.states);
            nfa
        }


        // --- Regla 8: Paréntesis de Grupo () ---
        RegexAst::Group(inner) => {
            // El grupo no hace nada matemáticamente más que heredar su interior 
            // porque el parser ya nos armó el árbol con prioridad
            build_nfa_from_ast(inner, id_counter)
        }

        // --- Regla 9: Clase de Caracteres ([a-z] y más) ---
        RegexAst::CharClass(c_string) => {
            // Un 'CharClass' se construye traduciendo su rango a una Unión gigante de Literales
            let clean_string = c_string.replace('\'', "").replace('"', "");
            let chars: Vec<char> = clean_string.chars().collect();
            let mut expanded_chars = Vec::new();

            // Lógica para expandir rangos como "0-9", "a-z", "A-Z", "0-9a-f"
            let mut is_negated = false;
            let mut i = 0;
            if !chars.is_empty() && chars[0] == '^' {
                is_negated = true;
                i = 1;
            }

            while i < chars.len() {
                // Check if we have a range: char - char
                if i + 2 < chars.len() && chars[i+1] == '-' {
                    let start = chars[i];
                    let end = chars[i+2];
                    if start <= end {
                        for code in (start as u32)..=(end as u32) {
                             if let Some(ch) = std::char::from_u32(code) {
                                 expanded_chars.push(ch);
                             }
                        }
                    }
                    i += 3; // Skip start, dash, end
                } else {
                    // Just a literal char
                    expanded_chars.push(chars[i]);
                    i += 1;
                }
            }
            
            if is_negated {
                let mut inverted = Vec::new();
                for code in 9..=126u8 {
                    let ch = code as char;
                    if !expanded_chars.contains(&ch) {
                        inverted.push(ch);
                    }
                }
                expanded_chars = inverted;
            }

            // Si la clase estaba vacía (ej []), se expande a nada (nunca matchea epsilon en lexers reales usualmente)
            // Pero para NFA debe tener inicio y fin.
            if expanded_chars.is_empty() {
                 let nfa = Nfa::new(id_counter);
                 // Un autómata "muerto" que no acepta nada, o epsilon si queremos.
                 // Usualmente [] no matchea nada. Dejemoslo desconectado
                 return nfa;
            }

            let mut nfa = Nfa::new(id_counter);    
            for c in expanded_chars {
                // Creamos un mini-NFA para cada letra y copiamos la topología "Unión"
                let mut char_nfa = Nfa::new(id_counter);
                char_nfa.add_transition(char_nfa.start_state, char_nfa.end_state, Transition::Literal(c));

                nfa.add_transition(nfa.start_state, char_nfa.start_state, Transition::Epsilon);
                char_nfa.add_transition(char_nfa.end_state, nfa.end_state, Transition::Epsilon);
                nfa.states.extend(char_nfa.states);
            }
            nfa
        }
    }
}

pub fn combine_nfas(nfas: Vec<Nfa>, id_counter: &mut usize) -> Nfa {
    let mut super_nfa = Nfa::new(id_counter);
    // Por cada pequeño AFN que recibimos:
    for mut nfa in nfas {
        // Marcamos su final para saber internamente "este era el premio de la regla X"
        if let Some(state) = nfa.states.get_mut(&nfa.end_state) {
            state.is_accept = true; // Este sí es un estado súper ganador!
        }
        
        // 1. Trazamos un pase Epsilon mágico desde el INICIO TOTAL de la app, 
        // hacia la entrada individual de este pequeño AFN (Bifurcación múltiple en paralelo)
        super_nfa.add_transition(super_nfa.start_state, nfa.start_state, Transition::Epsilon);
        
        // 2. Metemos todas casitas que este poseía adentro del NFA gordo (Super NFA)
        super_nfa.states.extend(nfa.states);
    }
    super_nfa
}










