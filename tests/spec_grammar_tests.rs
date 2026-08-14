// Tests for YALex/YALP parsing and error handling.
use lexer_generator::{sintactico, lexico};

use lexico::spec::parser::parse_yalex;
use lexico::regex::parser::parse_regex;
use sintactico::gramatica::grammar::Grammar;

#[test]
fn yalex_invalid_definition_reports_error() {
    let src = "let digit ['0'-'9']\nrule tokens =\n  | digit { return NUM }\n";
    let err = parse_yalex(src).expect_err("should reject invalid definition");
    let msg = format!("{}", err);
    assert!(msg.contains("definición inválida"), "expected InvalidDefinition error");
}

#[test]
fn yalex_unclosed_header_reports_error() {
    let src = "{\n int x;\n\nlet digit = ['0'-'9']\nrule tokens =\n  | digit { return NUM }\n";
    let err = parse_yalex(src).expect_err("should reject unclosed header");
    let msg = format!("{}", err);
    assert!(msg.contains("header no cerrado"), "expected header close error");
}

#[test]
fn regex_unclosed_quote_reports_error() {
    let err = parse_regex("'abc").expect_err("should reject unterminated string");
    let msg = format!("{}", err);
    assert!(msg.contains("Comillas simples no cerradas"));
}

#[test]
fn yalp_missing_separator_reports_error() {
    let src = "%token ID\nS : ID ;\n";
    let err = Grammar::parse_for_lr_from_str(src).expect_err("should require %%");
    assert!(err.contains("%%"));
}

#[test]
fn yalp_invalid_production_line_has_line_number() {
    let src = "%token ID\n%%\nS ID ;\n";
    let err = Grammar::parse_for_lr_from_str(src).expect_err("should reject invalid production");
    assert!(err.contains("línea"), "should include line number");
    assert!(err.contains("falta ':'"), "should mention missing ':'");
}

#[test]
fn yalp_unknown_nonterminal_reports_error() {
    let src = "%token ID\n%%\nS : X ;\n";
    let err = Grammar::parse_for_lr_from_str(src).expect_err("should reject undefined nonterminal");
    assert!(err.contains("No-Terminal"), "should mention undefined nonterminal");
}

// A13 — the module header advertises both /* */ and // comments, but only the
// former was ever stripped; a `//` line in the productions section used to get
// absorbed into the next block and could end up as the reported start_symbol.
#[test]
fn yalp_line_comment_is_stripped_not_absorbed_into_next_production() {
    let src = "%token ID\n// gramática de expresiones\n%%\nE : ID ;\n";
    let grammar = Grammar::parse_for_lr_from_str(src)
        .expect("a // comment before %% must not break parsing");
    assert_eq!(grammar.start_symbol, "E", "start_symbol should not include the comment text");
}

#[test]
fn yalp_line_comment_inside_productions_section_is_ignored() {
    let src = "%token ID PLUS\n%%\n// suma de dos identificadores\nE : ID PLUS ID ;\n";
    let grammar = Grammar::parse_for_lr_from_str(src)
        .expect("a // comment inside the productions section must not break parsing");
    assert_eq!(grammar.productions.len(), 1);
    assert_eq!(grammar.productions[0].head, "E");
}
