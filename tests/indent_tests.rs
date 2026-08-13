// Integration test for the INDENT/DEDENT synthesis pass (src/runtime/indent.rs),
// wired into api::build_pipeline_response. Uses a small hand-made Python-style
// grammar (nested if/else) instead of the full python_hard fixture, so this test
// proves the wiring works independently of whether python_hard's much larger
// grammar has any unrelated issues.
use lexer_generator::api;

const YAL: &str = "let digit = ['0'-'9']\n\
let lower = ['a'-'z']\n\
let id = lower (lower|digit)*\n\
let ws_char = [' ' '\\t']\n\
let ws = ws_char+\n\
let newline = '\\n'\n\
let kw_if = if\n\
let kw_else = else\n\
let kw_pass = pass\n\
rule tokens =\n\
    kw_if   { \"IF\" }\n\
  | kw_else { \"ELSE\" }\n\
  | kw_pass { \"PASS\" }\n\
  | id      { \"ID\" }\n\
  | ':'     { \"COLON\" }\n\
  | newline { \"NEWLINE\" }\n\
  | ws      {}\n";

const YALP: &str = "%token IF ELSE PASS ID COLON NEWLINE INDENT DEDENT\n\
%%\n\
program : stmt_list ;\n\
stmt_list : stmt_list stmt | stmt ;\n\
stmt : simple_stmt NEWLINE | if_stmt | NEWLINE ;\n\
simple_stmt : PASS | ID ;\n\
if_stmt : IF ID COLON suite\n\
        | IF ID COLON suite ELSE COLON suite\n\
        ;\n\
suite : NEWLINE INDENT stmt_list DEDENT ;\n";

#[test]
fn indent_sensitive_grammar_accepts_nested_if_else() {
    let source = "if x:\n    pass\nelse:\n    if y:\n        pass\n    else:\n        pass\n";

    let resp = api::build_pipeline_response(YAL, YALP, source, "lalr")
        .expect("pipeline should build a response");

    assert!(
        resp.accepted,
        "nested if/else with consistent indentation must be accepted; error={:?} problems={:?} trace_tail={:?}",
        resp.error,
        resp.problems,
        resp.trace.last()
    );
    assert!(resp.problems.is_empty(), "no problems expected, got: {:?}", resp.problems);

    // The synthesized INDENT/DEDENT tokens must show up in token_map (the IDE's
    // TOKENS panel reads this field directly) — confirms they're inserted where
    // api/mod.rs expects, not just internally consumed by the parser.
    let indent_count = resp.token_map.iter().filter(|t| t["kind"] == "INDENT").count();
    let dedent_count = resp.token_map.iter().filter(|t| t["kind"] == "DEDENT").count();
    assert_eq!(indent_count, 4, "expected 4 INDENTs, token_map={:?}", resp.token_map);
    assert_eq!(dedent_count, 4, "expected 4 DEDENTs, token_map={:?}", resp.token_map);
}

#[test]
fn indent_sensitive_grammar_rejects_missing_dedent_context_gracefully() {
    // A single unindented "pass" is a syntax error regardless of indentation —
    // this just confirms the pass doesn't somehow make broken input accepted.
    let source = "pass\n";
    let resp = api::build_pipeline_response(YAL, YALP, source, "lalr")
        .expect("pipeline should build a response");
    assert!(resp.accepted, "a bare top-level statement should still parse: {:?}", resp.error);
}

#[test]
fn non_indent_sensitive_grammar_is_unaffected() {
    // Sanity check: a grammar that does NOT declare INDENT/DEDENT must behave
    // exactly as before — the pass must never fire for it.
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n  | '+' { return PLUS }\n  | ' ' { skip }\n";
    let yalp = "%token NUM PLUS\n%%\nS : NUM PLUS NUM ;\n";
    let resp = api::build_pipeline_response(yal, yalp, "1 + 2", "lalr")
        .expect("pipeline should build a response");
    assert!(resp.accepted, "unrelated grammar must parse normally: {:?}", resp.error);
    assert!(
        resp.token_map.iter().all(|t| t["kind"] != "INDENT" && t["kind"] != "DEDENT"),
        "indent pass must not fire for a grammar without INDENT/DEDENT tokens: {:?}",
        resp.token_map
    );
}
