// Post-procesamiento de indentación estilo Python.
//
// El motor de lexeo del proyecto es un DFA clásico (Thompson → subconjuntos
// → minimización) de una sola pasada: reconoce tokens por expresión regular
// contra el stream de caracteres, sin memoria de "en qué columna empezó la
// línea anterior". La indentación sensible al contexto NO es un lenguaje
// regular — necesita una pila de niveles de indentación comparada línea a
// línea, un paso aparte del DFA (el mismo truco que usa el propio tokenizer
// de CPython). Este módulo es ese paso aparte.
//
// Es estrictamente opt-in por gramática (ver `is_indent_sensitive`): una
// gramática que no declare NEWLINE + INDENT + DEDENT como %token nunca pasa
// por aquí, así que el 99% de las gramáticas del proyecto no se ven
// afectadas en absoluto.

use std::collections::HashSet;

const NEWLINE_KIND: &str = "NEWLINE";
const INDENT_KIND: &str = "INDENT";
const DEDENT_KIND: &str = "DEDENT";

/// La gramática debe declarar los tres tokens para que `synthesize` tenga
/// sentido: sin NEWLINE no hay forma de saber dónde termina una línea
/// lógica, y sin INDENT/DEDENT no hay nada que sintetizar. Mismo patrón que
/// `grammar.ignores` — un %token declarado activa comportamiento especial
/// en el post-procesamiento sin que el core del lexer/parser sepa nada de
/// "Python": `Grammar::validate` ya soporta declarar y usar INDENT/DEDENT
/// como tokens ordinarios en producciones (python_hard.yalp:16,98 y
/// hardtest.yalp ya lo hacen), solo faltaba quién los emitiera.
pub fn is_indent_sensitive(grammar_tokens: &HashSet<String>) -> bool {
    grammar_tokens.contains(NEWLINE_KIND)
        && grammar_tokens.contains(INDENT_KIND)
        && grammar_tokens.contains(DEDENT_KIND)
}

/// Inserta tokens INDENT/DEDENT sintéticos en un stream YA FILTRADO de
/// ignorables (whitespace, comentarios, etc. — lo que sea que el llamador
/// ya haya descartado antes de llegar aquí), como `(kind, lexeme, line,
/// col)`.
///
/// Algoritmo (idéntico en espíritu al tokenizer de Python):
///   - Se mantiene una pila de niveles de indentación, arrancando en `[0]`.
///   - Cada vez que el token actual es el PRIMER token significativo de una
///     línea lógica (es decir: el token inmediatamente después de un
///     NEWLINE, o el primer token del archivo), su columna se compara
///     contra el tope de la pila:
///       - si es mayor: se apila y se sintetiza un INDENT.
///       - si es menor: se desapila (posiblemente varias veces) y se
///         sintetiza un DEDENT por cada nivel cerrado.
///       - si es igual: no pasa nada.
///   - Una línea cuyo ÚNICO token es el propio NEWLINE (línea en blanco, o
///     una línea cuyo contenido era todo ignorable — comentario,
///     docstring) NO dispara la comparación: el disparador es "el próximo
///     token que no sea NEWLINE", así que una línea sin ningún token
///     significativo simplemente no cuenta para la indentación, igual que
///     en Python real.
///   - Al agotarse los tokens, cualquier nivel que siga abierto (> 0) se
///     cierra con un DEDENT, anclado a la línea del último token real.
///
/// Devuelve `Err` si, tras desapilar, la columna de una línea no coincide
/// con NINGÚN nivel abierto (indentación inconsistente — el mismo caso que
/// en Python real levanta `IndentationError`).
pub fn synthesize(
    tokens: Vec<(String, String, usize, usize)>,
) -> Result<Vec<(String, String, usize, usize)>, String> {
    let mut stack: Vec<usize> = vec![0];
    let mut out: Vec<(String, String, usize, usize)> = Vec::with_capacity(tokens.len());
    let mut at_line_start = true;
    let mut last_line = 1usize;

    for (kind, lexeme, line, col) in tokens {
        // Una línea cuyo único contenido era un comentario o docstring (ya
        // filtrados como IGNORE antes de llegar aquí) todavía deja su propio
        // NEWLINE de cierre en el stream — igual que una línea en blanco.
        // Gramáticas con `suite: NEWLINE INDENT ...` no toleran un segundo
        // NEWLINE consecutivo (no hay producción que lo consuma), así que
        // ambos casos se descartan aquí: si YA estábamos "al inicio de
        // línea" (la línea anterior no dejó ningún token significativo) un
        // NEWLINE más no aporta nada gramaticalmente. Mismo comportamiento
        // que el NL (vs. NEWLINE) del tokenizer real de Python.
        if kind == NEWLINE_KIND && at_line_start {
            last_line = line;
            continue;
        }

        if at_line_start && kind != NEWLINE_KIND {
            let indent = col.saturating_sub(1);
            let top = *stack.last().unwrap();

            if indent > top {
                stack.push(indent);
                out.push((INDENT_KIND.to_string(), String::new(), line, 1));
            } else {
                while indent < *stack.last().unwrap() {
                    stack.pop();
                    out.push((DEDENT_KIND.to_string(), String::new(), line, 1));
                }
                if indent != *stack.last().unwrap() {
                    return Err(format!(
                        "Indentación inconsistente en la línea {}: la columna {} no coincide \
                         con ningún nivel de indentación abierto ({:?}).",
                        line,
                        col,
                        stack
                    ));
                }
            }

            at_line_start = false;
        }

        if kind == NEWLINE_KIND {
            at_line_start = true;
        }

        last_line = line;
        out.push((kind, lexeme, line, col));
    }

    // Si el archivo no termina con un salto de línea, la última línea lógica
    // nunca dispara `at_line_start = true` y por lo tanto nunca deja un
    // NEWLINE en `out` — pero la gramática espera uno antes del/de los
    // DEDENT de cierre (`simple_stmt NEWLINE`, igual que cualquier otra
    // sentencia). El tokenizer de CPython hace lo mismo: sintetiza un
    // NEWLINE final si falta antes de emitir ENDMARKER.
    if !at_line_start {
        out.push((NEWLINE_KIND.to_string(), String::new(), last_line, 1));
    }

    while stack.len() > 1 {
        stack.pop();
        out.push((DEDENT_KIND.to_string(), String::new(), last_line, 1));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tok(kind: &str, line: usize, col: usize) -> (String, String, usize, usize) {
        (kind.to_string(), kind.to_string(), line, col)
    }

    fn kinds(tokens: &[(String, String, usize, usize)]) -> Vec<&str> {
        tokens.iter().map(|(k, _, _, _)| k.as_str()).collect()
    }

    #[test]
    fn is_indent_sensitive_requires_all_three_tokens() {
        let mut set: HashSet<String> = HashSet::new();
        assert!(!is_indent_sensitive(&set));
        set.insert("NEWLINE".to_string());
        set.insert("INDENT".to_string());
        assert!(!is_indent_sensitive(&set), "falta DEDENT");
        set.insert("DEDENT".to_string());
        assert!(is_indent_sensitive(&set));
    }

    #[test]
    fn no_indentation_change_emits_nothing_extra() {
        // "IF ... COLON NEWLINE" seguido de otra línea al mismo nivel (col 1).
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 1),
            tok("NEWLINE", 2, 5),
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(kinds(&out), vec!["IF", "COLON", "NEWLINE", "PASS", "NEWLINE"]);
    }

    #[test]
    fn simple_indent_and_dedent() {
        // if ...:      (col 1)
        //     pass     (col 5 -> indent 4)
        // x = 1        (col 1 -> dedent a 0)
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 5),
            tok("NEWLINE", 2, 9),
            tok("ID", 3, 1),
            tok("NEWLINE", 3, 6),
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(
            kinds(&out),
            vec!["IF", "COLON", "NEWLINE", "INDENT", "PASS", "NEWLINE", "DEDENT", "ID", "NEWLINE"]
        );
    }

    #[test]
    fn nested_indent_and_multi_level_dedent() {
        // if ...:            col 1
        //     if ...:        col 5  -> INDENT (4)
        //         pass       col 9  -> INDENT (8)
        // x = 1              col 1  -> DEDENT, DEDENT (8 -> 4 -> 0)
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("IF", 2, 5),
            tok("COLON", 2, 14),
            tok("NEWLINE", 2, 15),
            tok("PASS", 3, 9),
            tok("NEWLINE", 3, 13),
            tok("ID", 4, 1),
            tok("NEWLINE", 4, 6),
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(
            kinds(&out),
            vec![
                "IF", "COLON", "NEWLINE", "INDENT", "IF", "COLON", "NEWLINE", "INDENT", "PASS",
                "NEWLINE", "DEDENT", "DEDENT", "ID", "NEWLINE"
            ]
        );
    }

    #[test]
    fn blank_line_inside_a_block_does_not_affect_the_stack() {
        // if ...:
        //     pass
        // <línea en blanco>
        //     pass
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 5),
            tok("NEWLINE", 2, 9),
            tok("NEWLINE", 3, 1), // línea en blanco: único token es NEWLINE
            tok("PASS", 4, 5),
            tok("NEWLINE", 4, 9),
        ];
        let out = synthesize(input).unwrap();
        // Solo UN INDENT (de la línea 2) — la línea en blanco no dispara nada
        // en la pila NI dejar rastro en el stream (se descarta, igual que un
        // NL de CPython), y la línea 4 sigue al mismo nivel que la 2, así que
        // tampoco dispara. El DEDENT final es el flush de EOF (el input nunca
        // vuelve a nivel 0).
        assert_eq!(
            kinds(&out),
            vec!["IF", "COLON", "NEWLINE", "INDENT", "PASS", "NEWLINE", "PASS", "NEWLINE", "DEDENT"]
        );
    }

    #[test]
    fn comment_only_line_newline_is_dropped_not_just_ignored_for_indent() {
        // Una línea cuyo único contenido era un comentario (ya filtrado como
        // IGNORE antes de llegar a `synthesize`) todavía deja su propio
        // NEWLINE de cierre en el stream. Una gramática con
        // `suite: NEWLINE INDENT ...` no tolera un segundo NEWLINE seguido —
        // debe descartarse por completo, no solo "no contar" para la pila.
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("NEWLINE", 2, 30), // línea 2 era solo un comentario
            tok("PASS", 3, 5),
            tok("NEWLINE", 3, 9),
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(
            kinds(&out),
            vec!["IF", "COLON", "NEWLINE", "INDENT", "PASS", "NEWLINE", "DEDENT"],
            "el NEWLINE huérfano de la línea de comentario debe desaparecer del todo: {:?}",
            out
        );
    }

    #[test]
    fn missing_trailing_newline_before_eof_is_synthesized() {
        // Un archivo que no termina en '\n' (última línea sin terminador)
        // nunca dispara `at_line_start`, así que sin este fix el DEDENT de
        // cierre de EOF llegaría sin el NEWLINE que `simple_stmt NEWLINE`
        // exige. CPython sintetiza ese NEWLINE final; nosotros también.
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 5), // sin NEWLINE detrás: EOF llega justo aquí
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(
            kinds(&out),
            vec!["IF", "COLON", "NEWLINE", "INDENT", "PASS", "NEWLINE", "DEDENT"]
        );
    }

    #[test]
    fn eof_flushes_remaining_indent_levels() {
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 5),
            tok("NEWLINE", 2, 9),
        ];
        let out = synthesize(input).unwrap();
        assert_eq!(kinds(&out), vec!["IF", "COLON", "NEWLINE", "INDENT", "PASS", "NEWLINE", "DEDENT"]);
    }

    #[test]
    fn inconsistent_indentation_is_an_error() {
        // Abre a columna 5 (indent 4), luego una línea a columna 3 (indent 2)
        // que no coincide con ningún nivel abierto ([0, 4]).
        let input = vec![
            tok("IF", 1, 1),
            tok("COLON", 1, 10),
            tok("NEWLINE", 1, 11),
            tok("PASS", 2, 5),
            tok("NEWLINE", 2, 9),
            tok("ID", 3, 3),
            tok("NEWLINE", 3, 8),
        ];
        let err = synthesize(input).unwrap_err();
        assert!(err.contains("línea 3"), "el error debe mencionar la línea: {}", err);
    }
}
