//! Pipeline léxico completo: `.yal` → tabla de transición, en un solo lugar.
//!
//! Antes esta cadena estaba escrita TRES veces —`api::lexico`, `main.rs` y
//! `bin/test_pipeline.rs`—, palabra por palabra y con los mismos nombres de
//! variables; solo cambiaba el manejo de errores y lo que cada llamador
//! imprimía por el camino. Cualquier corrección a una de las tres se quedaba
//! en esa copia.
//!
//! Vive en `lexico` y no en `api` a propósito: es lógica de la fase léxica, y
//! un binario de línea de comandos no debería tener que depender de la capa
//! HTTP para construir un lexer.
//!
//! La clave para poder unificar sin perder nada es devolver **todos** los
//! artefactos intermedios, no solo la tabla: `main.rs` existe justamente para
//! mostrar cada fase (imprime el número de estados del AFD antes y después de
//! minimizar, y grafica el AST consolidado y el autómata), así que necesita
//! los pasos intermedios que un `build → tabla` descartaría. Quien solo
//! quiera la tabla ignora el resto.

use crate::lexico::automata::dfa::Dfa;
use crate::lexico::automata::minimize::minimize_dfa;
use crate::lexico::automata::nfa::{build_nfa_from_ast, combine_nfas, Nfa};
use crate::lexico::automata::subset::build_dfa_from_nfa;
use crate::lexico::regex::ast::RegexAst;
use crate::lexico::regex::parser::parse_regex;
use crate::lexico::spec::ast::SpecIR;
use crate::lexico::spec::expand::{expand_definitions, ExpandedRule};
use crate::lexico::spec::parser::parse_yalex;
use crate::lexico::table::transition_table::{self, TransitionTable};

/// Todo lo que produce el pipeline léxico, en el orden en que se produce.
pub struct LexerArtifacts {
    /// La especificación tal como se parseó (cabecera, definiciones, reglas,
    /// tráiler). `codegen::rust_codegen` necesita `header`/`trailer`.
    pub spec: SpecIR,
    /// Las reglas con sus macros ya sustituidas.
    pub expanded: Vec<ExpandedRule>,
    /// El AST de cada regla, en el mismo orden que `expanded` — `main.rs` los
    /// une en un AST consolidado para graficarlo.
    pub asts: Vec<RegexAst>,
    /// El AFN maestro: la unión de un autómata por regla.
    pub master_nfa: Nfa,
    /// El AFD antes de minimizar — se conserva para poder comparar su número
    /// de estados contra el minimizado.
    pub dfa: Dfa,
    /// El AFD ya minimizado, que es el que se vuelca a la tabla.
    pub min_dfa: Dfa,
    pub table: TransitionTable,
}

/// Corre el pipeline entero y devuelve todos sus artefactos.
///
/// Los errores se devuelven como `Result` en vez de abortar el proceso: los
/// llamadores de consola deciden si imprimir y salir, y el servidor los
/// convierte en una respuesta HTTP. Antes cada copia decidía eso por su
/// cuenta, y por eso la de `test_pipeline.rs` llamaba a `process::exit` desde
/// dentro de la construcción.
pub fn build_all(yal_src: &str) -> Result<LexerArtifacts, String> {
    let spec = parse_yalex(yal_src).map_err(|e| format!("Error al parsear .yal: {}", e))?;
    let expanded = expand_definitions(&spec);

    let mut id_counter = 0usize;
    let mut asts = Vec::with_capacity(expanded.len());
    let mut nfas = Vec::with_capacity(expanded.len());
    for rule in &expanded {
        let ast = parse_regex(&rule.pattern_expanded)
            .map_err(|e| format!("Error en regex '{}': {}", rule.pattern_expanded, e))?;
        let mut nfa = build_nfa_from_ast(&ast, &mut id_counter);
        // El estado final de cada AFN lleva la acción y la prioridad de SU
        // regla: es lo que permite que, tras unirlos todos, el simulador sepa
        // qué token reconoció y cuál gana cuando dos reglas empatan.
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        asts.push(ast);
        nfas.push(nfa);
    }

    let master_nfa = combine_nfas(nfas, &mut id_counter);
    let dfa = build_dfa_from_nfa(&master_nfa);
    let min_dfa = minimize_dfa(&dfa);
    let table = transition_table::build(&min_dfa);

    Ok(LexerArtifacts { spec, expanded, asts, master_nfa, dfa, min_dfa, table })
}

/// Atajo para quien solo necesita la tabla.
pub fn build_table(yal_src: &str) -> Result<TransitionTable, String> {
    build_all(yal_src).map(|a| a.table)
}
