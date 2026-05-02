// parseo del archivo .yalp y sus delimitadores /* %token %%// src/analizador_sintactico/grammar.rs
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
    pub head: String,               // El lado izquierdo (ej. "production1")
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
        let content = fs::read_to_string(filepath)
            .map_err(|e| format!("Error al leer el archivo: {}", e))?;

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

        Ok(grammar)
    }

    fn parse_tokens_section(&mut self, section: &str) {
        for line in section.lines() {
            let line = line.trim();
            // Ignorar comentarios delimitados por /* y */ (puedes mejorar esto para multilínea)
            if line.starts_with("/*") || line.is_empty() {
                continue;
            }

            if line.starts_with("%token") {
                // Una línea puede tener múltiples tokens separados por espacio
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
        // Separamos por ';' para obtener cada bloque de producción individual
        let prod_blocks = section.split(';');

        for block in prod_blocks {
            let block = block.trim();
            if block.is_empty() || block.starts_with("/*") {
                continue;
            }

            // Separamos la cabeza (head) de las reglas usando ':'
            let parts: Vec<&str> = block.split(':').collect();
            if parts.len() != 2 {
                continue; // O manejar el error de sintaxis
            }

            let head = parts[0].trim().to_string();
            
            // Asignar el símbolo inicial si es la primera producción
            if self.start_symbol.is_empty() {
                self.start_symbol = head.clone();
            }

            let mut bodies = Vec::new();
            // Separamos las diferentes reglas usando el símbolo '|'
            let rules = parts[1].split('|');

            for rule in rules {
                let mut symbol_list = Vec::new();
                for sym_str in rule.split_whitespace() {
                    // Clasificamos: si está en nuestro HashSet de tokens, es Terminal. 
                    // Si no, es un No-Terminal.
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