// parseo de archivos .yalp y sus delimitadores /* %token %% // src/analizador_sintactico/grammar.rs
use std::collections::HashSet;
use std::fs;

/// Representa cualquier elemento dentro de una regla de la gramática.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

        // 4. Validar que no haya tokens olvidados actuando como falsos no-terminales
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
