// src/bin/test_lr1.rs — Tester interactivo para el parser LR(1)
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::lr1::{LR1Automaton, LR1Tables};
use std::io::{self, Write};

fn sym_name(s: &Symbol) -> &str {
    match s {
        Symbol::Terminal(t) | Symbol::NonTerminal(t) => t.as_str(),
    }
}

fn main() {
    println!("=== TESTER PARSER LR(1) ===");
    print!("Introduce la ruta del archivo .yalp: ");
    io::stdout().flush().unwrap();

    let mut path_buf = String::new();
    io::stdin().read_line(&mut path_buf).unwrap();
    let filepath = path_buf.trim();

    let grammar = match Grammar::parse_for_lr(filepath) {
        Ok(g) => {
            println!("\nGramatica '{}' cargada (sin transformaciones LL1).", filepath);
            g
        }
        Err(e) => {
            eprintln!("Error al cargar la gramatica: {}", e);
            return;
        }
    };

    // ── FIRST sets (necesarios para calcular lookaheads en la cerradura) ─────
    let first_sets = calculate_first(&grammar);

    // ── Autómata LR(1) ────────────────────────────────────────────────────────
    println!("\n--- CONSTRUYENDO AUTOMATA LR(1) ---");
    let automaton = LR1Automaton::build(&grammar, &first_sets);
    println!("Total de estados: {}", automaton.states.len());

    println!("\n--- DETALLE DE ESTADOS ---");
    for state in &automaton.states {
        if let Some((from, sym)) = &state.origin {
            println!(
                "Estado I{}: GOTO(I{}, {}) = I{}",
                state.id,
                from,
                sym_name(sym),
                state.id
            );
        } else {
            println!("Estado I0: (Estado Inicial)");
        }

        // Ordenar items: kernel primero, luego alfabético
        let mut sorted: Vec<&analizador_sintactico::lr1::LR1Item> =
            state.items.iter().collect();
        sorted.sort_by(|a, b| {
            let ak = a.dot_pos > 0 || a.head == automaton.start_head;
            let bk = b.dot_pos > 0 || b.head == automaton.start_head;
            match (ak, bk) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.cmp(b),
            }
        });

        for item in sorted {
            println!("    {}", item.display());
        }
        println!();
    }

    // ── Tablas ACTION / GOTO ──────────────────────────────────────────────────
    println!("--- CONSTRUYENDO TABLAS ACTION / GOTO ---");
    let tables = LR1Tables::build(&automaton);

    if tables.conflicts.is_empty() {
        println!("Sin conflictos: la gramatica ES LR(1).");
    } else {
        println!("CONFLICTOS DETECTADOS ({}):", tables.conflicts.len());
        for c in &tables.conflicts {
            println!("  ! {}", c);
        }
    }

    // ── Imprimir tabla ACTION ─────────────────────────────────────────────────
    println!("\n--- TABLA ACTION ---");

    // Recoger todas las columnas terminales que aparecen en ACTION
    let mut terminals: Vec<String> = tables
        .action
        .keys()
        .map(|(_, t)| t.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    terminals.sort();

    let col_w = 18usize;
    print!("| {:>6} |", "Estado");
    for t in &terminals {
        print!(" {:<width$} |", t, width = col_w);
    }
    println!();
    print!("|--------|");
    for _ in &terminals {
        print!("{:-<width$}|", "", width = col_w + 2);
    }
    println!();

    let mut state_ids: Vec<usize> = (0..automaton.states.len()).collect();
    state_ids.sort();

    for sid in &state_ids {
        print!("| {:>6} |", format!("I{}", sid));
        for t in &terminals {
            let cell = match tables.action.get(&(*sid, t.clone())) {
                Some(a) => a.display(),
                None => String::new(),
            };
            print!(" {:<width$} |", cell, width = col_w);
        }
        println!();
    }

    // ── Imprimir tabla GOTO ───────────────────────────────────────────────────
    println!("\n--- TABLA GOTO ---");

    let mut non_terminals: Vec<String> = tables
        .goto
        .keys()
        .map(|(_, nt)| nt.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    non_terminals.sort();

    if non_terminals.is_empty() {
        println!("(sin entradas GOTO)");
    } else {
        print!("| {:>6} |", "Estado");
        for nt in &non_terminals {
            print!(" {:<10} |", nt);
        }
        println!();
        print!("|--------|");
        for _ in &non_terminals {
            print!("------------|");
        }
        println!();

        for sid in &state_ids {
            let row_has_entry = non_terminals
                .iter()
                .any(|nt| tables.goto.contains_key(&(*sid, nt.clone())));
            if !row_has_entry {
                continue;
            }
            print!("| {:>6} |", format!("I{}", sid));
            for nt in &non_terminals {
                let cell = match tables.goto.get(&(*sid, nt.clone())) {
                    Some(dest) => format!("I{}", dest),
                    None => String::new(),
                };
                print!(" {:<10} |", cell);
            }
            println!();
        }
    }

    // ── Loop de prueba de parseo ──────────────────────────────────────────────
    if tables.conflicts.is_empty() {
        loop {
            println!("\n--- PRUEBA DE PARSEO LR(1) ---");
            println!("Introduce tokens separados por espacios (o 'exit' para salir):");
            print!("> ");
            io::stdout().flush().unwrap();

            let mut line = String::new();
            if io::stdin().read_line(&mut line).unwrap() == 0 {
                break; // EOF
            }
            let line = line.trim();

            if line.is_empty() {
                continue;
            }
            if line == "exit" {
                break;
            }

            let tokens: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            println!("Parseando: {:?}", tokens);

            match tables.parse(tokens) {
                Ok(()) => println!("  La cadena es VALIDA para esta gramatica."),
                Err(e) => eprintln!("  Error: {}", e),
            }
        }
    } else {
        println!("\nNo se ejecuta parseo: hay conflictos en la tabla.");
    }
}
