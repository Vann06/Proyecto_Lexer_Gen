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

// --- Clases de caracteres ([...]) ---

/// Un elemento reconocido dentro del contenido crudo de una clase `[...]`:
/// o bien un `'x'` explícitamente citado en el .yal (puede envolver más de un
/// carácter si viene de un escape como `\s`, que se expande a 4 caracteres),
/// o un carácter suelto fuera de comillas (rango sin citar como `0-9`, el
/// operador `-` de un rango citado `'a'-'z'`, o un byte ya decodificado por un
/// escape como `\"`/`\n` que quedó sin comillas propias).
enum ClassAtom {
    Quoted(Vec<char>),
    Bare(char),
}

/// Separa `class_content` (el texto crudo entre `[` y `]`, con los escapes ya
/// decodificados por regex/parser.rs pero las comillas `'x'` todavía intactas)
/// en átomos, distinguiendo qué caracteres estaban DENTRO de una cita `'...'`
/// de los que están sueltos. Sin este paso, un `'` que vino de un escape
/// (`\"` → `"` suelto) es indistinguible de una comilla delimitadora, y un
/// espacio suelto entre dos grupos citados (`'a'-'z' 'A'-'Z'`) es
/// indistinguible de un espacio que el usuario quiso incluir citándolo
/// (`' '`) — ver hallazgos A11 y el bug de espacio-como-miembro-de-clase.
fn tokenize_class_atoms(content: &str) -> Vec<ClassAtom> {
    let mut atoms = Vec::new();
    let mut in_quote = false;
    let mut current: Vec<char> = Vec::new();

    for c in content.chars() {
        if c == '\'' {
            if in_quote {
                atoms.push(ClassAtom::Quoted(std::mem::take(&mut current)));
            }
            in_quote = !in_quote;
        } else if in_quote {
            current.push(c);
        } else {
            atoms.push(ClassAtom::Bare(c));
        }
    }
    // Comilla sin cerrar (.yal malformado): no perder los caracteres acumulados.
    if in_quote && !current.is_empty() {
        atoms.push(ClassAtom::Quoted(current));
    }

    atoms
}

/// Si el átomo representa exactamente un carácter (citado o suelto), lo
/// devuelve — es el único caso válido como extremo de un rango `x-y`.
fn single_char(atom: &ClassAtom) -> Option<char> {
    match atom {
        ClassAtom::Bare(c) => Some(*c),
        ClassAtom::Quoted(v) if v.len() == 1 => Some(v[0]),
        ClassAtom::Quoted(_) => None,
    }
}

/// Traduce el contenido crudo de una clase `[...]` (ya sin los corchetes) a
/// `(negada, miembros_expandidos)`. Reconoce rangos citados (`'a'-'z'`), rangos
/// sueltos (`a-z`), miembros citados sin espacio entre sí (`'a'''.''''`), y
/// escapes ya decodidos que caen sueltos (p. ej. `\"` → `"` suelto en `[^\"\n\r]`).
/// Un espacio SUELTO (fuera de comillas) entre grupos citados es un separador,
/// no un miembro — solo cuenta como miembro si viene citado explícitamente (`' '`).
fn expand_char_class(class_content: &str) -> (bool, Vec<char>) {
    let (is_negated, body) = match class_content.strip_prefix('^') {
        Some(rest) => (true, rest),
        None => (false, class_content),
    };

    let atoms = tokenize_class_atoms(body);
    let mut expanded = Vec::new();
    let mut i = 0;
    while i < atoms.len() {
        // Rango: [endpoint] Bare('-') [endpoint], con ambos extremos de un solo carácter.
        if i + 2 < atoms.len() {
            if let (Some(start), ClassAtom::Bare('-'), Some(end)) =
                (single_char(&atoms[i]), &atoms[i + 1], single_char(&atoms[i + 2]))
            {
                if start <= end {
                    for code in (start as u32)..=(end as u32) {
                        if let Some(ch) = char::from_u32(code) {
                            expanded.push(ch);
                        }
                    }
                }
                i += 3;
                continue;
            }
        }

        match &atoms[i] {
            // Solo el espacio ASCII suelto es separador de estilo entre grupos citados
            // (p. ej. 'a'-'z' 'A'-'Z'). \n/\t/\r sueltos NUNCA son separadores aquí —
            // solo llegan sueltos cuando vinieron de un escape (\n, \t, \r en [^\"\n\r]),
            // y ahí sí son miembros literales de la clase.
            ClassAtom::Bare(' ') => {}
            ClassAtom::Bare(c) => expanded.push(*c),
            ClassAtom::Quoted(v) => expanded.extend(v.iter().copied()),
        }
        i += 1;
    }

    (is_negated, expanded)
}

// ---  Algoritmo de Thompson ---

// Función principal. Recibe el Arbol, y construye la estructura Nfa
pub fn build_nfa_from_ast(ast: &crate::lexico::regex::ast::RegexAst, id_counter: &mut usize) -> Nfa {
    use crate::lexico::regex::ast::RegexAst; // Para no escribir tanto
    
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
            let (is_negated, mut expanded_chars) = expand_char_class(c_string);

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

#[cfg(test)]
mod class_tests {
    use super::expand_char_class;

    fn sorted(mut v: Vec<char>) -> Vec<char> {
        v.sort();
        v.dedup();
        v
    }

    #[test]
    fn space_between_quoted_groups_is_a_separator_not_a_member() {
        // ['a'-'z' 'A'-'Z'] — a bare space between two quoted ranges must NOT become
        // a class member (previously it did, so `letter` also matched a literal ' ').
        let (negated, members) = expand_char_class("'a'-'z' 'A'-'Z'");
        assert!(!negated);
        assert!(!members.contains(&' '), "space leaked into the class: {:?}", members);
        assert!(members.contains(&'a') && members.contains(&'z'));
        assert!(members.contains(&'A') && members.contains(&'Z'));
        assert!(!members.contains(&'0'));
    }

    #[test]
    fn quoted_space_is_still_a_member_when_explicit() {
        // [' ''\t''\n'] — by the time nfa.rs sees this, regex/parser.rs already
        // decoded \t and \n into raw bytes, so the content is three back-to-back
        // quoted single chars: 'X''Y''Z' with X=' ', Y=TAB, Z=NEWLINE.
        let content: String = ['\'', ' ', '\'', '\'', '\t', '\'', '\'', '\n', '\''].iter().collect();
        let (negated, members) = expand_char_class(&content);
        assert!(!negated);
        assert!(members.contains(&' '), "quoted space was dropped: {:?}", members);
        assert!(members.contains(&'\t'));
        assert!(members.contains(&'\n'));
    }

    #[test]
    fn negated_class_keeps_a_quote_char_that_arrived_unquoted_from_an_escape() {
        // [^\"\n\r] — regex/parser.rs already decoded \" to a bare '"' with no
        // wrapping quotes; it must survive as a real excluded member (A11), not be
        // stripped just because it happens to be the same character used to delimit
        // quoted class items.
        let content = "^\"\n\r"; // ^, then a bare '"', newline, CR — as parser.rs would hand it
        let (negated, members) = expand_char_class(content);
        assert!(negated);
        assert!(members.contains(&'"'), "quote character was lost: {:?}", members);
        assert!(members.contains(&'\n'));
        assert!(members.contains(&'\r'));
    }

    #[test]
    fn adjacent_quoted_singles_are_not_merged_into_a_range() {
        // ['+''-'] — two individually-quoted chars, not a range.
        let (negated, members) = expand_char_class("'+''-'");
        assert!(!negated);
        assert_eq!(sorted(members), vec!['+', '-']);
    }

    #[test]
    fn quoted_dash_is_a_literal_not_a_range_operator() {
        // [';''-''_'] — a *quoted* '-' must stay a literal member, never a range dash.
        let (negated, members) = expand_char_class("';''-''_'");
        assert!(!negated);
        assert!(members.contains(&';') && members.contains(&'-') && members.contains(&'_'));
        assert_eq!(members.len(), 3);
    }

    #[test]
    fn bare_unquoted_range_still_works() {
        let (negated, members) = expand_char_class("0-9");
        assert!(!negated);
        assert_eq!(sorted(members), ('0'..='9').collect::<Vec<_>>());
    }
}










