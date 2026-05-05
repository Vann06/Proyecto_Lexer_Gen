// src/bin/test_parser.rs
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::lr0::LR0Automaton;
use std::io::{self, Write};

fn main() {
    println!("=== GENERADOR YAPar ===");
    print!("Introduce la ruta del archivo .yalp a evaluar: ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let filepath = input.trim();

    // Usamos parse_for_lr() en lugar de parse_from_file() para que la gramática
    // NO sea transformada (sin left-factoring ni eliminación de recursión izquierda).
    // Los parsers LR(0) manejan prefijos comunes y recursión izquierda de forma
    // nativa — transformar la gramática antes crea no-terminales artificiales
    // (_factN) que distorsionan los estados del autómata.
    match Grammar::parse_for_lr(filepath) {
        Ok(grammar) => {
            println!("\nGramática '{}' cargada exitosamente (sin transformaciones LL(1)).", filepath);

            println!("\n--- CONSTRUYENDO AUTÓMATA LR(0) ---");
            let automata = LR0Automaton::build(&grammar);
            
            println!("Total de estados generados: {}", automata.states.len());
            println!("\n--- DETALLE DE ESTADOS ---");
            
            for state in &automata.states {
                // TÍTULO DEL ESTADO (Goto)
                if let Some((from, sym)) = &state.origin {
                    let sym_str = match sym { Symbol::Terminal(t) => t, Symbol::NonTerminal(nt) => nt };
                    println!("Estado I{}: Goto(I{}, {}) = I{}", state.id, from, sym_str, state.id);
                } else {
                    println!("Estado I0: (Estado Inicial)");
                }

                // ORDENAR ITEMS (Kernel primero)
                let mut sorted_items: Vec<_> = state.items.iter().collect();
                sorted_items.sort_by(|a, b| {
                    let a_is_kernel = a.dot_pos > 0 || a.head == automata.start_head;
                    let b_is_kernel = b.dot_pos > 0 || b.head == automata.start_head;
                    if a_is_kernel == b_is_kernel { a.cmp(b) } 
                    else if a_is_kernel { std::cmp::Ordering::Less } 
                    else { std::cmp::Ordering::Greater }
                });
                
                for item in sorted_items {
                    let mut body_str = String::new();
                    for (i, sym) in item.body.iter().enumerate() {
                        if i == item.dot_pos { body_str.push_str(" . "); }
                        match sym {
                            Symbol::Terminal(t) => body_str.push_str(&format!("{} ", t)),
                            Symbol::NonTerminal(nt) => body_str.push_str(&format!("{} ", nt)),
                        }
                    }
                    if item.dot_pos == item.body.len() { body_str.push_str(" ."); }
                    
                    println!("    {} -> {}", item.head, body_str.trim());
                }
                println!();
            }

            println!("--- TABLA DE TRANSICIONES 2D (GOTO / SHIFT) ---");
            
            // 1. Obtener todas las columnas (símbolos que tienen transiciones)
            let mut columns: Vec<Symbol> = automata.transitions.keys().map(|(_, sym)| sym.clone()).collect();
            columns.sort();
            columns.dedup();

            // 2. Imprimir Cabecera
            print!("| {:<6} |", "Estado");
            for col in &columns {
                let col_name = match col { Symbol::Terminal(t) => t, Symbol::NonTerminal(nt) => nt };
                print!(" {:<6} |", col_name);
            }
            println!();
            
            // Separador
            print!("|--------|");
            for _ in &columns { print!("--------|"); }
            println!();

            // 3. Imprimir Filas
            for state in &automata.states {
                print!("| {:<6} |", format!("I{}", state.id));
                for col in &columns {
                    if let Some(dest) = automata.transitions.get(&(state.id, col.clone())) {
                        print!(" {:<6} |", format!("I{}", dest));
                    } else {
                        print!(" {:<6} |", ""); // Celda vacía
                    }
                }
                println!();
            }
        }
        Err(e) => eprintln!(" Error crítico: {}", e),
    }
}