// Expansión de macros en los patrones de las reglas.
// Las macros pueden tener la forma {nombre} o simplemente nombre,
// y son reemplazadas por la expresión regular correspondiente 
// de la sección "let".
//
// Se expanden las definiciones primero (para que una pueda usar anteriores)
// y luego se expanden en las reglas.

use std::collections::HashMap;

use crate::spec::ast::{Rule, SpecIR};

/// Regla con su patrón ya completamente expandido (sin referencias a macros).
#[derive(Debug, Clone)]
pub struct ExpandedRule {
    pub pattern_expanded: String,
    pub action_code: String,
    pub priority: usize,
}

/// Recorre todas las reglas del SpecIR y expande sus macros.
pub fn expand_definitions(spec: &SpecIR) -> Vec<ExpandedRule> {
    let mut expanded_defs: HashMap<String, String> = HashMap::new();

    // Las definiciones se expanden secuencialmente de arriba a abajo.
    // Una definición solo puede utilizar macros definidas antes que ella.
    for def in &spec.definitions {
        let expanded_regex = expand_string(&def.regex, &expanded_defs);
        expanded_defs.insert(def.name.clone(), expanded_regex);
    }

    spec.rules
        .iter()
        .map(|rule| ExpandedRule {
            pattern_expanded: expand_string(&rule.pattern_raw, &expanded_defs),
            action_code: rule.action_code.clone(),
            priority: rule.priority,
        })
        .collect()
}

/// Expande todas las referencias a macros en un string, respetando 
/// el contexto de comillas simples, dobles y clases de caracteres.
fn expand_string(input: &str, defs: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    
    let mut in_single = false;
    let mut in_double = false;
    let mut in_class = false;
    
    while i < chars.len() {
        let c = chars[i];
        
        if c == '\\' {
            result.push(c);
            i += 1;
            if i < chars.len() {
                result.push(chars[i]);
            }
        } else if c == '\'' && !in_double && !in_class {
            in_single = !in_single;
            result.push(c);
        } else if c == '"' && !in_single && !in_class {
            in_double = !in_double;
            result.push(c);
        } else if c == '[' && !in_single && !in_double {
            in_class = true;
            result.push(c);
        } else if c == ']' && in_class {
            in_class = false;
            result.push(c);
        } else if !in_single && !in_double && !in_class && (c.is_alphabetic() || c == '_') {
            // Recolectar identificador
            let mut id = String::new();
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                id.push(chars[i]);
                i += 1;
            }
            
            // Reemplazar si es una macro conocida
            if let Some(replacement) = defs.get(&id) {
                // Solo insertamos los paréntesis si el reemplazo no los tiene ya a los bordes,
                // O siempre, para simplificar e igualar la lógica original de `final-mod` (que añade paréntesis)
                result.push_str(&format!("({})", replacement));
            } else {
                result.push_str(&id);
            }
            continue; // i ya fue incrementado en el while
        } else {
            // Soporte para formato {macro} (opcional en YALex, pero útil)
            if c == '{' && !in_single && !in_double && !in_class {
                let mut id = String::new();
                let mut j = i + 1;
                while j < chars.len() && chars[j] != '}' {
                    id.push(chars[j]);
                    j += 1;
                }
                if j < chars.len() && chars[j] == '}' {
                    if let Some(replacement) = defs.get(&id) {
                        result.push_str(&format!("({})", replacement));
                        i = j + 1;
                        continue;
                    }
                }
            }
            
            result.push(c);
        }
        
        i += 1;
    }
    
    result
}