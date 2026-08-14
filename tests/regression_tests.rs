// Regression tests for the bugs found in the pre-refactor audit (see the plan file).
// Each test documents the concrete failure scenario for one bug and is expected to
// FAIL until that bug is fixed. Keep the bug id (Ax) in the test name / comment so the
// fix commit can be matched back to this file.
use lexer_generator::{sintactico, api, lexico};

use sintactico::gramatica::first::calculate_first;
use sintactico::gramatica::follow::calculate_follow;
use sintactico::gramatica::grammar::{Grammar, Symbol};
use sintactico::runtime::ll1::LL1Parser;
use lexico::table::transition_table::TransitionTable;
use lexico::runtime::simulator::{LexResult, Simulator};

/// Rebuilds a lexer transition table from a `.yal` source, mirroring
/// `api::build_lexer_table_from_str` (private to that module) so this test file can
/// drive the lexer pipeline directly without touching HTTP/JSON plumbing.
fn build_lexer_table(yal_src: &str) -> TransitionTable {
    let spec_ir = lexico::spec::parser::parse_yalex(yal_src).expect("valid .yal for test fixture");
    let expanded = lexico::spec::expand::expand_definitions(&spec_ir);

    let mut id_counter = 0usize;
    let mut nfas = Vec::new();
    for rule in &expanded {
        let ast = lexico::regex::parser::parse_regex(&rule.pattern_expanded)
            .unwrap_or_else(|e| panic!("regex '{}' failed to parse: {}", rule.pattern_expanded, e));
        let mut nfa = lexico::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        nfas.push(nfa);
    }
    let master = lexico::automata::nfa::combine_nfas(nfas, &mut id_counter);
    let dfa = lexico::automata::subset::build_dfa_from_nfa(&master);
    let min_dfa = lexico::automata::minimize::minimize_dfa(&dfa);
    lexico::table::transition_table::build(&min_dfa)
}

// ─────────────────────────────────────────────────────────────────────────────
// A3 — auto-augmentation heuristic (`contains("prima")`) corrupts the LALR table
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a3_augmentation_heuristic_does_not_corrupt_table_for_prima_start_symbol() {
    // "prima" is an ordinary Spanish word (as in `expresion_primaria`) — nothing about
    // this grammar is pre-augmented, but the substring heuristic in lr1.rs/lr0.rs
    // treats it as if it were.
    let content = "%token NUM PLUS\n%%\nprima : prima PLUS NUM | NUM ;\n";

    // A bare NUM is a legitimately complete derivation of `prima` (via the second
    // alternative) — it SHOULD be accepted; this is just a sanity check, not the
    // regression probe (a single-token input can't distinguish premature accept
    // from correct accept, since there's nothing left to parse either way).
    let resp = api::build_parse_response(content, vec!["NUM".into()], "lalr")
        .expect("grammar should compile");
    assert!(resp.accepted, "a bare NUM should derive 'prima' via NUM alone: {:?}", resp.trace);

    // The real probe: 'NUM PLUS NUM' requires reducing the first NUM up to `prima`
    // as an INTERMEDIATE step (to build the left-recursive 'prima PLUS NUM'
    // pattern) before the PLUS/second NUM are even shifted. The buggy heuristic
    // turned every complete item with head == "prima" into Accept, so that first,
    // intermediate reduction fired Accept prematurely instead of Reduce-and-continue,
    // and the trailing 'PLUS NUM' was rejected as unexpected leftover input.
    let resp2 = api::build_parse_response(
        content,
        vec!["NUM".into(), "PLUS".into(), "NUM".into()],
        "lalr",
    )
    .expect("grammar should compile");
    assert!(
        resp2.accepted,
        "'NUM PLUS NUM' must be accepted by 'prima : prima PLUS NUM | NUM ;' — \
         if it's rejected, the augmentation heuristic corrupted the table (A3): {:?}",
        resp2.error
    );
    let acc_positions: Vec<usize> = resp2.trace.iter().enumerate()
        .filter(|(_, s)| s["action"] == "acc")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(
        acc_positions, vec![resp2.trace.len() - 1],
        "'acc' must appear exactly once, as the LAST step — a premature accept \
         mid-derivation is exactly the A3 symptom: {:?}",
        resp2.trace
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// A5 — `$` usable as a token name causes an index-out-of-bounds panic in the driver
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a5_dollar_token_name_is_rejected_at_parse_time() {
    let content = "%token ID $\n%%\nS : ID $ ;\n";
    let result = Grammar::parse_for_lr_from_str(content);
    assert!(
        result.is_err(),
        "'$' is the EOF sentinel used internally by the LR driver; declaring it as an \
         ordinary token must be rejected, otherwise ACTION[s,\"$\"] can be a real Shift \
         and the driver reads past the end of the input (A5)"
    );
}

#[test]
fn a5_dollar_token_name_does_not_panic_the_driver() {
    let content = "%token ID $\n%%\nS : ID $ ;\n";
    // Regardless of whether grammar parsing rejects '$' outright, the driver itself
    // must never panic on malformed/adversarial input reachable over HTTP.
    let result = std::panic::catch_unwind(|| {
        api::build_parse_response(content, vec!["ID".to_string()], "lalr")
    });
    assert!(result.is_ok(), "parsing must not panic the request thread (A5)");
}

// ─────────────────────────────────────────────────────────────────────────────
// A7 — `%nonassoc` error cells can be silently resurrected as a Reduce
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a7_nonassoc_error_cell_is_not_resurrected_by_a_later_reduce() {
    use sintactico::tablas::{insert_action, Action, Conflict, PrecInfo};
    use std::collections::{HashMap, HashSet};

    let mut action: HashMap<(usize, String), Action> = HashMap::new();
    let mut conflicts: Vec<Conflict> = Vec::new();
    let mut nonassoc_errors: HashSet<(usize, String)> = HashSet::new();
    let prod_index: HashMap<(String, Vec<Symbol>), usize> = HashMap::new();

    let mut prec_map = HashMap::new();
    prec_map.insert(
        "EQ".to_string(),
        PrecInfo {
            level: 0,
            assoc: sintactico::gramatica::grammar::Associativity::NonAssoc,
        },
    );

    // 1. A shift on EQ is already in the table (from a transition).
    insert_action(&mut action, &mut conflicts, &mut nonassoc_errors, 0, "EQ".to_string(), Action::Shift(5), &prod_index, &prec_map);

    // 2. A complete item reduces on EQ too, body ends in EQ (same precedence level) ->
    //    nonassoc conflict -> the cell should become an explicit error (not just absent).
    let body_a = vec![Symbol::Terminal("EQ".to_string())];
    insert_action(
        &mut action, &mut conflicts, &mut nonassoc_errors, 0, "EQ".to_string(),
        Action::Reduce { head: "A".to_string(), body: body_a },
        &prod_index, &prec_map,
    );

    // 3. A second, unrelated complete item also reduces on EQ in the same state
    //    (e.g. another production whose lookahead set includes EQ). Because the error
    //    cell today is encoded as "absence of a key", this call sees no existing entry
    //    and silently re-populates the cell with a Reduce — losing the nonassoc error.
    let body_b = vec![Symbol::Terminal("ID".to_string())];
    insert_action(
        &mut action, &mut conflicts, &mut nonassoc_errors, 0, "EQ".to_string(),
        Action::Reduce { head: "B".to_string(), body: body_b },
        &prod_index, &prec_map,
    );

    assert!(
        !matches!(action.get(&(0, "EQ".to_string())), Some(Action::Reduce { .. })),
        "the nonassoc error cell at (state 0, 'EQ') was resurrected as a Reduce by a \
         later, unrelated insert_action call (A7): {:?}",
        action.get(&(0, "EQ".to_string()))
    );
    assert!(
        nonassoc_errors.contains(&(0, "EQ".to_string())),
        "the nonassoc conflict must be recorded explicitly so it can't be resurrected"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// A8 — left-factoring never terminates on duplicate/empty alternatives
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a8_left_factoring_terminates_on_duplicate_alternatives() {
    let content = "%token ID\n%%\nS : E ;\nE : ID | ID ;\n";

    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = Grammar::parse_for_ll1_from_str(content);
        let _ = tx.send(result.is_ok());
    });

    match rx.recv_timeout(std::time::Duration::from_secs(5)) {
        Ok(_) => { /* terminated — good, whatever the outcome */ }
        Err(_) => panic!(
            "left-factoring did not terminate within 5s on 'E : ID | ID ;' — \
             infinite loop factoring AUX -> ε | ε forever (A8)"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// A9 — eliminate_ambiguity silently drops a repeated-head production block
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a9_eliminate_ambiguity_merges_repeated_head_blocks_instead_of_dropping() {
    // Legal yacc-style grammar: the same non-terminal split across two blocks.
    let content = "%token ID PLUS\n%%\nE : E PLUS T ;\nE : T ;\nT : ID ;\n";
    let grammar = Grammar::parse_for_ll1_from_str(content)
        .expect("grammar should parse and transform for LL(1)");

    let first_sets = calculate_first(&grammar);
    let follow_sets = calculate_follow(&grammar, &first_sets);
    let parser = LL1Parser::build(&grammar, &first_sets, &follow_sets)
        .expect("LL(1) table should build without conflicts");

    let result = parser.parse(vec!["ID".to_string()]);
    assert!(
        result.is_ok(),
        "a bare 'ID' must be derivable from E via the second block 'E : T ;', but it \
         was silently dropped when eliminate_ambiguity deduplicated by head (A9): {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// A11 — quote-stripping in character classes corrupts already-decoded escapes
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a11_negated_class_keeps_escaped_quote_in_the_excluded_set() {
    // Mirrors examples/lexer/ejemplo_c.yal's string-literal rule.
    let yal = "rule tokens =\n  | \\\"[^\\\"\\n\\r]*\\\" { return STR }\n";
    let table = build_lexer_table(yal);
    let mut sim = Simulator::new(&table, "\"a\", \"b\"");

    let t1 = sim.next_token();
    match t1 {
        LexResult::Token(tok) => assert_eq!(
            tok.lexeme, "\"a\"",
            "the first string literal must stop at its own closing quote; if the \
             negated class [^\"\\n\\r] lost the '\"' from its excluded set, maximal \
             munch swallows past it into the next literal (A11): got {:?}",
            tok.lexeme
        ),
        other => panic!("expected a STR token, got {:?}", other),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// A6 — line/col are not restored on maximal-munch backtracking
// ─────────────────────────────────────────────────────────────────────────────
#[test]
fn a6_simulator_restores_line_col_on_maximal_munch_backtrack() {
    let yal = "let digit = ['0'-'9']\n\
               rule tokens =\n\
               \x20 | digit+ '.' digit+ { return FLOAT }\n\
               \x20 | digit+            { return INT }\n\
               \x20 | '.'               { return DOT }\n";
    let table = build_lexer_table(yal);
    // "12.x": FLOAT is attempted speculatively (12.?) but fails at 'x' (not a digit),
    // so the driver backtracks to the last accepting state: INT "12".
    let mut sim = Simulator::new(&table, "12.x");

    let t1 = sim.next_token();
    match &t1 {
        LexResult::Token(tok) => assert_eq!(tok.lexeme, "12"),
        other => panic!("expected INT '12', got {:?}", other),
    }

    let t2 = sim.next_token();
    match t2 {
        LexResult::Token(tok) => {
            assert_eq!(tok.lexeme, ".");
            assert_eq!(tok.line, 1);
            assert_eq!(
                tok.col, 3,
                "after backtracking from the speculative FLOAT scan, the '.' token \
                 must start at col 3 (right after '12'); an inflated column means \
                 line/col were not rolled back together with pos (A6)"
            );
        }
        other => panic!("expected DOT '.', got {:?}", other),
    }
}
