// Fase 13: Generar archivo Rust del lexer (generated/lexer.rs).
// Tablas estáticas + next_token/tokenize + header/trailer del .yal.

use std::fs;
use std::path::Path;
use crate::lexico::table::transition_table::{self, TransitionTable};
use crate::lexico::spec::expand::ExpandedRule;

/// Escapa un string para usarlo dentro de un literal Rust `"..."`.
fn escape_rust_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

/// Opciones de generación que dependen de la gramática (`.yalp`) — el lexer
/// generado en sí no tiene noción de `Grammar`, así que lo que se necesita
/// de ahí se hornea aquí en el momento de generar.
#[derive(Debug, Clone, Default)]
pub struct CodegenOptions {
    /// Si es true, emite `synthesize_indentation`/`tokenize_for_parser`
    /// (ver `runtime::indent`). Debe venir de
    /// `indent::is_indent_sensitive(&grammar.tokens)`.
    pub indent_sensitive: bool,
    /// Kinds (en cualquier case; se normalizan a MAYÚSCULAS) a descartar
    /// antes de sintetizar indentación — típicamente `grammar.ignores`
    /// (ej. COMMENT, DOCSTRING). El sentinel "IGNORED" (tokens con acción
    /// vacía/`skip`/`ignore`, ej. whitespace) siempre se incluye, sin
    /// necesidad de listarlo.
    pub ignored_kinds: Vec<String>,
}

/// Construye el código Rust del lexer como String, sin tocar el filesystem.
/// `emit_file` es un wrapper delgado sobre esto que además escribe a disco.
pub fn emit_string(
    tt: &TransitionTable,
    _rules: &[ExpandedRule],
    header: Option<&str>,
    trailer: Option<&str>,
    opts: &CodegenOptions,
) -> String {
    let mut code = String::new();

    code.push_str("// Generado automáticamente por YALex — NO editar\n\n");

    if let Some(h) = header {
        code.push_str(h.trim());
        code.push_str("\n\n");
    }

    let n = tt.n_states;
    code.push_str(&format!("const N_STATES: usize = {};\n", n));
    code.push_str("const DEAD: i32 = -1;\n\n");

    code.push_str(&format!("static DELTA: [[i32; 128]; {}] = [\n", n));
    for s in 0..n {
        code.push_str("    [");
        let row: Vec<String> = (0..128u8)
            .map(|c| tt.delta[s][c as usize].to_string())
            .collect();
        code.push_str(&row.join(", "));
        code.push_str("],\n");
    }
    code.push_str("];\n\n");

    code.push_str(&format!(
        "static ACCEPT: [Option<(&'static str, &'static str)>; {}] = [\n",
        n
    ));
    for s in 0..n {
        match &tt.accept[s] {
            Some(act) => {
                // Extracción de kind vía transition_table::kind_from_action —
                // única fuente de verdad compartida con el Simulator interpretado
                // (antes esta copia solo entendía `Token::X` y caía a "Unknown"
                // para el estilo `{ "X" }` que usan 5 de los 8 .yal de ejemplo).
                // Se normaliza a MAYÚSCULAS para calzar con los %token del .yalp,
                // igual que hace el pipeline interpretado (lex_normalize_kind).
                let kind_name = transition_table::kind_from_action(act).to_uppercase();
                code.push_str(&format!("    Some((\"{}\", \"{}\")),\n", kind_name, escape_rust_string(act)));
            }
            None => code.push_str("    None,\n"),
        }
    }
    code.push_str("];\n\n");

    code.push_str("#[derive(Debug, Clone)]\npub struct Token {\n");
    code.push_str("    pub kind: &'static str,\n");
    code.push_str("    pub action: &'static str,\n");
    code.push_str("    pub lexeme: String,\n");
    code.push_str("    pub line: usize,\n");
    code.push_str("    pub col: usize,\n");
    code.push_str("}\n\n");

    code.push_str(
        "pub fn next_token(\
            input: &[char], pos: &mut usize, line: &mut usize, col: &mut usize)\
            -> Option<Result<Token, String>>\n{\n",
    );
    code.push_str("    if *pos >= input.len() { return None; }\n");
    code.push_str(&format!("    let mut state: i32 = {};\n", tt.start));
    code.push_str("    let start = *pos;\n");
    code.push_str("    let start_line = *line;\n");
    code.push_str("    let start_col = *col;\n");
    code.push_str("    let (mut last_pos, mut last_tok, mut last_action) = (None::<usize>, None::<&str>, None::<&str>);\n");
    code.push_str("    let (mut last_line, mut last_col) = (start_line, start_col);\n\n");
    code.push_str("    while *pos < input.len() {\n");
    code.push_str("        let c = input[*pos] as usize;\n");
    code.push_str("        if c >= 128 { break; }\n");
    code.push_str("        let next = DELTA[state as usize][c];\n");
    code.push_str("        if next == DEAD { break; }\n");
    code.push_str("        state = next; *pos += 1;\n");
    code.push_str("        if input[*pos - 1] == '\\n' { *line += 1; *col = 1; } else { *col += 1; }\n");
    code.push_str("        if let Some((tok, act)) = ACCEPT[state as usize] {\n");
    code.push_str("            last_pos = Some(*pos); last_tok = Some(tok); last_action = Some(act);\n");
    code.push_str("            last_line = *line; last_col = *col;\n");
    code.push_str("        }\n");
    code.push_str("    }\n\n");
    code.push_str("    if let Some(p) = last_pos {\n");
    code.push_str("        // Roll line/col back together with pos: they may have drifted past a\n");
    code.push_str("        // longer speculative match that ultimately failed (maximal munch).\n");
    code.push_str("        *pos = p; *line = last_line; *col = last_col;\n");
    code.push_str("        let lexeme: String = input[start..p].iter().collect();\n");
    code.push_str("        Some(Ok(Token { kind: last_tok.unwrap(), action: last_action.unwrap(), lexeme, line: start_line, col: start_col }))\n");
    code.push_str("    } else {\n");
    code.push_str("        let bad = input[start]; *pos = start + 1;\n");
    code.push_str("        *line = start_line; *col = start_col;\n");
    code.push_str("        if bad == '\\n' { *line += 1; *col = 1; } else { *col += 1; }\n");
    code.push_str("        Some(Err(format!(\"Error léxico línea {}:{} — '{}'\", start_line, start_col, bad)))\n");
    code.push_str("    }\n");
    code.push_str("}\n\n");

    code.push_str("pub fn tokenize(src: &str) -> (Vec<Token>, Vec<String>) {\n");
    code.push_str("    let chars: Vec<char> = src.chars().collect();\n");
    code.push_str("    let (mut pos, mut line, mut col) = (0, 1, 1);\n");
    code.push_str("    let mut tokens = Vec::new(); let mut errors = Vec::new();\n");
    code.push_str("    while let Some(res) = next_token(&chars, &mut pos, &mut line, &mut col) {\n");
    code.push_str("        match res {\n");
    code.push_str("            Ok(t) => tokens.push(t),\n");
    code.push_str("            Err(e) => errors.push(e),\n");
    code.push_str("        }\n");
    code.push_str("    }\n");
    code.push_str("    (tokens, errors)\n");
    code.push_str("}\n");

    if opts.indent_sensitive {
        code.push('\n');
        emit_indent_support(&mut code, &opts.ignored_kinds);
    }

    if let Some(t) = trailer {
        code.push_str("\n\n");
        code.push_str(t.trim());
        code.push('\n');
    }

    code
}

/// Emite el post-procesamiento de indentación estilo Python — mismo
/// algoritmo que `runtime::indent::synthesize` (ver ese módulo para la
/// explicación completa), pero como código fuente Rust en vez de una
/// función que corre en el intérprete: el lexer generado es standalone y
/// no tiene acceso a esa función en tiempo de ejecución.
fn emit_indent_support(code: &mut String, ignored_kinds: &[String]) {
    // El sentinel "IGNORED" (acciones vacías/skip/ignore, ej. whitespace)
    // siempre se filtra, sin que el llamador tenga que pedirlo.
    let mut ignored: Vec<String> = vec!["IGNORED".to_string()];
    for k in ignored_kinds {
        let up = k.to_uppercase();
        if !ignored.contains(&up) {
            ignored.push(up);
        }
    }
    let ignored_list = ignored
        .iter()
        .map(|k| format!("\"{}\"", escape_rust_string(k)))
        .collect::<Vec<_>>()
        .join(", ");

    code.push_str(&format!("static IGNORED_KINDS: &[&str] = &[{}];\n\n", ignored_list));

    code.push_str("/// Sintetiza tokens INDENT/DEDENT (estilo Python) sobre un stream YA\n");
    code.push_str("/// FILTRADO de ignorables. Ver runtime::indent::synthesize (la función\n");
    code.push_str("/// de la que este código es un port literal) para la explicación\n");
    code.push_str("/// completa del algoritmo: pila de niveles de indentación comparada por\n");
    code.push_str("/// línea lógica, NEWLINEs huérfanos de línea en blanco/comentario\n");
    code.push_str("/// descartados, NEWLINE final sintetizado si falta antes de EOF.\n");
    code.push_str("pub fn synthesize_indentation(tokens: Vec<Token>) -> Result<Vec<Token>, String> {\n");
    code.push_str("    let mut stack: Vec<usize> = vec![0];\n");
    code.push_str("    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());\n");
    code.push_str("    let mut at_line_start = true;\n");
    code.push_str("    let mut last_line = 1usize;\n\n");
    code.push_str("    for tok in tokens {\n");
    code.push_str("        if tok.kind == \"NEWLINE\" && at_line_start {\n");
    code.push_str("            last_line = tok.line;\n");
    code.push_str("            continue;\n");
    code.push_str("        }\n\n");
    code.push_str("        if at_line_start && tok.kind != \"NEWLINE\" {\n");
    code.push_str("            let indent = tok.col.saturating_sub(1);\n");
    code.push_str("            let top = *stack.last().unwrap();\n");
    code.push_str("            if indent > top {\n");
    code.push_str("                stack.push(indent);\n");
    code.push_str("                out.push(Token { kind: \"INDENT\", action: \"\", lexeme: String::new(), line: tok.line, col: 1 });\n");
    code.push_str("            } else {\n");
    code.push_str("                while indent < *stack.last().unwrap() {\n");
    code.push_str("                    stack.pop();\n");
    code.push_str("                    out.push(Token { kind: \"DEDENT\", action: \"\", lexeme: String::new(), line: tok.line, col: 1 });\n");
    code.push_str("                }\n");
    code.push_str("                if indent != *stack.last().unwrap() {\n");
    code.push_str("                    return Err(format!(\n");
    code.push_str("                        \"Indentación inconsistente en la línea {}: la columna {} no coincide con ningún nivel de indentación abierto ({:?}).\",\n");
    code.push_str("                        tok.line, tok.col, stack\n");
    code.push_str("                    ));\n");
    code.push_str("                }\n");
    code.push_str("            }\n");
    code.push_str("            at_line_start = false;\n");
    code.push_str("        }\n\n");
    code.push_str("        if tok.kind == \"NEWLINE\" { at_line_start = true; }\n");
    code.push_str("        last_line = tok.line;\n");
    code.push_str("        out.push(tok);\n");
    code.push_str("    }\n\n");
    code.push_str("    if !at_line_start {\n");
    code.push_str("        out.push(Token { kind: \"NEWLINE\", action: \"\", lexeme: String::new(), line: last_line, col: 1 });\n");
    code.push_str("    }\n");
    code.push_str("    while stack.len() > 1 {\n");
    code.push_str("        stack.pop();\n");
    code.push_str("        out.push(Token { kind: \"DEDENT\", action: \"\", lexeme: String::new(), line: last_line, col: 1 });\n");
    code.push_str("    }\n\n");
    code.push_str("    Ok(out)\n");
    code.push_str("}\n\n");

    code.push_str("/// Filtra IGNORED_KINDS y sintetiza INDENT/DEDENT — punto de entrada\n");
    code.push_str("/// recomendado para alimentar un parser generado a partir de una\n");
    code.push_str("/// gramática sensible a indentación (equivalente standalone de lo que\n");
    code.push_str("/// hace build_pipeline_response en el servidor).\n");
    code.push_str("pub fn tokenize_for_parser(src: &str) -> Result<(Vec<Token>, Vec<String>), String> {\n");
    code.push_str("    let (raw, errors) = tokenize(src);\n");
    code.push_str("    let significant: Vec<Token> = raw.into_iter()\n");
    code.push_str("        .filter(|t| !IGNORED_KINDS.contains(&t.kind))\n");
    code.push_str("        .collect();\n");
    code.push_str("    let synthesized = synthesize_indentation(significant)?;\n");
    code.push_str("    Ok((synthesized, errors))\n");
    code.push_str("}\n");
}

/// Genera el archivo `path` con el lexer en Rust.
/// Crea el directorio padre si no existe.
pub fn emit_file(
    path: &str,
    tt: &TransitionTable,
    rules: &[ExpandedRule],
    header: Option<&str>,
    trailer: Option<&str>,
    opts: &CodegenOptions,
) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, emit_string(tt, rules, header, trailer, opts))
}
