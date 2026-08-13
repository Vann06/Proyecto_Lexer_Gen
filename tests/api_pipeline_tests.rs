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
