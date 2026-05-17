// src/bin/test_lalr.rs
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::lr1::LR1Automaton;
use analizador_sintactico::lalr::merge_by_core;
use analizador_sintactico::tables::{LRTable, print_productions};
use analizador_sintactico::parser_lr::{LRParser, print_trace};
use analizador_sintactico::parse_tree::{ParseToken, print_ascii, to_dot};
use std::io::{self, Write};

fn main() {
    println!("=== PARSER LALR(1) ===");
    print!("Introduce la ruta del archivo .yalp: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let filepath = input.trim();

    let grammar = match Grammar::parse_for_lr(filepath) {
        Ok(g)  => g,
        Err(e) => { eprintln!("Error: {}", e); return; }
    };

    println!("\nGramática '{}' cargada (sin transformaciones).", filepath);
    println!("\n--- PRODUCCIONES ---");
    for prod in &grammar.productions {
        for body in &prod.bodies {
            let body_str: String = if body.is_empty() {
                "ε".to_string()
            } else {
                body.iter().map(|s| match s {
                    Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
                }).collect::<Vec<_>>().join(" ")
            };
            println!("  {} → {}", prod.head, body_str);
        }
    }

    // ─── PASO 1: FIRST sets ───────────────────────────────────────────────
    let first_sets = calculate_first(&grammar);

    // ─── PASO 2: Autómata LR(1) canónico ─────────────────────────────────
    println!("\n--- CONSTRUYENDO AUTÓMATA LR(1) CANÓNICO ---");
    let lr1 = LR1Automaton::build(&grammar, &first_sets);
    println!("Total de estados LR(1): {}", lr1.states.len());

    println!("\n--- DETALLE DE ESTADOS LR(1) ---");
    for state in &lr1.states {
        print_lr1_state(state, &lr1.start_head);
    }

    // ─── PASO 3: Fusión por core → LALR ──────────────────────────────────
    println!("\n--- FUSIONANDO POR CORE (LR(1) → LALR) ---");
    let lr1_count = lr1.states.len();
    let lalr = merge_by_core(lr1);
    let lalr_count = lalr.states.len();
    println!("Estados LR(1): {}  →  Estados LALR: {}", lr1_count, lalr_count);

    println!("\nMapa de fusión:");
    for state in &lalr.states {
        println!("  LALR I{:>2}  ←  LR1 {:?}", state.id, state.lr1_ids);
    }

    println!("\n--- DETALLE DE ESTADOS LALR ---");
    for state in &lalr.states {
        print_lalr_state(state, &lalr.start_head);
    }

    // ─── PASO 4: Tabla ACTION/GOTO ────────────────────────────────────────
    println!("\n--- TABLA ACTION/GOTO ---");
    let table = LRTable::build_from_lalr(&lalr, &grammar);

    print_productions(&grammar);
    println!();
    table.print_table(&grammar);

    if table.conflicts.is_empty() {
        println!("\n✓ Gramática LALR(1) aceptada — sin conflictos.");
    } else {
        println!("\n⚠  {} conflicto(s) detectado(s):", table.conflicts.len());
        for c in &table.conflicts {
            println!("   {}", c.describe());
        }
    }

    // ─── PASO 5: Loop de parseo interactivo ──────────────────────────────
    let parser = LRParser::new(&table);

    println!("\n--- PRUEBA DE PARSEO INTERACTIVO ---");
    println!("Escribe una secuencia de tokens separados por espacios ('exit' para salir).");
    println!("Tokens disponibles: {:?}", {
        let mut t: Vec<_> = grammar.tokens.iter().collect();
        t.sort();
        t
    });

    loop {
        print!("\n> ");
        io::stdout().flush().unwrap();
        let mut line = String::new();
        io::stdin().read_line(&mut line).unwrap();
        let line = line.trim();

        if line == "exit" || line.is_empty() {
            break;
        }

        let kinds: Vec<String> = line.split_whitespace().map(String::from).collect();
        match parser.parse(kinds.clone()) {
            Ok(trace) => {
                print_trace(&trace);
                println!("✓ Cadena VÁLIDA.");

                // Construir e imprimir el árbol de derivación
                let tokens = ParseToken::from_kinds(kinds);
                match parser.parse_tree(tokens) {
                    Ok(tree) => {
                        println!("\n--- ÁRBOL DE DERIVACIÓN ---");
                        print_ascii(&tree);

                        // Exportar a DOT para visualización
                        let dot = to_dot(&tree);
                        if let Err(e) = std::fs::create_dir_all("output") {
                            eprintln!("(no se pudo crear output/: {})", e);
                        } else if let Err(e) = std::fs::write("output/parse_tree_lalr.dot", &dot) {
                            eprintln!("(no se pudo escribir DOT: {})", e);
                        } else {
                            println!("\n(DOT escrito en output/parse_tree_lalr.dot)");
                        }
                    }
                    Err(e) => println!("✗ Error al construir árbol: {}", e),
                }
            }
            Err(e) => {
                println!("✗ {}", e);
            }
        }
    }
}

fn print_lr1_state(state: &analizador_sintactico::lr1::LR1State, start_head: &str) {
    if let Some((from, sym)) = &state.origin {
        let sym_str = match sym { Symbol::Terminal(t) | Symbol::NonTerminal(t) => t };
        println!("Estado I{}: Goto(I{}, {})", state.id, from, sym_str);
    } else {
        println!("Estado I0: (inicial)");
    }

    let mut sorted: Vec<_> = state.items.iter().collect();
    sorted.sort_by(|a, b| {
        let ak = a.dot_pos > 0 || a.head == start_head;
        let bk = b.dot_pos > 0 || b.head == start_head;
        if ak == bk { a.cmp(b) } else if ak { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
    });

    for item in sorted {
        println!("    [{}, {}]", fmt_item_body(&item.head, &item.body, item.dot_pos), item.lookahead);
    }
    println!();
}

fn print_lalr_state(state: &analizador_sintactico::lalr::LALRState, start_head: &str) {
    if let Some((from, sym)) = &state.origin {
        let sym_str = match sym { Symbol::Terminal(t) | Symbol::NonTerminal(t) => t };
        println!("Estado I{}: Goto(I{}, {})", state.id, from, sym_str);
    } else {
        println!("Estado I0: (inicial)");
    }

    let mut sorted = state.items.clone();
    sorted.sort_by(|a, b| {
        let ak = a.dot_pos > 0 || a.head == *start_head;
        let bk = b.dot_pos > 0 || b.head == *start_head;
        if ak == bk { a.head.cmp(&b.head).then(a.dot_pos.cmp(&b.dot_pos)) }
        else if ak { std::cmp::Ordering::Less } else { std::cmp::Ordering::Greater }
    });

    for item in &sorted {
        let mut las: Vec<_> = item.lookaheads.iter().collect();
        las.sort();
        println!("    [{}, {:?}]", fmt_item_body(&item.head, &item.body, item.dot_pos), las);
    }
    println!();
}

fn fmt_item_body(head: &str, body: &[Symbol], dot_pos: usize) -> String {
    let mut s = format!("{} →", head);
    for (i, sym) in body.iter().enumerate() {
        if i == dot_pos { s.push_str(" ."); }
        match sym {
            Symbol::Terminal(t) | Symbol::NonTerminal(t) => s.push_str(&format!(" {}", t)),
        }
    }
    if dot_pos == body.len() { s.push_str(" ."); }
    s
}
