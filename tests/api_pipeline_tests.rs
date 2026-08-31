// Tests for API business logic (no HTTP server required).
use lexer_generator::api;

#[test]
fn pipeline_reports_lex_error_with_location() {
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n";
    let yalp = "%token NUM\n%%\nS : NUM ;\n";
    let src = "12a";

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return response");

    let lex_err = resp.problems.iter().find(|p| p["code"] == "L001");
    assert!(lex_err.is_some(), "should report lexical error L001");
    let lex_err = lex_err.unwrap();
    assert!(lex_err["line"].as_u64().is_some(), "lex error should include line");
    assert!(lex_err["col"].as_u64().is_some(), "lex error should include col");
    assert!(lex_err["loc"].as_str().unwrap_or("").contains("input.txt"));
}

#[test]
fn lr_parse_error_includes_expected_tokens() {
    let yalp = "%token ID PLUS\n%%\nS : ID PLUS ID ;\n";
    let resp = api::build_parse_response(
        yalp,
        vec!["ID".to_string()],
        "lalr",
    ).expect("parse response");

    let last = resp.trace.last().expect("trace should have steps");
    let desc = last["desc"].as_str().unwrap_or("");
    assert!(desc.contains("Esperado:"), "parse error should list expected tokens");
}

#[test]
fn pipeline_syntax_error_includes_expected_tokens_in_message() {
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n";
    let yalp = "%token NUM PLUS\n%%\nS : NUM PLUS NUM ;\n";
    let src = "12 34"; // missing PLUS

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return response");

    let syn_err = resp.problems.iter().find(|p| p["code"] == "P001");
    assert!(syn_err.is_some(), "should report syntax error P001");
    let syn_err = syn_err.unwrap();
    let msg = syn_err["msg"].as_str().unwrap_or("");
    assert!(msg.contains("Esperado:"), "syntax error should include expected tokens");
}

// A4 — the workspace's default lexer.yal/parser.yalp/input.txt must be mutually
// consistent (same tokens on both sides) and `{ skip }` must be treated as a
// discard action, exactly like the workspace files served by src/bin/api.rs.
#[test]
fn default_workspace_files_parse_a_default_input_line() {
    let yal = "let digit = ['0'-'9']\nlet letter = ['a'-'z' 'A'-'Z']\nlet id = letter (letter|digit)*\nrule tokens =\n  | id         { return ID }\n  | digit+     { return NUM }\n  | '+'        { return PLUS }\n  | '*'        { return STAR }\n  | ' '        { skip }";
    let yalp = "%token ID NUM PLUS STAR\n%start E\n%%\nE : E PLUS T\n  | T\n  ;\nT : T STAR F\n  | F\n  ;\nF : ID\n  | NUM\n  ;\n";

    for line in ["a + b", "3 * x", "a + b * c"] {
        let resp = api::build_pipeline_response(yal, yalp, line, "lalr")
            .unwrap_or_else(|e| panic!("pipeline should build a response for '{}': {}", line, e));
        assert!(
            resp.problems.is_empty() && resp.accepted,
            "default workspace files should accept '{}' with no problems, got: accepted={} problems={:?}",
            line, resp.accepted, resp.problems
        );
    }
}

// B2 — parse_recovering_with_pos (panic-mode recovery) existed in parser_lr.rs but
// nothing called it, so build_pipeline_response only ever reported the FIRST
// syntax error on a line even when there were several independent ones. Now
// wired in for LALR/SLR: verify two unrelated mistakes both get reported.
#[test]
fn pipeline_reports_more_than_one_syntax_error_via_panic_mode_recovery() {
    let yal = "let digit = ['0'-'9']\nlet letter = ['a'-'z']\nrule tokens =\n  | letter { return ID }\n  | digit+ { return NUM }\n  | '='    { return ASSIGN }\n  | ';'    { return SEMI }\n  | ' '    { skip }";
    let yalp = "%token ID ASSIGN NUM SEMI\n%%\nprogram : stmt_list ;\nstmt_list : stmt_list stmt | stmt ;\nstmt : ID ASSIGN NUM SEMI ;\n";
    // Two independent mistakes: 'z' where a NUM is expected, and later a stray ';'
    // where the recovered state expects ASSIGN.
    let src = "x = 5 ; y = z ; w = 9 ;";

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return a response");

    assert!(!resp.accepted, "input has genuine syntax errors, must not be accepted");
    let syntax_errors: Vec<_> = resp.problems.iter().filter(|p| p["code"] == "P001").collect();
    assert!(
        syntax_errors.len() >= 2,
        "expected at least 2 independently-recovered syntax errors, got {}: {:?}",
        syntax_errors.len(), resp.problems
    );
}

/// El endpoint expone los ambitos parciales, no solo la tabla global.
///
/// Lo que se comprueba de verdad es que aparezca una variable declarada DENTRO
/// DE UN BLOQUE: la tabla global no la lleva (al terminar el recorrido solo
/// queda el Global) y `symbol_table` solo anida lo de funciones y clases, asi
/// que sin este campo el frontend no tiene forma de mostrarla.
#[test]
fn pipeline_exposes_partial_scopes_including_block_locals() {
    let yal = std::fs::read_to_string("workspace/compiscript.yal").expect("lexer");
    let yalp = std::fs::read_to_string("workspace/compiscript.yalp").expect("gramatica");
    let source = std::fs::read_to_string("workspace/ejemplo.cps").expect("fuente");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "ejemplo.cps")
        .expect("el pipeline no debe fallar");

    assert!(!resp.scopes.is_empty(), "debe haber ambitos: {:#?}", resp.scopes);

    // `resultado` vive en el bloque del cuerpo de `suma`.
    let bloque = resp
        .scopes
        .iter()
        .find(|s| {
            s["symbols"]
                .as_array()
                .map(|syms| syms.iter().any(|y| y["name"] == "resultado"))
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("`resultado` debe aparecer en algun ambito: {:#?}", resp.scopes));

    assert_eq!(bloque["kind"], "Block", "vive en un bloque, no en la funcion");
    let sym = bloque["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .find(|y| y["name"] == "resultado")
        .unwrap();
    assert_eq!(sym["ty"], "integer", "llega tipado, no solo declarado");
    assert!(sym["line"].as_u64().unwrap() > 0 && sym["col"].as_u64().unwrap() > 0);

    // Y NO esta en la tabla global: es justo el hueco que este campo tapa.
    assert!(
        !resp.symbol_table.contains("resultado"),
        "la tabla global no lo lleva; por eso hacen falta los ambitos:\n{}",
        resp.symbol_table
    );

    // El ambito con nombre trae su etiqueta, para poder mostrarlo en el IDE.
    assert!(
        resp.scopes.iter().any(|s| s["kind"] == "Function" && s["label"] == "suma"),
        "el ambito de `suma` debe venir etiquetado: {:#?}",
        resp.scopes
    );
}

/// Sin `%ident` no hay analisis semantico, asi que tampoco ambitos — igual que
/// pasa con `symbol_table` y `closures`.
#[test]
fn pipeline_without_ident_directive_returns_no_scopes() {
    let yal = std::fs::read_to_string("workspace/miniprog.yal").expect("lexer");
    let yalp = std::fs::read_to_string("workspace/miniprog.yalp").expect("gramatica");
    let source = std::fs::read_to_string("workspace/miniprog_test.txt").expect("fuente");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "miniprog.txt")
        .expect("el pipeline no debe fallar");

    assert!(resp.scopes.is_empty(), "miniprog.yalp no trae %ident");
    assert!(resp.symbol_table.is_empty());
}
