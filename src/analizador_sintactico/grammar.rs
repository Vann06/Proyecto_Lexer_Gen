// parseo de archivos .yalp y sus delimitadores /* %token %% // 
use std::collections::HashSet;
use std::fs;

/// Representa cualquier elemento dentro de una regla de la gramática.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Symbol {
    Terminal(String),    // Tokens que vienen del YALex (ej. TOKEN_1, WS)
    NonTerminal(String), // Otras producciones (ej. production1)
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
    pub tokens: HashSet<String>,      // Guardamos los tokens declarados
    pub ignores: HashSet<String>,     // Tokens a ignorar
    pub productions: Vec<Production>, // Lista de todas las producciones
    pub start_symbol: String,         // El no-terminal inicial (la primera producción)
}

impl Grammar {
    /// Lee un archivo YAPar y construye la gramática en memoria.
    pub fn parse_from_file(filepath: &str) -> Result<Self, String> {
        let raw_content = fs::read_to_string(filepath)
            .map_err(|e| format!("Error al leer el archivo: {}", e))?;

        // 0. NUEVO: Limpiamos todos los comentarios /* ... */ del texto antes de parsear
        let content = Self::remove_comments(&raw_content);

        // 1. Dividir el archivo en la sección de tokens y la sección de producciones usando '%%'
        let sections: Vec<&str> = content.split("%%").collect();
        if sections.len() < 2 {
            return Err("El archivo debe contener el separador '%%'".to_string());
        }

        let mut grammar = Grammar {
            tokens: HashSet::new(),
            ignores: HashSet::new(),
            productions: Vec::new(),
            start_symbol: String::new(),
        };

        // 2. Procesar la primera sección (Tokens e Ignores)
        grammar.parse_tokens_section(sections[0]);

        // 3. Procesar la segunda sección (Producciones)
        grammar.parse_productions_section(sections[1]);

        // ─────────────────────────────────────────────────────────────────────────
        // PASO 4 — ELIMINACIÓN DE AMBIGÜEDAD (OBLIGATORIO ANTES DE TODO LO DEMÁS)
        //
        // Este paso DEBE ejecutarse antes de:
        //   • calculate_first()  → FIRST depende de qué terminals pueden iniciar
        //                          cada producción; con recursión izquierda o prefijos
        //                          comunes, los conjuntos serían incorrectos.
        //   • calculate_follow() → FOLLOW depende de FIRST; un FIRST corrompido
        //                          produce conflictos de FOLLOW indetectables.
        //   • LR0Automaton::build() → El cierre de Ítems LR(0) expande producciones
        //                          una por una; con ambigüedad puede generar estados
        //                          duplicados o conflictos shift/reduce imposibles de
        //                          resolver automáticamente.
        //   • SLR/LALR table construction → Una gramática ambigua produce celdas de
        //                          la tabla con múltiples acciones (conflictos), lo
        //                          que hace que el analizador sea no-determinista.
        //
        // Las transformaciones que se aplican son:
        //   1. Eliminación de recursión por la izquierda (directa e indirecta):
        //      A → A α | β   se reescribe a   A → β A'  /  A' → α A' | ε
        //      Necesario para LL(1) (que no puede manejar recursión izquierda)
        //      y conveniente para LR (reduce el tamaño del autómata).
        //   2. Factorización por la izquierda (Left Factoring):
        //      A → α β | α γ  se reescribe a  A → α A'  /  A' → β | γ
        //      Necesario para LL(1): elimina la ambigüedad de qué producción
        //      elegir al ver el primer token del input.
        // ─────────────────────────────────────────────────────────────────────────
        grammar.eliminate_ambiguity();

        // 5. Validar que no haya tokens olvidados actuando como falsos no-terminales
        grammar.validate()?;

        Ok(grammar)
    }

    /// Función auxiliar de limpieza: Borra todo lo que esté entre /* y */
    fn remove_comments(input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();
        let mut in_comment = false;

        while let Some(c) = chars.next() {
            if in_comment {
                if c == '*' {
                    if let Some(&'/') = chars.peek() {
                        chars.next(); // Consumir la '/'
                        in_comment = false;
                    }
                }
            } else {
                if c == '/' {
                    if let Some(&'*') = chars.peek() {
                        chars.next(); // Consumir el '*'
                        in_comment = true;
                        continue;
                    }
                }
                result.push(c);
            }
        }
        result
    }

    /// Aplica transformaciones para eliminar ambigüedad en la gramática antes
    /// de que cualquier algoritmo posterior (FIRST, FOLLOW, LR0, SLR, LALR)
    /// pueda operar sobre ella.
    ///
    /// # Transformaciones aplicadas
    ///
    /// ## 1. Eliminación de recursión por la izquierda (Left Recursion Elimination)
    /// Una producción de la forma `A → A α | β` es **recursiva por la izquierda**.
    /// Esto causa que un parser descendente (LL) entre en un bucle infinito y que
    /// un parser LR genere estados redundantes. Se transforma en:
    /// ```
    ///   A  → β A'
    ///   A' → α A' | ε
    /// ```
    ///
    /// ## 2. Factorización por la izquierda (Left Factoring)
    /// Cuando dos producciones del mismo no-terminal comparten un prefijo común:
    /// `A → α β | α γ`, un parser LL no puede decidir cuál aplicar sin hacer
    /// backtracking. Se factoriza como:
    /// ```
    ///   A  → α A'
    ///   A' → β | γ
    /// ```
    ///
    /// # Por qué AQUÍ y no en otro lugar
    /// Esta función es llamada en `parse_from_file` inmediatamente después de
    /// construir las producciones y ANTES de `validate()`, `calculate_first()`,
    /// `calculate_follow()` y `LR0Automaton::build()`.  
    /// Si se ejecutara después, los algoritmos recibirían una gramática potencialmente
    /// ambigua y producirían resultados silenciosamente incorrectos.
    pub fn eliminate_ambiguity(&mut self) {
        // ── FASE 1: Eliminar recursión izquierda directa ──────────────────────
        // Recorremos cada producción buscando cuerpos que comiencen con su propia cabeza.
        let mut new_productions: Vec<Production> = Vec::new();

        for prod in &self.productions {
            // Separar cuerpos recursivos (A → A α) de los no-recursivos (A → β)
            let mut recursive_bodies: Vec<Vec<Symbol>> = Vec::new();
            let mut non_recursive_bodies: Vec<Vec<Symbol>> = Vec::new();

            for body in &prod.bodies {
                if body.first() == Some(&Symbol::NonTerminal(prod.head.clone())) {
                    // Es recursivo por la izquierda: A → A α
                    // Guardamos sólo el 'α' (el resto tras A)
                    recursive_bodies.push(body[1..].to_vec());
                } else {
                    non_recursive_bodies.push(body.clone());
                }
            }

            if recursive_bodies.is_empty() {
                // No hay recursión izquierda: la producción queda igual
                new_productions.push(prod.clone());
            } else {
                // Creamos el no-terminal auxiliar A'
                let prime_head = format!("{}'", prod.head);

                // Transformar A → β  en  A → β A'
                let transformed_non_recursive: Vec<Vec<Symbol>> = non_recursive_bodies
                    .into_iter()
                    .map(|mut body| {
                        body.push(Symbol::NonTerminal(prime_head.clone()));
                        body
                    })
                    .collect();

                // Si no había cuerpos no-recursivos, la cabeza base puede derivar solo A'
                let base_bodies = if transformed_non_recursive.is_empty() {
                    vec![vec![Symbol::NonTerminal(prime_head.clone())]]
                } else {
                    transformed_non_recursive
                };

                new_productions.push(Production {
                    head: prod.head.clone(),
                    bodies: base_bodies,
                });

                // Construir A' → α A' | ε
                let mut prime_bodies: Vec<Vec<Symbol>> = recursive_bodies
                    .into_iter()
                    .map(|mut alpha| {
                        alpha.push(Symbol::NonTerminal(prime_head.clone()));
                        alpha
                    })
                    .collect();
                // Agregar la producción épsilon para A'
                prime_bodies.push(vec![]); // ε representado como cuerpo vacío

                new_productions.push(Production {
                    head: prime_head,
                    bodies: prime_bodies,
                });
            }
        }

        self.productions = new_productions;

        // ── FASE 2: Factorización por la izquierda ────────────────────────────
        // Se repite hasta que no haya más prefijos comunes que factorizar.
        let mut changed = true;
        let mut counter = 0usize; // Para nombrar no-terminales auxiliares únicos

        while changed {
            changed = false;
            let mut result: Vec<Production> = Vec::new();

            for prod in &self.productions {
                let factored = Self::left_factor_production(prod, &mut counter);
                if factored.len() > 1 {
                    // Se generaron producciones nuevas: hubo factorización
                    changed = true;
                }
                result.extend(factored);
            }

            self.productions = result;
        }
    }

    /// Factoriza por la izquierda UNA producción. Devuelve 1 producción si no
    /// había prefijos comunes, o N producciones si se generaron auxiliares.
    fn left_factor_production(prod: &Production, counter: &mut usize) -> Vec<Production> {
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
                *counter += 1;
                let aux_head = format!("{}_fact{}", prod.head, counter);

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
            } else if line.starts_with("IGNORE") {
                let ignore_decl: Vec<&str> = line[6..].split_whitespace().collect();
                for i in ignore_decl {
                    self.ignores.insert(i.to_string());
                }
            }
        }
    }

    fn parse_productions_section(&mut self, section: &str) {
        let prod_blocks = section.split(';');

        for block in prod_blocks {
            let block = block.trim();
            if block.is_empty() { continue; }

            let parts: Vec<&str> = block.split(':').collect();
            if parts.len() != 2 { continue; }

            let head = parts[0].trim().to_string();
            
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
        }
    }
}
