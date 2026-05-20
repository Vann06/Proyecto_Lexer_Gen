// Pipeline end-to-end: archivo fuente → lexer → parser → árbol de derivación.
//
// Uso:
//   cargo run --bin test_pipeline -- <gramatica.yalp> <lexer.yal> <fuente> [--ll1|--lalr|--slr]
//
// Por defecto usa LALR(1). Con --ll1 usa LL(1), con --slr usa SLR(1).
//
// Flujo:
//   1. Construye la tabla del lexer compilando el .yal (mismas fases que main.rs).
//   2. Carga la gramática del .yalp y construye la tabla del parser.
//   3. Lee el archivo fuente y tokeniza con la Simulator.
//   4. Filtra tokens ignorables (Whitespace, Comment, Ignored).
//   5. Mapea Vec<Token> a Vec<ParseToken> usando el kind extraído del lexer.
//   6. Llama parse_tree y muestra el árbol en ASCII + escribe DOT a output/.

#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

#[path = "../spec/mod.rs"]
mod spec;
#[path = "../regex/mod.rs"]
mod regex;
#[path = "../automata/mod.rs"]
mod automata;
#[path = "../table/mod.rs"]
mod table;
#[path = "../runtime/mod.rs"]
mod runtime;
#[path = "../graph/mod.rs"]
mod graph;
#[path = "../error.rs"]
mod error;

use std::env;
use std::fs;

use analizador_sintactico::grammar::Grammar;
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::lr0::LR0Automaton;
use analizador_sintactico::lr1::LR1Automaton;
use analizador_sintactico::lalr::merge_by_core;
use analizador_sintactico::tables::LRTable;
use analizador_sintactico::parser_lr::LRParser;
use analizador_sintactico::ll1::LL1Parser;
use analizador_sintactico::parse_tree::{ParseToken, print_ascii, to_dot};

use crate::spec::parser::parse_yalex;
use crate::spec::expand::expand_definitions;
use crate::regex::parser::parse_regex;
use crate::runtime::simulator::{Simulator, LexResult, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode { LALR, SLR, LL1 }

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Uso: {} <gramatica.yalp> <lexer.yal> <fuente> [--ll1|--lalr|--slr]", args[0]);
        std::process::exit(1);
    }
    let yalp_path = &args[1];
    let yal_path  = &args[2];
    let src_path  = &args[3];
    let mode = args.iter().skip(4).find_map(|a| match a.as_str() {
        "--ll1"  => Some(Mode::LL1),
        "--lalr" => Some(Mode::LALR),
        "--slr"  => Some(Mode::SLR),
        _ => None,
    }).unwrap_or(Mode::LALR);

    println!("=== PIPELINE LEXER → PARSER → ÁRBOL ===");
    println!("  gramática : {}", yalp_path);
    println!("  lexer     : {}", yal_path);
    println!("  fuente    : {}", src_path);
    println!("  modo      : {:?}\n", mode);

    // ── 1. Construir tabla del lexer ────────────────────────────────────────
    let lexer_table = build_lexer_table(yal_path);
    println!("✓ Lexer construido ({} estados).\n", lexer_table.n_states);

    // ── 2. Leer fuente y tokenizar ──────────────────────────────────────────
    let source = fs::read_to_string(src_path).unwrap_or_else(|e| {
        eprintln!("Error al leer fuente: {}", e);
        std::process::exit(1);
    });
    let mut sim = Simulator::new(&lexer_table, &source);
    let mut raw_tokens: Vec<Token> = Vec::new();
    let mut lex_errors: Vec<String> = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => raw_tokens.push(t),
            LexResult::Error { lexeme, line, col } => {
                lex_errors.push(format!("línea {}:{} — '{}'", line, col, lexeme));
            }
            LexResult::EOF => break,
        }
    }
    if !lex_errors.is_empty() {
        eprintln!("⚠ Errores léxicos:");
        for e in &lex_errors { eprintln!("  {}", e); }
    }
    println!("✓ Lexer produjo {} tokens raw.", raw_tokens.len());

    // ── 3. Cargar gramática y filtrar tokens ────────────────────────────────
    let grammar = match mode {
        Mode::LALR | Mode::SLR => Grammar::parse_for_lr(yalp_path),
        Mode::LL1              => Grammar::parse_from_file(yalp_path),
    }.unwrap_or_else(|e| {
        eprintln!("Error al cargar gramática: {}", e);
        std::process::exit(1);
    });

    let parse_tokens: Vec<ParseToken> = raw_tokens.iter()
        .map(|t| ParseToken { kind: normalize_kind(&t.kind), lexeme: t.lexeme.clone() })
        .filter(|t| !is_ignored(&t.kind, &grammar))
        .collect();

    println!("✓ Tras filtrar ignorables: {} tokens al parser.", parse_tokens.len());
    println!("  tokens: {:?}\n",
             parse_tokens.iter().map(|t| t.kind.clone()).collect::<Vec<_>>());

    // ── 4. Construir parser y parsear ───────────────────────────────────────
    let tree = match mode {
        Mode::LALR => {
            let first_sets = calculate_first(&grammar);
            let lr1   = LR1Automaton::build(&grammar, &first_sets);
            let lalr  = merge_by_core(lr1);
            let table = LRTable::build_from_lalr(&lalr, &grammar);
            if !table.conflicts.is_empty() {
                println!("⚠ {} conflicto(s) en la tabla:", table.conflicts.len());
                for c in &table.conflicts { println!("   {}", c.describe()); }
            }
            let parser = LRParser::new(&table);
            parser.parse_tree(parse_tokens)
        }
        Mode::SLR => {
            let first_sets  = calculate_first(&grammar);
            let follow_sets = calculate_follow(&grammar, &first_sets);
            let lr0   = LR0Automaton::build(&grammar);
            let table = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
            if !table.conflicts.is_empty() {
                println!("⚠ {} conflicto(s) en la tabla:", table.conflicts.len());
                for c in &table.conflicts { println!("   {}", c.describe()); }
            }
            let parser = LRParser::new(&table);
            parser.parse_tree(parse_tokens)
        }
        Mode::LL1 => {
            let first_sets  = calculate_first(&grammar);
            let follow_sets = calculate_follow(&grammar, &first_sets);
            let parser = LL1Parser::build(&grammar, &first_sets, &follow_sets)
                .unwrap_or_else(|e| {
                    eprintln!("LL(1) no construible: {}", e);
                    std::process::exit(1);
                });
            parser.parse_tree(parse_tokens)
        }
    };

    match tree {
        Ok(t) => {
            println!("\n--- ÁRBOL DE DERIVACIÓN ---");
            print_ascii(&t);

            let dot = to_dot(&t);
            fs::create_dir_all("output").ok();
            let dot_path = format!("output/parse_tree_{}.dot", match mode {
                Mode::LALR => "lalr",
                Mode::SLR  => "slr",
                Mode::LL1  => "ll1",
            });
            if let Err(e) = fs::write(&dot_path, &dot) {
                eprintln!("✗ No se pudo escribir DOT: {}", e);
            } else {
                println!("\n✓ DOT escrito en {} (genera PNG con: dot -Tpng {} -o tree.png)",
                         dot_path, dot_path);
            }
        }
        Err(e) => {
            eprintln!("\n✗ Error de parseo: {}", e);
            std::process::exit(2);
        }
    }
}

/// Compila un .yal a una tabla de transición usando las mismas fases que main.rs.
fn build_lexer_table(yal_path: &str) -> crate::table::transition_table::TransitionTable {
    let yal_src = fs::read_to_string(yal_path).unwrap_or_else(|e| {
        eprintln!("Error al leer .yal: {}", e);
        std::process::exit(1);
    });
    let spec = parse_yalex(&yal_src).unwrap_or_else(|e| {
        eprintln!("Error al parsear .yal: {}", e);
        std::process::exit(1);
    });
    let expanded = expand_definitions(&spec);

    let mut id_counter = 0;
    let mut nfas = Vec::new();
    for rule in &expanded {
        let ast = parse_regex(&rule.pattern_expanded).unwrap_or_else(|e| {
            eprintln!("Error en regex '{}': {}", rule.pattern_expanded, e);
            std::process::exit(1);
        });
        let mut nfa = crate::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
        if let Some(fs) = nfa.states.get_mut(&nfa.end_state) {
            fs.accept_action = Some((rule.priority, rule.action_code.clone()));
        }
        nfas.push(nfa);
    }
    let master = crate::automata::nfa::combine_nfas(nfas, &mut id_counter);
    let dfa     = crate::automata::subset::build_dfa_from_nfa(&master);
    let min_dfa = crate::automata::minimize::minimize_dfa(&dfa);
    crate::table::transition_table::build(&min_dfa)
}

/// Normaliza el kind del lexer a UPPERCASE para que coincida con los tokens
/// declarados en el .yalp (que siguen la convención ALL_CAPS de yacc/bison).
/// Ejemplo: "Id" → "ID", "NumInt" → "NUMINT", "LParen" → "LPAREN".
fn normalize_kind(kind: &str) -> String {
    kind.to_uppercase()
}

/// Token a omitir antes de pasarlo al parser: whitespace, comentarios y los
/// tokens declarados como IGNORE en la gramática .yalp.
fn is_ignored(kind: &str, grammar: &Grammar) -> bool {
    let lower = kind.to_lowercase();
    if lower.contains("whitespace") || lower.contains("comment") || lower == "ignored" {
        return true;
    }
    grammar.ignores.contains(kind)
}
