// Lectura y parsing del archivo .yal
// Detecta y separa:
//   - Definiciones (let name = regex)
//   - Sección de reglas (rule tokens = ...)
//   - Bloques de código opcionales (header y trailer)
// El resultado queda en la estructura SpecIR

use crate::error::LexerGenError;
use crate::spec::ast::{Definition, Rule, SpecIR};

pub fn parse_yalex(input: &str) -> Result<SpecIR, LexerGenError> {
    let mut header = None;
    let mut trailer = None;
    let mut definitions = Vec::new();
    let mut rules = Vec::new();

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;
    let mut in_rule_section = false;
    let mut priority = 1;

    while i < lines.len() {
        let line = lines[i].trim();

        if line.is_empty() || line.starts_with("(*") || line.starts_with("//") {
            i += 1;
            continue;
        }

        // Header opcional entre llaves al inicio
        if i == 0 && line.starts_with('{') {
            let mut block = String::new();
            let mut found_end = false;
            let mut brace_count = 0;

            while i < lines.len() {
                for c in lines[i].chars() {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
                        brace_count -= 1;
                    }
                }
                
                block.push_str(lines[i]);
                block.push('\n');

                if brace_count == 0 {
                    found_end = true;
                    break;
                }
                i += 1;
            }

            if !found_end {
                return Err(LexerGenError::InvalidSpec(
                    "header no cerrado correctamente".to_string(),
                ));
            }

            let mut block_trimmed = block.trim().to_string();
            if block_trimmed.starts_with('{') {
                block_trimmed.remove(0);
            }
            if block_trimmed.ends_with('}') {
                block_trimmed.pop();
            }

            header = Some(block_trimmed);
            i += 1;
            continue;
        }

        // Definiciones let
        if line.starts_with("let ") {
            let rest = line.trim_start_matches("let ").trim();
            let parts: Vec<&str> = rest.splitn(2, '=').collect();

            if parts.len() != 2 {
                return Err(LexerGenError::InvalidDefinition(line.to_string()));
            }

            let name = parts[0].trim().to_string();
            let regex = parts[1].trim().to_string();

            definitions.push(Definition { name, regex });
            i += 1;
            continue;
        }

        // Inicio de la sección de reglas: "rule <nombre> ="
        // Todo lo que sigue (hasta fin de archivo o nuevo bloque) son reglas
        if line.starts_with("rule ") {
            in_rule_section = true;
            i += 1;
            continue;
        }

        // Reglas
        if in_rule_section && (line.starts_with('|') || !line.starts_with("let ")) {
            if line == "{" {
                // possible trailer
                let mut block = String::new();
                let mut found_end = false;
                let mut brace_count = 0;

                while i < lines.len() {
                    for c in lines[i].chars() {
                        if c == '{' {
                            brace_count += 1;
                        } else if c == '}' {
                            brace_count -= 1;
                        }
                    }

                    block.push_str(lines[i]);
                    block.push('\n');

                    if brace_count == 0 {
                        found_end = true;
                        break;
                    }
                    i += 1;
                }

                if !found_end {
                    return Err(LexerGenError::InvalidSpec(
                        "trailer no cerrado correctamente".to_string(),
                    ));
                }

                let mut block_trimmed = block.trim().to_string();
                if block_trimmed.starts_with('{') {
                    block_trimmed.remove(0);
                }
                if block_trimmed.ends_with('}') {
                    block_trimmed.pop();
                }

                trailer = Some(block_trimmed);
                i += 1;
                continue;
            }

            if let Some((pattern, action)) = split_rule_pattern_action(line) {
                rules.push(Rule {
                    pattern_raw: pattern,
                    action_code: action,
                    priority,
                });
                priority += 1;
                i += 1;
                continue;
            }
        }

        i += 1;
    }

    if rules.is_empty() {
        return Err(LexerGenError::InvalidSpec(
            "no se encontraron reglas en la especificación".to_string(),
        ));
    }

    Ok(SpecIR {
        header,
        definitions,
        rules,
        trailer,
    })
}

fn split_rule_pattern_action(line: &str) -> Option<(String, String)> {
    let clean = line.trim_start_matches('|').trim();

    let brace_end = clean.rfind('}')?;
    let brace_start = clean[..brace_end].rfind('{')?;

    if brace_end <= brace_start {
        return None;
    }

    let pattern = clean[..brace_start].trim().to_string();
    let action = clean[brace_start + 1..brace_end].trim().to_string();

    Some((pattern, action))
}