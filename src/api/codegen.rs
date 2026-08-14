// Genera el lexer standalone en Rust para un .yal dado, opcionalmente
// enriquecido con lo que declara su .yalp (filtrado de ignorables, soporte
// de indentación) — ver `crate::lexico::codegen::rust_codegen`.
use crate::sintactico::gramatica::grammar::Grammar;
use crate::lexico::runtime::indent;

use super::lexico::build_lexer_artifacts;
use super::{CodegenResponse, ProblemData};

/// Genera el lexer standalone en Rust para el `.yal` dado. `yalp` es
/// opcional (pasar `""` si no hay gramática cargada): sin ella el código se
/// genera igual, solo que sin filtrado de ignorables ni soporte de
/// indentación, porque esas dos cosas dependen de qué declara el `.yalp`
/// (`grammar.ignores` y si NEWLINE+INDENT+DEDENT están declarados — ver
/// `runtime::indent::is_indent_sensitive`).
pub fn build_codegen_response(yal: &str, yalp: &str) -> Result<CodegenResponse, String> {
    let (spec, expanded, table) = build_lexer_artifacts(yal)?;

    let mut opts = crate::lexico::codegen::rust_codegen::CodegenOptions::default();
    let mut problems = Vec::new();
    if !yalp.trim().is_empty() {
        match Grammar::parse_for_lr_from_str(yalp) {
            Ok(grammar) => {
                opts.indent_sensitive = indent::is_indent_sensitive(&grammar.tokens);
                opts.ignored_kinds = grammar.ignores.iter().cloned().collect();
            }
            Err(e) => {
                // El .yal ya generó tabla sin problema; un .yalp inválido no
                // debe tumbar la generación del lexer, solo degrada (sin
                // filtrado de ignorables ni indentación) y se reporta.
                problems.push(ProblemData {
                    level: "warn".to_string(),
                    code: "G001".to_string(),
                    msg: format!("No se pudo leer parser.yalp para codegen: {}", e),
                    loc: "parser.yalp".to_string(),
                });
            }
        }
    }

    let code = crate::lexico::codegen::rust_codegen::emit_string(
        &table,
        &expanded,
        spec.header.as_deref(),
        spec.trailer.as_deref(),
        &opts,
    );

    Ok(CodegenResponse {
        n_states: table.n_states,
        indent_sensitive: opts.indent_sensitive,
        code,
        problems,
    })
}
