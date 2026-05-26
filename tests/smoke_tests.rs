#![allow(dead_code, unused_imports, unused_assignments)]
// Integración LALR(1) — usa #[path] ya que no hay lib.rs
#[path = "../src/analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::Grammar;
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::lr1::LR1Automaton;
use analizador_sintactico::lr0::LR0Automaton;
use analizador_sintactico::lalr::merge_by_core;
use analizador_sintactico::tables::LRTable;
use analizador_sintactico::parser_lr::{LRParser, ParseStep};
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::ll1::LL1Parser;
use analizador_sintactico::parse_tree::{ParseToken, to_dot};

const EXPR_YALP: &str = "examples/grammar/expr_left_recursive.yalp";
const CC_YALP:   &str = "examples/grammar/lalr_cc.yalp";
const PREC_YALP: &str = "examples/grammar/expr_ambiguous_prec.yalp";

// ─── Test 1 ──────────────────────────────────────────────────────────────────
#[test]
fn lr1_expr_grammar_builds() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    assert!(lr1.states.len() > 0, "debe haber al menos un estado");
}

// ─── Test 2 ──────────────────────────────────────────────────────────────────
// La gramática S→CC produce 10 estados LR(1) y 7 LALR.
#[test]
fn lalr_cc_fuses_states() {
    let grammar    = Grammar::parse_for_lr(CC_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    assert_eq!(lr1.states.len(), 10, "LR(1) debe tener 10 estados");

    let lalr = merge_by_core(lr1);
    assert_eq!(lalr.states.len(), 7, "LALR debe tener 7 estados");
}

// ─── Test 3 ──────────────────────────────────────────────────────────────────
#[test]
fn lalr_expr_table_has_no_conflicts() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);

    assert!(
        table.conflicts.is_empty(),
        "conflictos: {:?}",
        table.conflicts.iter().map(|c| c.describe()).collect::<Vec<_>>()
    );
}

// ─── Test 4 ──────────────────────────────────────────────────────────────────
#[test]
fn lalr_parses_valid_expression() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);
    let parser     = LRParser::new(&table);

    let tokens = vec!["ID".to_string(), "PLUS".to_string(), "ID".to_string(),
                      "STAR".to_string(), "ID".to_string()];
    let trace = parser.parse(tokens).expect("debe parsear correctamente");
    assert!(matches!(trace.last(), Some(ParseStep::Accept)));
}

// ─── Test 5 ──────────────────────────────────────────────────────────────────
#[test]
fn lalr_rejects_invalid_expression() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);
    let parser     = LRParser::new(&table);

    let tokens = vec!["ID".to_string(), "PLUS".to_string()];
    assert!(parser.parse(tokens).is_err());
}

// ─── Test 7 ──────────────────────────────────────────────────────────────────
// Gramática ambigua con %left/%right: los conflictos deben resolverse por
// precedencia sin necesidad de estratificar la gramática.
#[test]
fn lalr_precedence_resolves_conflicts() {
    let grammar    = Grammar::parse_for_lr(PREC_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);

    assert!(
        table.conflicts.is_empty(),
        "con %left/%right no debe haber conflictos sin resolver: {:?}",
        table.conflicts.iter().map(|c| c.describe()).collect::<Vec<_>>()
    );
}

// ─── Test 8 ──────────────────────────────────────────────────────────────────
// LALR construye el árbol de derivación bottom-up correcto para ID PLUS ID.
#[test]
fn lalr_builds_parse_tree() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);
    let parser     = LRParser::new(&table);

    let tokens = ParseToken::from_kinds(vec![
        "ID".to_string(), "PLUS".to_string(), "ID".to_string(),
    ]);
    let tree = parser.parse_tree(tokens).expect("debe construir árbol");

    // Raíz: e (símbolo inicial). e → e PLUS t → t PLUS t → f PLUS f → ID PLUS ID
    assert_eq!(tree.symbol, "e");
    // El árbol debe tener al menos un nodo PLUS en alguna parte
    assert!(
        find_symbol(&tree, "PLUS"),
        "el árbol LALR debe contener un nodo PLUS"
    );
    // Debe haber al menos dos hojas ID
    assert_eq!(count_symbol(&tree, "ID"), 2);
}

// ─── Test 9 ──────────────────────────────────────────────────────────────────
// LL(1) construye el árbol top-down para una gramática simple.
#[test]
fn ll1_builds_parse_tree() {
    let grammar     = Grammar::parse_from_file(EXPR_YALP).expect("debe cargar");
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let parser      = LL1Parser::build(&grammar, &first_sets, &follow_sets)
        .expect("LL(1) debe construirse");

    let tokens = ParseToken::from_kinds(vec![
        "ID".to_string(), "PLUS".to_string(), "ID".to_string(),
    ]);
    let tree = parser.parse_tree(tokens).expect("debe construir árbol");

    // La raíz es el símbolo inicial (tras eliminación de recursión sigue siendo "e")
    assert_eq!(tree.symbol, "e");
    assert!(find_symbol(&tree, "PLUS"), "árbol LL(1) debe contener PLUS");
    assert_eq!(count_symbol(&tree, "ID"), 2);
}

// ─── Test 10 ─────────────────────────────────────────────────────────────────
// to_dot produce un grafo válido.
#[test]
fn parse_tree_dot_export() {
    let grammar    = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets = calculate_first(&grammar);
    let lr1        = LR1Automaton::build(&grammar, &first_sets);
    let lalr       = merge_by_core(lr1);
    let table      = LRTable::build_from_lalr(&lalr, &grammar);
    let parser     = LRParser::new(&table);

    let tokens = ParseToken::from_kinds(vec!["ID".to_string()]);
    let tree = parser.parse_tree(tokens).expect("debe parsear");
    let dot = to_dot(&tree);

    assert!(dot.starts_with("digraph ParseTree"), "DOT inicia con digraph ParseTree");
    assert!(dot.contains("ID"), "DOT debe mencionar el terminal ID");
    assert!(dot.contains("->"), "DOT debe contener al menos una arista");
    assert!(dot.trim_end().ends_with('}'), "DOT cierra con '}}'");
}

// ─── Helpers para tests de árbol ─────────────────────────────────────────────
fn find_symbol(node: &analizador_sintactico::parse_tree::ParseNode, sym: &str) -> bool {
    if node.symbol == sym { return true; }
    node.children.iter().any(|c| find_symbol(c, sym))
}

fn count_symbol(node: &analizador_sintactico::parse_tree::ParseNode, sym: &str) -> usize {
    let here = if node.symbol == sym { 1 } else { 0 };
    here + node.children.iter().map(|c| count_symbol(c, sym)).sum::<usize>()
}

// ─── Test 6 ──────────────────────────────────────────────────────────────────
#[test]
fn lalr_left_recursion_preserved() {
    let grammar = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");

    let has_left_recursion = grammar.productions.iter().any(|prod| {
        prod.bodies.iter().any(|body| {
            body.first()
                .map(|s| match s {
                    analizador_sintactico::grammar::Symbol::NonTerminal(nt) => nt == &prod.head,
                    _ => false,
                })
                .unwrap_or(false)
        })
    });

    assert!(has_left_recursion, "parse_for_lr debe preservar la recursión izquierda");
}

// ─── Test 11 ─────────────────────────────────────────────────────────────────
// SLR(1) construye tabla sin conflictos para la gramática de expresiones.
// La gramática expr_left_recursive no tiene ambigüedad, por lo que SLR y LALR
// deben producir tablas equivalentes (sin conflictos).
#[test]
fn slr_expr_table_has_no_conflicts() {
    let grammar     = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let lr0         = LR0Automaton::build(&grammar);
    let table       = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);

    assert!(
        table.conflicts.is_empty(),
        "SLR no debe tener conflictos en expr_left_recursive: {:?}",
        table.conflicts.iter().map(|c| c.describe()).collect::<Vec<_>>()
    );
}

// ─── Test 12 ─────────────────────────────────────────────────────────────────
// SLR(1) parsea y construye árbol correctamente para ID PLUS ID.
#[test]
fn slr_parses_and_builds_tree() {
    let grammar     = Grammar::parse_for_lr(EXPR_YALP).expect("debe cargar");
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let lr0         = LR0Automaton::build(&grammar);
    let table       = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
    let parser      = LRParser::new(&table);

    let tokens = ParseToken::from_kinds(vec![
        "ID".to_string(), "PLUS".to_string(), "ID".to_string(),
    ]);
    let tree = parser.parse_tree(tokens).expect("SLR debe parsear ID PLUS ID");

    assert_eq!(tree.symbol, "e");
    assert!(find_symbol(&tree, "PLUS"), "árbol SLR debe contener PLUS");
    assert_eq!(count_symbol(&tree, "ID"), 2);
}

// ─── Test 13 ─────────────────────────────────────────────────────────────────
// SLR tiene MÁS conflictos que LALR en la gramática ambigua con precedencia.
// La gramática expr_ambiguous_prec tiene conflictos S/R que LALR resuelve por
// lookahead preciso; SLR los ve igualmente pero los resuelve por precedencia.
#[test]
fn slr_ambiguous_resolved_by_precedence() {
    let grammar     = Grammar::parse_for_lr(PREC_YALP).expect("debe cargar");
    let first_sets  = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let lr0         = LR0Automaton::build(&grammar);
    let table       = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);

    assert!(
        table.conflicts.is_empty(),
        "SLR con %left/%right no debe dejar conflictos sin resolver: {:?}",
        table.conflicts.iter().map(|c| c.describe()).collect::<Vec<_>>()
    );
}
