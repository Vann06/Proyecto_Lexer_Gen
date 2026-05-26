#![allow(dead_code, unused_imports, unused_assignments)]
// Tests for YALex/YALP parsing and error handling.
#[path = "../src/error.rs"]
mod error;
#[path = "../src/spec/mod.rs"]
mod spec;
#[path = "../src/regex/mod.rs"]
mod regex;
#[path = "../src/analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use spec::parser::parse_yalex;
use regex::parser::parse_regex;
use analizador_sintactico::grammar::Grammar;

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
