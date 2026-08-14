// Tests for error message content and accurate line/column locations.
use lexer_generator::api;

#[test]
fn pipeline_parse_error_location_multiline() {
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n  | ['\\n' ' ' '\\t']+ { }\n  | '+' { return PLUS }\n";
    let yalp = "%token NUM PLUS\n%%\nS : NUM PLUS NUM ;\n";
    let src = "12\n+\n"; // missing final NUM

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return response");

    let syn_err = resp.problems.iter().find(|p| p["code"] == "P001");
    assert!(syn_err.is_some(), "should report syntax error P001");
    let syn_err = syn_err.unwrap();
    // Expect the error to point at the '+' token on line 2, column 1
    assert_eq!(syn_err["line"].as_u64().unwrap(), 2);
    assert_eq!(syn_err["col"].as_u64().unwrap(), 1);
    let msg = syn_err["msg"].as_str().unwrap_or("");
    assert!(msg.contains("Esperado:") || msg.contains("Esperado"));
}

#[test]
fn pipeline_lex_error_multiline_location() {
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n  | ['\\n' ' ' '\\t']+ { }\n";
    let yalp = "%token NUM\n%%\nS : NUM ;\n";
    let src = "12\nx\n"; // 'x' is not recognized

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return response");

    let lex_err = resp.problems.iter().find(|p| p["code"] == "L001");
    assert!(lex_err.is_some(), "should report lexical error L001");
    let lex_err = lex_err.unwrap();
    assert_eq!(lex_err["line"].as_u64().unwrap(), 2);
    assert_eq!(lex_err["col"].as_u64().unwrap(), 1);
    let msg = lex_err["msg"].as_str().unwrap_or("");
    assert!(msg.contains("Carácter no reconocido") || msg.contains("no reconocido"));
}

#[test]
fn pipeline_misspelled_keyword_becomes_lex_error() {
    let yal = "let letter = ['a'-'z' 'A'-'Z']\nlet digit = ['0'-'9']\nlet id = letter (letter|digit)*\nrule tokens =\n  | 'return' { return RETURN }\n  | id { return ID }\n  | ['\\n' ' ' '\\t']+ { }\n";
    let yalp = "%token RETURN ID\n%%\nS : RETURN ;\n";
    let src = "retrun\n"; // typo: retrun

    let resp = api::build_pipeline_response(yal, yalp, src, "lalr")
        .expect("pipeline should return response");

    let lex_err = resp.problems.iter().find(|p| p["code"] == "L001");
    assert!(lex_err.is_some(), "misspelled keyword should produce L001");
    let lex_err = lex_err.unwrap();
    assert_eq!(lex_err["line"].as_u64().unwrap(), 1);
    assert_eq!(lex_err["col"].as_u64().unwrap(), 1);
    let msg = lex_err["msg"].as_str().unwrap_or("");
    assert!(msg.contains("quiso") || msg.contains("¿quiso"), "message should suggest correction");
}
