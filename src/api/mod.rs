// API logic shared by the HTTP server and tests.
//
// Dividido por fase — mismo corte que `src/lexico`/`src/sintactico` — más
// `pipeline` (el único punto que cruza ambas mitades) y `dot` (exporta el
// autómata LR(0) a Graphviz). Este módulo solo conserva los tipos de
// respuesta JSON y reexporta las cuatro funciones públicas; las firmas no
// cambiaron, así que `api::build_pipeline_response` etc. siguen resolviendo
// igual para `src/bin/api.rs` y los tests de integración.
mod codegen;
mod dot;
mod lexico;
mod pipeline;
mod sintactico;

pub use codegen::build_codegen_response;
pub use pipeline::{build_pipeline_response, build_pipeline_response_named};
pub use sintactico::{build_compile_response, build_parse_response};
// Re-exportada porque tests/semantic_analysis_tests.rs la necesita para
// construir un árbol real (parse .yal → expandir → NFA → DFA → tabla) sin
// duplicar ese pipeline una cuarta vez (ya está triplicado entre main.rs,
// test_pipeline.rs y este módulo — ver ORGANIZACION.md § "Fases futuras").
pub use lexico::build_lexer_artifacts;

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Serialize)]
pub struct StateData {
    pub id: usize,
    pub items: Vec<String>,
}

#[derive(Serialize)]
pub struct ProdData {
    pub n: usize,
    pub lhs: String,
    pub rhs: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct ProblemData {
    pub level: String,
    pub code: String,
    pub msg: String,
    pub loc: String,
}

#[derive(Serialize, Default)]
pub struct ParseResponse {
    pub trace: Vec<Value>,
    pub accepted: bool,
    pub error: Option<String>,
    pub problems: Vec<Value>,
    pub token_map: Vec<Value>,
    /// DOT del árbol de derivación REAL (`sintactico::runtime::parse_tree::
    /// ParseNode`, construido con `LRParser::parse_tree`/`LL1Parser::parse_tree`
    /// sobre el `token_map` de esta misma respuesta) — no la reconstrucción por
    /// string-matching que hacía antes el frontend sobre `trace[].desc`. Vacío
    /// si la entrada fue rechazada o el árbol no se pudo construir.
    pub parse_tree_dot: String,
    /// `SymbolTable::dump()` tras correr `semantico::analyzer::analyze` sobre
    /// ese mismo árbol — solo si el `.yalp` trae la directiva `%ident` (ver
    /// `semantico::spec::SemanticSpec::from_grammar`). Vacío si no.
    pub symbol_table: String,
}

#[derive(Serialize)]
pub struct CodegenResponse {
    pub code: String,
    pub n_states: usize,
    pub indent_sensitive: bool,
    pub problems: Vec<ProblemData>,
}

#[derive(Serialize)]
pub struct CompileResponse {
    pub states: Vec<StateData>,
    pub action: HashMap<String, HashMap<String, String>>,
    pub goto: HashMap<String, HashMap<String, usize>>,
    pub terminals: Vec<String>,
    pub non_terminals: Vec<String>,
    pub first: HashMap<String, Vec<String>>,
    pub follow: HashMap<String, Vec<String>>,
    pub prods: Vec<ProdData>,
    pub problems: Vec<ProblemData>,
    pub start_symbol: String,
    pub lr0_dot: String,
}
