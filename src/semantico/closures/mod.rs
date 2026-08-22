// Closures (siguiente fase tras Fase 15): funciones anidadas que referencian
// variables/parámetros del entorno de definición de una función ENCERRADORA
// (no globales, no propios) — "resolución de nombres libres" en la
// terminología del libro del dragón. La detección vive en
// `analyzer::Analyzer` (usa `SymbolTable::lookup_with_scope` para saber en
// qué profundidad se resolvió cada uso); este módulo solo modela y acumula
// el RESULTADO — qué función captura qué variables, y desde dónde — mismo
// espíritu que `errors::ErrorCollector` para los diagnósticos semánticos.
use std::collections::HashMap;

use serde_json::{json, Value};

/// Una variable libre capturada, con la posición del USO que la reveló (no
/// de su declaración — eso ya vive en el `Symbol` original de la tabla).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    pub name: String,
    pub line: usize,
    pub col: usize,
}

/// El conjunto de variables libres que una función anidada captura de algún
/// entorno encerrador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Closure {
    pub function: String,
    pub captures: Vec<Capture>,
}

/// Acumula closures durante el recorrido — un `record_capture` por cada uso
/// de un nombre libre encontrado, deduplicado por nombre dentro de la misma
/// función (una variable capturada se cuenta una vez, sin importar cuántas
/// veces se use en el cuerpo).
#[derive(Debug, Default)]
pub struct ClosureCollector {
    by_function: HashMap<String, Vec<Capture>>,
    // Preserva el orden de PRIMERA aparición de cada función — un HashMap no
    // garantiza orden, y una salida determinista importa para dump()/tests.
    order: Vec<String>,
}

impl ClosureCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_capture(&mut self, function: &str, name: &str, line: usize, col: usize) {
        if !self.by_function.contains_key(function) {
            self.order.push(function.to_string());
        }
        let list = self.by_function.entry(function.to_string()).or_default();
        if !list.iter().any(|c| c.name == name) {
            list.push(Capture { name: name.to_string(), line, col });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.by_function.is_empty()
    }

    /// Closures detectados, en el orden en que sus funciones aparecieron
    /// primero en el recorrido.
    pub fn closures(&self) -> Vec<Closure> {
        self.order
            .iter()
            .map(|f| Closure { function: f.clone(), captures: self.by_function[f].clone() })
            .collect()
    }

    /// Captura de una función puntual, si tiene alguna — para tests y para
    /// consultas puntuales sin pasar por `closures()`.
    pub fn captures_of(&self, function: &str) -> Option<&[Capture]> {
        self.by_function.get(function).map(|v| v.as_slice())
    }

    /// Volcado legible, mismo espíritu que `SymbolTable::dump()`: una
    /// función por línea con sus variables capturadas y de dónde vino cada
    /// uso que las reveló.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for closure in self.closures() {
            out.push_str(&closure.function);
            out.push_str(" captura: ");
            let names: Vec<String> = closure
                .captures
                .iter()
                .map(|c| format!("{} (@{}:{})", c.name, c.line, c.col))
                .collect();
            out.push_str(&names.join(", "));
            out.push('\n');
        }
        out
    }

    /// Forma `[{function, captures:[{name,line,col}]}]` — la que consume
    /// `/api/pipeline` para la pestaña de closures del IDE.
    pub fn to_json(&self) -> Vec<Value> {
        self.closures()
            .into_iter()
            .map(|c| {
                json!({
                    "function": c.function,
                    "captures": c.captures.iter().map(|cap| json!({
                        "name": cap.name,
                        "line": cap.line,
                        "col": cap.col,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_captures_means_empty() {
        let c = ClosureCollector::new();
        assert!(c.is_empty());
        assert!(c.closures().is_empty());
    }

    #[test]
    fn records_a_capture_and_exposes_it() {
        let mut c = ClosureCollector::new();
        c.record_capture("inner", "x", 3, 5);
        assert!(!c.is_empty());
        let caps = c.captures_of("inner").unwrap();
        assert_eq!(caps, &[Capture { name: "x".to_string(), line: 3, col: 5 }]);
    }

    #[test]
    fn repeated_use_of_the_same_free_variable_is_deduplicated() {
        let mut c = ClosureCollector::new();
        c.record_capture("inner", "x", 3, 5);
        c.record_capture("inner", "x", 4, 9); // mismo nombre, otro uso
        assert_eq!(c.captures_of("inner").unwrap().len(), 1, "una variable capturada cuenta una vez");
    }

    #[test]
    fn different_functions_keep_separate_capture_sets() {
        let mut c = ClosureCollector::new();
        c.record_capture("a", "x", 1, 1);
        c.record_capture("b", "y", 2, 1);
        assert_eq!(c.captures_of("a").unwrap().len(), 1);
        assert_eq!(c.captures_of("b").unwrap().len(), 1);
        assert!(c.captures_of("a").unwrap().iter().all(|cap| cap.name == "x"));
    }

    #[test]
    fn closures_preserves_first_seen_function_order() {
        let mut c = ClosureCollector::new();
        c.record_capture("segunda", "y", 2, 1);
        c.record_capture("primera", "x", 1, 1);
        c.record_capture("segunda", "z", 2, 5);
        let closures = c.closures();
        let names: Vec<&str> = closures.iter().map(|cl| cl.function.as_str()).collect();
        assert_eq!(names, vec!["segunda", "primera"], "orden de primera aparición, no alfabético");
    }

    #[test]
    fn to_json_has_the_shape_the_ide_expects() {
        let mut c = ClosureCollector::new();
        c.record_capture("inner", "x", 3, 5);
        let json = c.to_json();
        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["function"], "inner");
        assert_eq!(json[0]["captures"][0]["name"], "x");
        assert_eq!(json[0]["captures"][0]["line"], 3);
        assert_eq!(json[0]["captures"][0]["col"], 5);
    }

    #[test]
    fn dump_lists_one_function_per_line() {
        let mut c = ClosureCollector::new();
        c.record_capture("inner", "x", 3, 5);
        let dump = c.dump();
        assert!(dump.contains("inner captura: x (@3:5)"));
    }
}
