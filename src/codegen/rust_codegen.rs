// Fase 13: Generar archivo Rust del lexer (generated/lexer.rs).
// Tablas estáticas + next_token/tokenize + header/trailer del .yal.

use std::fs;
use std::path::Path;
use crate::table::transition_table::TransitionTable;
use crate::spec::expand::ExpandedRule;

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

/// Genera el archivo `path` con el lexer en Rust.
/// Crea el directorio padre si no existe.
pub fn emit_file(
    path: &str,
    tt: &TransitionTable,
    _rules: &[ExpandedRule],
    header: Option<&str>,
    trailer: Option<&str>,
) -> std::io::Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        fs::create_dir_all(parent)?;
    }

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
        "static ACCEPT: [Option<&'static str>; {}] = [\n",
        n
    ));
    for s in 0..n {
        match &tt.accept[s] {
            Some(act) => {
                code.push_str(&format!("    Some(\"{}\"),\n", escape_rust_string(act)));
            }
            None => code.push_str("    None,\n"),
        }
    }
    code.push_str("];\n\n");

    code.push_str("#[derive(Debug, Clone)]\npub struct Token {\n");
    code.push_str("    pub kind: &'static str,\n");
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
    code.push_str("    let (mut last_pos, mut last_tok) = (None::<usize>, None::<&str>);\n\n");
    code.push_str("    while *pos < input.len() {\n");
    code.push_str("        let c = input[*pos] as usize;\n");
    code.push_str("        if c >= 128 { break; }\n");
    code.push_str("        let next = DELTA[state as usize][c];\n");
    code.push_str("        if next == DEAD { break; }\n");
    code.push_str("        state = next; *pos += 1;\n");
    code.push_str("        if input[*pos - 1] == '\\n' { *line += 1; *col = 1; } else { *col += 1; }\n");
    code.push_str("        if let Some(tok) = ACCEPT[state as usize] {\n");
    code.push_str("            last_pos = Some(*pos); last_tok = Some(tok);\n");
    code.push_str("        }\n");
    code.push_str("    }\n\n");
    code.push_str("    if let Some(p) = last_pos {\n");
    code.push_str("        *pos = p;\n");
    code.push_str("        let lexeme: String = input[start..p].iter().collect();\n");
    code.push_str("        Some(Ok(Token { kind: last_tok.unwrap(), lexeme, line: start_line, col: start_col }))\n");
    code.push_str("    } else {\n");
    code.push_str("        let bad = input[start]; *pos = start + 1;\n");
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

    if let Some(t) = trailer {
        code.push_str("\n\n");
        code.push_str(t.trim());
        code.push('\n');
    }

    fs::write(path, code)
}
