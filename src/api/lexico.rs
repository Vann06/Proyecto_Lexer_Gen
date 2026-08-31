// Fachada del pipeline léxico para la capa API.
//
// La construcción real vive en `lexico::pipeline` — acá solo queda la forma
// que ya esperaban los llamadores de la API (una tupla de tres) más el helper
// de normalización de kind. Antes esta función CONTENÍA el pipeline, y por eso
// `main.rs` y `bin/test_pipeline.rs` tenían cada uno su propia copia en vez de
// poder reutilizarla sin depender de la capa HTTP.
use crate::lexico::pipeline;
use crate::lexico::spec::ast::SpecIR;
use crate::lexico::spec::expand::ExpandedRule;
use crate::lexico::table::transition_table::TransitionTable;
use crate::sintactico::gramatica::grammar::Grammar;

/// Los tres artefactos que necesita la capa API: la spec (por
/// `header`/`trailer`, que usa `codegen::rust_codegen`), las reglas expandidas
/// y la tabla. Firma intacta a propósito — `api::codegen`, `api::pipeline` y
/// los tests de integración la consumen tal cual.
pub fn build_lexer_artifacts(
    yal_src: &str,
) -> Result<(SpecIR, Vec<ExpandedRule>, TransitionTable), String> {
    let a = pipeline::build_all(yal_src)?;
    Ok((a.spec, a.expanded, a.table))
}

pub(crate) fn build_lexer_table_from_str(yal_src: &str) -> Result<TransitionTable, String> {
    pipeline::build_table(yal_src)
}

pub(crate) fn lex_normalize_kind(kind: &str) -> String {
    kind.to_uppercase()
}

/// Delega en `Grammar::ignores_kind`, que es donde vive el dato consultado.
pub(crate) fn lex_is_ignored(kind: &str, grammar: &Grammar) -> bool {
    grammar.ignores_kind(kind)
}
