// El único punto de la API que cruza las dos mitades: tokeniza con el lexer
// (léxico) y alimenta ese stream al parser (sintáctico) en una sola pasada,
// devolviendo un ParseResponse con line/col adjunta para que el frontend
// resalte la línea fuente en cada paso del trace.
use crate::sintactico::gramatica::grammar::Grammar;
use crate::lexico::runtime::indent;
use crate::lexico::runtime::simulator::{LexResult, Simulator};
use crate::semantico::analyzer::analyze;
use crate::semantico::spec::SemanticSpec;
use crate::sintactico::runtime::parse_tree::{to_dot, to_dot_annotated};
use serde_json::{json, Value};

use super::lexico::{build_lexer_table_from_str, lex_is_ignored, lex_normalize_kind};
use super::sintactico::{
    build_lr_table, build_parse_response, build_parse_response_with, build_real_parse_tree,
    build_real_parse_tree_with, collect_lr_syntax_errors, collect_lr_syntax_errors_with,
    push_syntax_problem,
};
use super::ParseResponse;

/// Punto de entrada usado por los binarios/tests existentes: mismo contrato
/// que antes, con `source_name` fijo en `"input.txt"` (el nombre que el
/// workspace por defecto siembra) — así ningún llamador previo tiene que
/// cambiar. `/api/pipeline` (bin/api.rs) usa la variante `_named` de abajo con
/// el nombre real del archivo cargado en el IDE.
pub fn build_pipeline_response(
    yal: &str,
    yalp: &str,
    source: &str,
    mode: &str,
) -> Result<ParseResponse, String> {
    build_pipeline_response_named(yal, yalp, source, mode, "input.txt")
}

/// Igual que `build_pipeline_response`, pero con el nombre real del archivo
/// fuente — se usa para ubicar cada diagnóstico (`loc: "{source_name}:L:C"`)
/// en vez del `"input.txt"` fijo que antes rompía el resaltado del gutter en
/// cuanto el archivo cargado no se llamaba así (p. ej. un `.cps`).
pub fn build_pipeline_response_named(
    yal: &str,
    yalp: &str,
    source: &str,
    mode: &str,
    source_name: &str,
) -> Result<ParseResponse, String> {
    let lexer_table = build_lexer_table_from_str(yal)?;

    // Lex the source (normalize CRLF to LF to avoid treating '\r' as invalid char)
    let source_clean = source.replace('\r', "");
    // Una sola lectura de la gramática y una sola construcción de la tabla para
    // toda la petición. Antes se parseaba tres veces y el autómata LR se
    // construía dos: una en `build_parse_response` y otra en
    // `build_real_parse_tree`/`collect_lr_syntax_errors`, cada una desde cero.
    // Construir el autómata LR(1) es la parte cara del pipeline.
    //
    // Solo para LALR/SLR: el camino LL(1) usa OTRA gramática (la factorizada,
    // con producciones renombradas) y su propia tabla, así que no puede
    // compartir esta.
    let grammar_for_filter = Grammar::parse_for_lr_from_str(yalp)?;
    let lr_table = if mode != "ll1" { Some(build_lr_table(&grammar_for_filter, mode)) } else { None };
    let mut sim = Simulator::new(&lexer_table, &source_clean);
    let mut token_map: Vec<(String, String, usize, usize)> = Vec::new();
    let mut lex_problems: Vec<Value> = Vec::new();

    // Tokenize whole source but keep token positions (line/col).
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let k = lex_normalize_kind(&t.kind);
                if !lex_is_ignored(&k, &grammar_for_filter) {
                    token_map.push((k.clone(), t.lexeme.clone(), t.line, t.col));
                }
            }
            LexResult::Error { lexeme, line, col } => {
                lex_problems.push(json!({
                    "level": "err",
                    "code": "L001",
                    "msg": format!("Carácter no reconocido: '{}'", lexeme),
                    "loc": format!("{}:{}:{}", source_name, line, col),
                    "line": line,
                    "col": col
                }));
            }
            LexResult::EOF => break,
        }
    }

    // Gramáticas sensibles a indentación (Python-style: declaran NEWLINE +
    // INDENT + DEDENT como %token) necesitan un post-procesamiento que el DFA
    // del lexer no puede hacer por sí solo — ver src/lexico/runtime/indent.rs. Es
    // opt-in por gramática, así que no afecta a ninguna otra gramática.
    // Degrada con un diagnóstico en vez de abortar toda la respuesta: el resto
    // del pipeline puede seguir intentando parsear con el token_map original.
    if indent::is_indent_sensitive(&grammar_for_filter.tokens) {
        match indent::synthesize(token_map.clone()) {
            Ok(synthesized) => token_map = synthesized,
            Err(msg) => {
                lex_problems.push(json!({
                    "level": "err",
                    "code": "L002",
                    "msg": msg,
                    "loc": source_name,
                }));
            }
        }
    }

    // Parse the whole token stream as ONE input — not grouped by physical line.
    // A source can be a single multi-line program (a line break is not a statement
    // boundary in general); callers that want several independent test cases already
    // send one call per case (frontend/IDE/app.jsx's handleParse sends one line at a
    // time, and tests/run_examples_cases.rs iterates `src.lines()` itself). Splitting
    // internally here both mis-locates errors in genuine multi-line input and rebuilds
    // the whole parse table once per line for no reason.
    let token_kinds: Vec<String> = token_map.iter().map(|(k, _, _, _)| k.clone()).collect();

    let mut response = ParseResponse::default();
    response.accepted = true;

    if !token_kinds.is_empty() {
        let resp = match &lr_table {
            Some(table) => build_parse_response_with(table, token_kinds.clone()),
            None => build_parse_response(yalp, token_kinds.clone(), mode)?,
        };
        response.accepted = resp.accepted;

        // Attach the source line of the token each step is about to consume, for the
        // UI to highlight while stepping through the trace.
        response.trace = resp
            .trace
            .iter()
            .map(|t| {
                let consumed = t
                    .get("remaining")
                    .and_then(|r| r.as_array())
                    .map(|arr| token_kinds.len().saturating_sub(arr.len()))
                    .unwrap_or(0);
                let line = token_map
                    .get(consumed)
                    .or_else(|| token_map.last())
                    .map(|tk| tk.2)
                    .unwrap_or(1);
                let mut entry = json!({ "line": line });
                if let Some(stack) = t.get("stack") { entry["stack"] = stack.clone(); }
                if let Some(remaining) = t.get("remaining") { entry["remaining"] = remaining.clone(); }
                if let Some(action) = t.get("action") { entry["action"] = action.clone(); }
                if let Some(desc) = t.get("desc") { entry["desc"] = desc.clone(); }
                entry
            })
            .collect();

        if !resp.accepted {
            response.error = Some("Error sintáctico".to_string());

            // Panic-mode recovery (only available for LALR/SLR — LL(1) has no
            // LRTable/LRParser) reports EVERY syntax error in one pass instead of
            // just the first (B2: parse_recovering_with_pos existed but nothing
            // called it). Fall back to the single first-error report — using the
            // same "which token was the driver stuck on" heuristic as before — when
            // recovery isn't available or finds nothing (e.g. LL(1) mode).
            let recovered = match &lr_table {
                Some(table) => collect_lr_syntax_errors_with(table, &grammar_for_filter, &token_map),
                None => collect_lr_syntax_errors(yalp, &token_map, mode),
            };
            if !recovered.is_empty() {
                for (kind, lexeme, line, col, msg) in &recovered {
                    push_syntax_problem(&mut lex_problems, kind, lexeme, *line, *col, msg, &grammar_for_filter.tokens, source_name);
                }
            } else {
                // Compute which token was consumed according to the trace's remaining length
                let consumed = resp
                    .trace
                    .last()
                    .and_then(|last| {
                        last.get("remaining").and_then(|r| r.as_array().map(|arr| token_kinds.len().saturating_sub(arr.len())))
                    })
                    .unwrap_or_else(|| {
                        resp.trace
                            .iter()
                            .filter(|s| {
                                let a = s.get("action").and_then(|v| v.as_str()).unwrap_or("");
                                a == "match" || a.starts_with('s')
                            })
                            .count()
                    });

                let loc_entry = token_map.get(consumed).or_else(|| token_map.last());
                if let Some((kind, lexeme, line, col)) = loc_entry {
                    let base_msg = resp.error.clone().unwrap_or_else(|| "Error sintáctico".to_string());
                    push_syntax_problem(&mut lex_problems, kind, lexeme, *line, *col, &base_msg, &grammar_for_filter.tokens, source_name);
                }
            }
        } else {
            // Aceptada: construir el árbol REAL (no el trace JSON) para
            // `parse_tree_dot`, y si el `.yalp` trae `%ident`, correr el
            // analizador semántico sobre ese mismo árbol. Solo para LALR/SLR:
            // `parse_for_ll1_from_str` aplica eliminación de recursión
            // izquierda y factorización, que renombra producciones — un
            // `SemanticSpec` escrito contra los nombres originales del .yalp
            // dejaría de encontrarlas y generaría diagnósticos falsos.
            // En LALR/SLR el árbol sale de la MISMA tabla que ya se usó para la
            // traza; solo LL(1) tiene que rearmar su gramática factorizada.
            let real_tree = match &lr_table {
                Some(table) => build_real_parse_tree_with(table, &token_map)
                    .map(|tree| (grammar_for_filter.clone(), tree)),
                None => build_real_parse_tree(yalp, &token_map, mode),
            };
            if let Some((grammar, tree)) = real_tree {
                // El DOT se genera DESPUES del analisis, no antes: si hubo
                // fase semantica el arbol sale anotado con el tipo de cada
                // expresion (el "arbol de analisis anotado" del libro), y eso
                // exige tener las anotaciones ya calculadas. Sin analisis
                // (LL(1), o un .yalp sin `%ident`) sale el DOT plano de
                // siempre.
                let mut annotations = None;
                if mode != "ll1" {
                    if let Some(spec) = SemanticSpec::from_grammar(&grammar) {
                        let analysis = analyze(&tree, &spec);
                        response.symbol_table = analysis.table.dump();
                        response.closures = analysis.closures.to_json();
                        response.scopes = analysis.scopes.to_json();
                        response.types = analysis.types.to_json(&tree);
                        lex_problems.extend(analysis.errors.to_problems(source_name));
                        annotations = Some(analysis.types);
                    }
                }
                // `tree` sigue vivo y sin mover desde antes de `analyze`, que
                // es la invariante que necesitan las claves de
                // `TypeAnnotations` (ver `semantico::types::annotations`).
                response.parse_tree_dot = match &annotations {
                    Some(types) => to_dot_annotated(&tree, types),
                    None => to_dot(&tree),
                };
            }
        }
    }

    let token_map_json: Vec<Value> = token_map
        .iter()
        .map(|(k, lx, l, c)| json!({"kind": k, "lexeme": lx, "line": l, "col": c}))
        .collect();

    response.problems = lex_problems;
    response.token_map = token_map_json;
    Ok(response)
}
