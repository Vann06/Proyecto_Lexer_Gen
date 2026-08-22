// parseo de archivos .yalp y sus delimitadores /* %token %% //
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fs;

/// Asociatividad de un operador declarada con %left / %right / %nonassoc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Associativity {
    Left,
    Right,
    NonAssoc,
}

/// Representa cualquier elemento dentro de una regla de la gramática.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Symbol {
    Terminal(String),    // Tokens que vienen del YALex (ej. TOKEN_1, WS)
    NonTerminal(String), // Otras producciones (ej. production1)
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Symbol::Terminal(t) | Symbol::NonTerminal(t) => write!(f, "{}", t),
        }
    }
}

/// Convierte un cuerpo de producción (lista de símbolos) en una cadena legible,
/// usando "ε" para el cuerpo vacío. Punto único de esta lógica — antes había una
/// copia casi idéntica en tables.rs, api/mod.rs, y matches inline sueltos en
/// parser_lr.rs, ll1.rs y los binarios test_lalr/test_ll1.
pub fn body_to_string(body: &[Symbol]) -> String {
    if body.is_empty() {
        "ε".to_string()
    } else {
        body.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(" ")
    }
}

/// Renderiza el cuerpo de un ítem LR con el punto en `dot_pos`, p. ej. "α • β".
/// Punto único de esta lógica — antes se repetía casi idéntica en
/// LR1Item::display (lr1.rs), format_lalr_item/format_lr0_item (api/mod.rs) y
/// fmt_item_body (src/bin/test_lalr.rs), cada uno con su propio bucle de "•".
pub fn dotted_body_to_string(body: &[Symbol], dot_pos: usize) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (i, sym) in body.iter().enumerate() {
        if i == dot_pos {
            parts.push("•".to_string());
        }
        parts.push(sym.to_string());
    }
    if dot_pos == body.len() {
        parts.push("•".to_string());
    }
    if parts.is_empty() {
        "•".to_string()
    } else {
        parts.join(" ")
    }
}

/// Representa una producción completa.
/// Ejemplo: production1 : production1 TOKEN_2 | TOKEN_3 ;
#[derive(Debug, Clone)]
pub struct Production {
    pub head: String,               // El lado izquierdo 
    pub bodies: Vec<Vec<Symbol>>,   // El lado derecho, separado por '|'
}

/// Contenedor principal de toda la información del archivo .yalp
#[derive(Debug, Clone)]
pub struct Grammar {
    pub tokens: HashSet<String>,
    pub ignores: HashSet<String>,
    pub productions: Vec<Production>,
    pub start_symbol: String,
    pub transformation_log: Vec<String>,
    pub precedence: Vec<(Associativity, Vec<String>)>,
    /// Directivas opcionales de análisis semántico (Fase 15) — declaradas por
    /// el propio `.yalp`, no hardcodeadas para ninguna gramática concreta:
    ///
    /// ```text
    /// %ident   ID
    /// %declare var_decl    variable
    /// %scope   func_decl   function
    /// ```
    ///
    /// `sintactico` no sabe qué es un `SymbolKind` o un `ScopeKind` — guarda
    /// los nombres tal como aparecen en el archivo y deja que
    /// `semantico::spec::SemanticSpec::from_grammar` los traduzca. Mantiene
    /// la dirección de dependencia semantico → sintactico, nunca al revés.
    pub ident_token: Option<String>,
    /// `(producción, kind)`, uno por línea `%declare`.
    pub declare_directives: Vec<(String, String)>,
    /// `(producción, kind)`, uno por línea `%scope`.
    pub scope_directives: Vec<(String, String)>,
    /// Directivas de clases/objetos (Fase 16), mismo espíritu que las de
    /// arriba: strings tal como aparecen en el `.yalp`, sin que este módulo
    /// sepa nada de `Type`/`SymbolKind`/etc. — `semantico::spec::SemanticSpec
    /// ::from_grammar` las traduce.
    ///
    /// `(producción, símbolo del hijo que es el nodo de tipo)`, uno por línea
    /// `%type_of` — p.ej. `%type_of var_decl tipo`.
    pub type_of_directives: Vec<(String, String)>,
    /// `(token terminal, nombre de tipo)`, uno por línea `%type_token` —
    /// p.ej. `%type_token INT_T integer`.
    pub type_token_directives: Vec<(String, String)>,
    /// Token de `this`, de la única línea `%this` (p.ej. `%this THIS`).
    pub this_token: Option<String>,
    /// `(producción, token de `.`)`, una por línea `%member_access` — puede
    /// haber varias, porque una gramática suele tener más de una producción
    /// con acceso a miembro: la de LECTURA (`primary: primary DOT ID`) y la
    /// de ASIGNACIÓN (`assign_stmt: primary DOT ID ASSIGN expr`).
    pub member_access_directives: Vec<(String, String)>,
    /// `(producción, token `new`, índice del ID de clase, índice de
    /// `arg_list`)`, de la única línea `%new` — p.ej. `%new atom NEW 1 3`.
    pub instantiation: Option<(String, String, usize, usize)>,
    /// `(producción, token de paréntesis de apertura, índice del llamado,
    /// índice de `arg_list`)`, de la única línea `%call` — p.ej.
    /// `%call primary LPAREN 0 2` para
    /// `primary: primary LPAREN arg_list RPAREN`.
    pub call: Option<(String, String, usize, usize)>,
    /// Símbolo de la producción recursiva de lista de argumentos separada
    /// por comas (p.ej. `args` en `args: args COMMA expr | expr`), de la
    /// única línea `%arg_list_symbol`. Lo usan tanto `%new` como `%call`.
    pub arg_list_symbol: Option<String>,
    /// Nombre convencional del método-constructor, de la única línea
    /// `%constructor` (p.ej. `%constructor constructor`).
    pub constructor_name: Option<String>,
}

impl Grammar {
    /// Lee un archivo YAPar y construye la gramática en memoria.
    /// Aplica eliminación de ambigüedad (recursión izquierda + left-factoring).
    /// USA ESTO SOLO para parsers LL(1).
    pub fn parse_from_file(filepath: &str) -> Result<Self, String> {
        let mut grammar = Self::parse_raw(filepath)?;

        // PASO 4 — ELIMINACIÓN DE AMBIGÜEDAD 
      
        grammar.eliminate_ambiguity()?;

        // Verificar que no quede ningún ciclo de recursión izquierda
        grammar.detect_left_recursion()?;

        // Validar que no haya tokens olvidados actuando como falsos no-terminales
        grammar.validate()?;

        Ok(grammar)
    }

    /// Lee un archivo YAPar y construye la gramática en memoria SIN transformaciones.
    /// USA ESTO para parsers LR(0), SLR, LALR — que manejan recursión izquierda
 
    pub fn parse_for_lr(filepath: &str) -> Result<Self, String> {
        let grammar = Self::parse_raw(filepath)?;
        // Solo validamos referencias, sin transformar la gramática
        grammar.validate()?;
        Ok(grammar)
    }

    /// Versión pública que parsea desde un string en memoria (para la API HTTP).
    /// No aplica transformaciones LL(1) — equivalente a parse_for_lr pero sin leer archivo.
    pub fn parse_for_lr_from_str(raw_content: &str) -> Result<Self, String> {
        let grammar = Self::parse_raw_from_str(raw_content)?;
        grammar.validate()?;
        Ok(grammar)
    }

    /// Parsea desde un string en memoria y aplica las transformaciones LL(1)
    /// (eliminación de recursión izquierda + factorización). Para la API HTTP.
    pub fn parse_for_ll1_from_str(raw_content: &str) -> Result<Self, String> {
        let mut grammar = Self::parse_raw_from_str(raw_content)?;
        grammar.eliminate_ambiguity()?;
        grammar.detect_left_recursion()?;
        grammar.validate()?;
        Ok(grammar)
    }

    /// Versión interna que acepta contenido ya leído como string.
    fn parse_raw_from_str(raw_content: &str) -> Result<Self, String> {
        let content = Self::remove_comments(raw_content);

        let sections: Vec<&str> = content.split("%%").collect();
        if sections.len() < 2 {
            return Err("El archivo debe contener el separador '%%'".to_string());
        }

        let mut separator_line = None;
        for (idx, line) in content.lines().enumerate() {
            if line.contains("%%") {
                separator_line = Some(idx + 1);
                break;
            }
        }
        let prod_start_line = separator_line.map(|n| n + 1).unwrap_or(1);

        let mut grammar = Grammar {
            tokens: HashSet::new(),
            ignores: HashSet::new(),
            productions: Vec::new(),
            start_symbol: String::new(),
            transformation_log: Vec::new(),
            precedence: Vec::new(),
            ident_token: None,
            declare_directives: Vec::new(),
            scope_directives: Vec::new(),
            type_of_directives: Vec::new(),
            type_token_directives: Vec::new(),
            this_token: None,
            member_access_directives: Vec::new(),
            instantiation: None,
            call: None,
            arg_list_symbol: None,
            constructor_name: None,
        };

        grammar.parse_tokens_section(sections[0]);
        grammar.parse_productions_section(sections[1], prod_start_line)?;

        Ok(grammar)
    }

    /// Función base interna: lee el archivo y delega en parse_raw_from_str.
    fn parse_raw(filepath: &str) -> Result<Self, String> {
        let raw_content = fs::read_to_string(filepath)
            .map_err(|e| format!("Error al leer el archivo: {}", e))?;
        Self::parse_raw_from_str(&raw_content)
    }

    /// Función auxiliar de limpieza: borra los comentarios `/* ... */` y `// ...`
    /// (el encabezado del módulo documenta ambos, pero solo el primero se
    /// reconocía — una línea `// comentario` en la sección de producciones se
    /// absorbía en el bloque siguiente y podía acabar siendo el `start_symbol`
    /// reportado). Los saltos de línea dentro de un comentario `//` se conservan
    /// para no desalinear la numeración de línea usada en los diagnósticos.
    fn remove_comments(input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();
        let mut in_block_comment = false;
        let mut in_line_comment = false;

        while let Some(c) = chars.next() {
            if in_block_comment {
                if c == '*' {
                    if let Some(&'/') = chars.peek() {
                        chars.next(); // Consumir la '/'
                        in_block_comment = false;
                    }
                }
                continue;
            }
            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                    result.push(c);
                }
                continue;
            }
            if c == '/' {
                if let Some(&'*') = chars.peek() {
                    chars.next(); // Consumir el '*'
                    in_block_comment = true;
                    continue;
                }
                if let Some(&'/') = chars.peek() {
                    chars.next(); // Consumir la segunda '/'
                    in_line_comment = true;
                    continue;
                }
            }
            result.push(c);
        }
        result
    }

    // Funciones auxiliares 

    // collect_used_names: Recolecta todos los nombres de tokens, ignores, no-terminales
    pub(crate) fn collect_used_names(&self) -> HashSet<String> {
        let mut used_names = HashSet::new();

        used_names.extend(self.tokens.iter().cloned());
        used_names.extend(self.ignores.iter().cloned());

        for prod in &self.productions {
            used_names.insert(prod.head.clone());

            for body in &prod.bodies {
                for symbol in body {
                    match symbol {
                        Symbol::Terminal(t) | Symbol::NonTerminal(t) => {
                            used_names.insert(t.clone());
                        }
                    }
                }
            }
        }

        used_names
    }

    /// Genera el símbolo inicial aumentado (`S' -> S`) para construir un autómata
    /// LR: siempre un nombre garantizado fresco (nunca colisiona con ningún token
    /// o no-terminal ya declarado), sin importar cómo se llame `start_symbol`.
    ///
    /// Antes se intentaba adivinar si la gramática de entrada "ya venía
    /// aumentada" mirando si `start_symbol` terminaba en `'` o contenía la
    /// subcadena "prima" — eso confundía identificadores ordinarios en español
    /// (`prima`, `expresion_primaria`) con gramáticas pre-aumentadas y corrompía
    /// la tabla ACTION/GOTO en silencio (ver hallazgo A3). Aumentar siempre,
    /// sin excepciones, elimina la ambigüedad de raíz.
    pub(crate) fn augmented_start_symbol(&self) -> String {
        let used = self.collect_used_names();
        let mut candidate = format!("{}'", self.start_symbol);
        while used.contains(&candidate) {
            candidate.push('\'');
        }
        candidate
    }

    // sanitize_aux_head:
    // Convierte un nombre de cabeza de producción en una forma segura para usar como auxiliar.
    fn sanitize_aux_head(head: &str) -> String {
        let sanitized: String = head
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        if sanitized.is_empty() {
            "NT".to_string()
        } else {
            sanitized
        }
    }

    // Genera un nombre único para un no-terminal auxiliar basado en la cabeza original.
    fn fresh_aux_name(
        head: &str,
        counters: &mut HashMap<String, usize>,
        used_names: &mut HashSet<String>,
    ) -> String {
        let safe_head = Self::sanitize_aux_head(head);
        let counter = counters.entry(safe_head.clone()).or_insert(0);

        loop {
            let candidate = format!("__{}_{}", safe_head, *counter);
            *counter += 1;

            if used_names.insert(candidate.clone()) {
                return candidate;
            }
        }
    }

    /// ## 1. Eliminación de recursión por la izquierda — DIRECTA e INDIRECTA
    /// ## 2. Factorización por la izquierda (Left Factoring)
    pub fn eliminate_ambiguity(&mut self) -> Result<(), String> {
        // ── FASE 1: Eliminación de recursión izquierda (directa + indirecta) ──
        self.transformation_log.clear(); // Limpiar

        let mut used_names = self.collect_used_names();
        let mut aux_counters: HashMap<String, usize> = HashMap::new(); // Para generar nombres únicos de auxiliares 

        self.transformation_log.push(
            "Inicio de limpieza LL(1): eliminación de recursión izquierda y factorización.".to_string()
        );

        let mut ordered: Vec<(String, Vec<Vec<Symbol>>)> = self
            .productions
            .iter()
            .map(|p| (p.head.clone(), p.bodies.clone()))
            .collect();
        let mut i = 0;
        while i < ordered.len() {
            // Paso 1 – Para cada j < i, sustituir Aⱼ al inicio de los cuerpos de Aᵢ
            for j in 0..i {
                let aj_name = ordered[j].0.clone();
                let aj_bodies = ordered[j].1.clone();

                let ai_bodies = ordered[i].1.clone();
                let mut new_ai_bodies: Vec<Vec<Symbol>> = Vec::new();

                for body in &ai_bodies {
                    if body.first() == Some(&Symbol::NonTerminal(aj_name.clone())) {
                        // Aᵢ → Aⱼ γ  →  expandir con las producciones de Aⱼ
                        let gamma = &body[1..]; // sufijo tras Aⱼ
                        for aj_body in &aj_bodies {
                            // δ γ  (δ son los símbolos de la producción de Aⱼ)
                            let mut new_body = aj_body.clone();
                            new_body.extend_from_slice(gamma);
                            new_ai_bodies.push(new_body);
                        }
                    } else {
                        new_ai_bodies.push(body.clone());
                    }
                }
                ordered[i].1 = new_ai_bodies;
            }

            // Paso 2 – Eliminar recursión izquierda DIRECTA de Aᵢ
            let ai_name = ordered[i].0.clone();
            let ai_bodies = ordered[i].1.clone();

            let mut recursive_bodies: Vec<Vec<Symbol>> = Vec::new();
            let mut non_recursive_bodies: Vec<Vec<Symbol>> = Vec::new();

            for body in &ai_bodies {
                if body.first() == Some(&Symbol::NonTerminal(ai_name.clone())) {
                    // A → A α  →  guardar solo α
                    recursive_bodies.push(body[1..].to_vec());
                } else {
                    non_recursive_bodies.push(body.clone());
                }
            }

            if recursive_bodies.is_empty() {
                // Sin recursión directa: queda igual
                ordered[i].1 = non_recursive_bodies;
            } else {
                // Crear A' (prima)
                let prime_head = Self::fresh_aux_name(
                    &ai_name,
                    &mut aux_counters,
                    &mut used_names,
                );

                self.transformation_log.push(format!(
                    "Eliminación de recursión izquierda en '{}': se crea '{}' auxiliar.",
                    ai_name, prime_head
                ));

                // A → β A'
                let transformed_non_recursive: Vec<Vec<Symbol>> = non_recursive_bodies
                    .into_iter()
                    .map(|mut body| {
                        body.push(Symbol::NonTerminal(prime_head.clone()));
                        body
                    })
                    .collect();

                let base_bodies = if transformed_non_recursive.is_empty() {
                    // Sólo había recursión directa sin alternativa no-recursiva
                    vec![vec![Symbol::NonTerminal(prime_head.clone())]]
                } else {
                    transformed_non_recursive
                };

                ordered[i].1 = base_bodies;

                // A' → α A' | ε
                // ε = vec![]  ← representación canónica en todo el sistema
                // Los módulos first.rs y follow.rs reconocen body.is_empty() como ε.
                let mut prime_bodies: Vec<Vec<Symbol>> = recursive_bodies
                    .into_iter()
                    .map(|mut alpha| {
                        alpha.push(Symbol::NonTerminal(prime_head.clone()));
                        alpha
                    })
                    .collect();
                prime_bodies.push(vec![]); // ε
                ordered.insert(
                    i + 1,
                    (prime_head, prime_bodies),
                );
            }
            i += 1; // avanzar el índice del while
        }

        // Reconstruir self.productions preservando el orden de la PRIMERA aparición
        // de cada cabeza, pero FUSIONANDO los cuerpos de cabezas repetidas (puede
        // haberlas tanto si la gramática original ya tenía A' como si el usuario
        // escribió la misma cabeza en dos bloques separados, un estilo yacc válido:
        // 'E : E PLUS T ;' y luego 'E : T ;'). Antes se descartaba el bloque
        // repetido entero en silencio — ver hallazgo A9.
        let mut merged: Vec<(String, Vec<Vec<Symbol>>)> = Vec::new();
        let mut head_index: HashMap<String, usize> = HashMap::new();
        for (head, bodies) in ordered {
            if let Some(&idx) = head_index.get(&head) {
                merged[idx].1.extend(bodies);
            } else {
                head_index.insert(head.clone(), merged.len());
                merged.push((head, bodies));
            }
        }
        self.productions = merged
            .into_iter()
            .map(|(head, bodies)| Production { head, bodies })
            .collect();

        // ── FASE 2: Factorización por la izquierda ────────────────────────────
        // Se repite hasta que no haya más prefijos comunes que factorizar.
        // `left_factor_production` deduplica alternativas idénticas antes de
        // factorizar (ver más abajo) precisamente para que esto converja; esta
        // cota es solo una red de seguridad ante algún caso patológico no
        // previsto — una gramática real nunca necesita más de un puñado de
        // rondas (ver hallazgo A8: antes de deduplicar, 'E : ID | ID ;' entraba
        // en un bucle infinito factorizando AUX -> ε | ε una y otra vez).
        const MAX_LEFT_FACTOR_ROUNDS: usize = 1000;
        let mut changed = true;
        let mut round = 0;

        while changed {
            round += 1;
            if round > MAX_LEFT_FACTOR_ROUNDS {
                return Err(format!(
                    "La factorización por la izquierda no converge después de {} rondas. \
                     Revisa la gramática: probablemente tiene alternativas redundantes o \
                     una estructura recursiva que la factorización no puede resolver.",
                    MAX_LEFT_FACTOR_ROUNDS
                ));
            }

            changed = false;
            let mut result: Vec<Production> = Vec::new();

            for prod in &self.productions {
                let factored = Self::left_factor_production(
                    prod,
                    &mut aux_counters,
                    &mut used_names,
                    &mut self.transformation_log,
                );
                if factored.len() > 1 {
                    // Se generaron producciones nuevas: hubo factorización
                    changed = true;
                }
                result.extend(factored);
            }

            self.productions = result;
        }
        Ok(())
    }

    pub fn detect_left_recursion(&self) -> Result<(), String> {
        // Construir el grafo: para cada NT A, qué NTs B pueden aparecer como
        // primer símbolo de alguno de sus cuerpos (arco A → B).
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for prod in &self.productions {
            let neighbors = graph.entry(prod.head.clone()).or_default();
            for body in &prod.bodies {
                if let Some(Symbol::NonTerminal(nt)) = body.first() {
                    if !neighbors.contains(nt) {
                        neighbors.push(nt.clone());
                    }
                }
            }
        }

        // DFS desde cada nodo buscando ciclos
        for start in graph.keys() {
            let mut visited: HashSet<String> = HashSet::new();
            let mut stack: Vec<(String, Vec<String>)> = Vec::new(); // (nodo, camino)
            stack.push((start.clone(), vec![start.clone()]));

            while let Some((node, path)) = stack.pop() {
                if let Some(neighbors) = graph.get(&node) {
                    for neighbor in neighbors {
                        if neighbor == start {
                            // Ciclo detectado
                            let mut cycle = path.clone();
                            cycle.push(start.clone());
                            return Err(format!(
                                "Recursión izquierda indirecta no eliminable detectada: {}. \
                                 Revisa la gramática; el parser LL(1) entraría en bucle infinito.",
                                cycle.join(" → ")
                            ));
                        }
                        if !visited.contains(neighbor.as_str()) {
                            visited.insert(neighbor.clone());
                            let mut new_path = path.clone();
                            new_path.push(neighbor.clone());
                            stack.push((neighbor.clone(), new_path));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Factoriza por la izquierda UNA producción. Devuelve 1 producción si no
    /// había prefijos comunes, o N producciones si se generaron auxiliares.
    fn left_factor_production(
        prod: &Production,
        aux_counters: &mut HashMap<String, usize>,
        used_names: &mut HashSet<String>,
        transformation_log: &mut Vec<String>,
    ) -> Vec<Production> {
        // Deduplicar alternativas idénticas ANTES de agrupar. Una alternativa
        // repetida ('A : x | x ;') no aporta nada al lenguaje reconocido, pero si
        // se deja pasar a la factorización de abajo, el prefijo común de dos
        // cuerpos idénticos es el cuerpo entero — el sufijo de ambos es ε, así
        // que se genera un auxiliar 'AUX -> ε | ε' con el MISMO problema, que
        // nunca converge (hallazgo A8). Se hace aquí, en cada llamada, para
        // cubrir tanto las producciones originales como los auxiliares que esta
        // misma función genera en rondas posteriores.
        let mut seen_bodies: HashSet<&Vec<Symbol>> = HashSet::new();
        let prod = Production {
            head: prod.head.clone(),
            bodies: prod
                .bodies
                .iter()
                .filter(|body| seen_bodies.insert(body))
                .cloned()
                .collect(),
        };
        let prod = &prod;

        // Agrupar cuerpos por su primer símbolo
        let mut groups: std::collections::BTreeMap<Option<&Symbol>, Vec<&Vec<Symbol>>> =
            std::collections::BTreeMap::new();

        for body in &prod.bodies {
            groups.entry(body.first()).or_default().push(body);
        }

        // Si todos los cuerpos tienen primeros símbolos diferentes, no hay que factorizar
        let has_common_prefix = groups.values().any(|g| g.len() > 1);
        if !has_common_prefix {
            return vec![prod.clone()];
        }

        // Hay al menos un grupo con prefijo común: factorizar
        let mut new_bodies: Vec<Vec<Symbol>> = Vec::new();
        let mut extra_productions: Vec<Production> = Vec::new();

        for (_first_sym, group) in groups {
            if group.len() == 1 {
                // Sólo una producción con ese primer símbolo: no requiere factorizar
                new_bodies.push(group[0].clone());
            } else {
                // Calcular el prefijo común más largo del grupo
                let prefix_len = Self::longest_common_prefix_len(&group);
                let prefix: Vec<Symbol> = group[0][..prefix_len].to_vec();

                // Crear A_factN para los sufijos
                let aux_head = Self::fresh_aux_name(
                    &prod.head,
                    aux_counters,
                    used_names,
                );

                transformation_log.push(format!(
                    "Factorización por la izquierda en '{}'. Prefijo común '{}' movido a auxiliar '{}'.",
                    prod.head,
                    body_to_string(&prefix),
                    aux_head
                ));

                // El cuerpo de A pasa a ser: prefijo A_factN
                let mut new_body = prefix.clone();
                new_body.push(Symbol::NonTerminal(aux_head.clone()));
                new_bodies.push(new_body);

                // Los sufijos van a A_factN
                let suffixes: Vec<Vec<Symbol>> = group
                    .iter()
                    .map(|body| {
                        if body.len() == prefix_len {
                            vec![] // El sufijo es ε
                        } else {
                            body[prefix_len..].to_vec()
                        }
                    })
                    .collect();

                extra_productions.push(Production {
                    head: aux_head,
                    bodies: suffixes,
                });
            }
        }

        let mut all = vec![Production {
            head: prod.head.clone(),
            bodies: new_bodies,
        }];
        all.extend(extra_productions);
        all
    }

    /// Calcula la longitud del prefijo común más largo entre un grupo de cuerpos.
    fn longest_common_prefix_len(group: &[&Vec<Symbol>]) -> usize {
        if group.is_empty() {
            return 0;
        }
        let first = group[0];
        let mut len = 0;
        'outer: for (i, sym) in first.iter().enumerate() {
            for other in &group[1..] {
                if other.get(i) != Some(sym) {
                    break 'outer;
                }
            }
            len = i + 1;
        }
        len
    }

    pub fn validate(&self) -> Result<(), String> {
        // '$' es el centinela de fin de entrada que el driver LR/LL usa
        // internamente (ver EOF en lr1.rs/follow.rs). Si un %token se llamara
        // "$", ACTION[estado, "$"] podría ser un Shift genuino y el driver
        // leería más allá del final del stream de tokens — un panic por índice
        // fuera de rango alcanzable desde HTTP (A5). Se rechaza en el parseo en
        // vez de intentar distinguirlo caso por caso en cada driver.
        if self.tokens.contains("$") {
            return Err(
                "Error crítico de gramática: '$' está reservado como centinela de fin de \
                 entrada y no puede declararse como %token.".to_string()
            );
        }

        let mut valid_non_terminals = HashSet::new();
        for prod in &self.productions {
            valid_non_terminals.insert(prod.head.clone());
        }

        for prod in &self.productions {
            for body in &prod.bodies {
                for symbol in body {
                    if let Symbol::NonTerminal(nt) = symbol {
                        if !valid_non_terminals.contains(nt) {
                            return Err(format!(
                                "Error crítico de gramática: El símbolo '{}' fue tratado como un No-Terminal porque no está en la lista de %token, pero tampoco existe ninguna regla que lo defina.", 
                                nt
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_tokens_section(&mut self, section: &str) {
        for line in section.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }

            if line.starts_with("%token") {
                let tokens_decl: Vec<&str> = line[6..].split_whitespace().collect();
                for t in tokens_decl {
                    self.tokens.insert(t.to_string());
                }
            } else if line.starts_with("%nonassoc") {
                let toks: Vec<String> = line[9..].split_whitespace().map(String::from).collect();
                for t in &toks { self.tokens.insert(t.clone()); }
                self.precedence.push((Associativity::NonAssoc, toks));
            } else if line.starts_with("%left") {
                let toks: Vec<String> = line[5..].split_whitespace().map(String::from).collect();
                for t in &toks { self.tokens.insert(t.clone()); }
                self.precedence.push((Associativity::Left, toks));
            } else if line.starts_with("%right") {
                let toks: Vec<String> = line[6..].split_whitespace().map(String::from).collect();
                for t in &toks { self.tokens.insert(t.clone()); }
                self.precedence.push((Associativity::Right, toks));
            } else if line.starts_with("IGNORE") {
                let ignore_decl: Vec<&str> = line[6..].split_whitespace().collect();
                for i in ignore_decl {
                    self.ignores.insert(i.to_string());
                }
            } else if line.starts_with("%ident") {
                if let Some(tok) = line[6..].split_whitespace().next() {
                    self.ident_token = Some(tok.to_string());
                }
            } else if line.starts_with("%declare") {
                let mut parts = line[8..].split_whitespace();
                if let (Some(production), Some(kind)) = (parts.next(), parts.next()) {
                    self.declare_directives.push((production.to_string(), kind.to_string()));
                }
            } else if line.starts_with("%scope") {
                let mut parts = line[6..].split_whitespace();
                if let (Some(production), Some(kind)) = (parts.next(), parts.next()) {
                    self.scope_directives.push((production.to_string(), kind.to_string()));
                }
            } else if line.starts_with("%type_of") {
                let mut parts = line[8..].split_whitespace();
                if let (Some(production), Some(symbol)) = (parts.next(), parts.next()) {
                    self.type_of_directives.push((production.to_string(), symbol.to_string()));
                }
            } else if line.starts_with("%type_token") {
                let mut parts = line[11..].split_whitespace();
                if let (Some(token), Some(kind)) = (parts.next(), parts.next()) {
                    self.type_token_directives.push((token.to_string(), kind.to_string()));
                }
            } else if line.starts_with("%this") {
                if let Some(tok) = line[5..].split_whitespace().next() {
                    self.this_token = Some(tok.to_string());
                }
            } else if line.starts_with("%member_access") {
                let mut parts = line[14..].split_whitespace();
                if let (Some(production), Some(dot_token)) = (parts.next(), parts.next()) {
                    self.member_access_directives.push((production.to_string(), dot_token.to_string()));
                }
            } else if line.starts_with("%new") {
                let mut parts = line[4..].split_whitespace();
                if let (Some(production), Some(new_token), Some(class_idx), Some(args_idx)) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                {
                    if let (Ok(class_idx), Ok(args_idx)) = (class_idx.parse::<usize>(), args_idx.parse::<usize>()) {
                        self.instantiation = Some((production.to_string(), new_token.to_string(), class_idx, args_idx));
                    }
                }
            } else if line.starts_with("%call") {
                let mut parts = line[5..].split_whitespace();
                if let (Some(production), Some(open_paren), Some(callee_idx), Some(args_idx)) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                {
                    if let (Ok(callee_idx), Ok(args_idx)) = (callee_idx.parse::<usize>(), args_idx.parse::<usize>()) {
                        self.call = Some((production.to_string(), open_paren.to_string(), callee_idx, args_idx));
                    }
                }
            } else if line.starts_with("%arg_list_symbol") {
                if let Some(sym) = line[16..].split_whitespace().next() {
                    self.arg_list_symbol = Some(sym.to_string());
                }
            } else if line.starts_with("%constructor") {
                if let Some(name) = line[12..].split_whitespace().next() {
                    self.constructor_name = Some(name.to_string());
                }
            }
        }
    }

    fn parse_productions_section(&mut self, section: &str, start_line: usize) -> Result<(), String> {
        let mut buf = String::new();
        let mut line = start_line;
        let mut block_start_line = start_line;

        for ch in section.chars() {
            if buf.trim().is_empty() && !ch.is_whitespace() {
                block_start_line = line;
            }
            if ch == '\n' {
                line += 1;
            }
            if ch == ';' {
                let block = buf.trim();
                if !block.is_empty() {
                    self.parse_production_block(block, block_start_line)?;
                }
                buf.clear();
            } else {
                buf.push(ch);
            }
        }

        if !buf.trim().is_empty() {
            return Err(format!(
                "Error en línea {}: falta ';' al final de producción.",
                block_start_line
            ));
        }
        Ok(())
    }

    fn parse_production_block(&mut self, block: &str, line: usize) -> Result<(), String> {
        let parts: Vec<&str> = block.split(':').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Error en línea {}: producción inválida, falta ':'.",
                line
            ));
        }

        let head = parts[0].trim();
        if head.is_empty() {
            return Err(format!(
                "Error en línea {}: cabeza de producción vacía.",
                line
            ));
        }
        let head = head.to_string();

        if self.start_symbol.is_empty() {
            self.start_symbol = head.clone();
        }

        let mut bodies = Vec::new();
        let rules = parts[1].split('|');

        for rule in rules {
            let mut symbol_list = Vec::new();
            for sym_str in rule.split_whitespace() {
                if self.tokens.contains(sym_str) {
                    symbol_list.push(Symbol::Terminal(sym_str.to_string()));
                } else {
                    symbol_list.push(Symbol::NonTerminal(sym_str.to_string()));
                }
            }
            bodies.push(symbol_list);
        }

        self.productions.push(Production { head, bodies });
        Ok(())
    }
}
