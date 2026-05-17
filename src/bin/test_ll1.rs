
#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::{Grammar, Symbol};
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;
use analizador_sintactico::ll1::LL1Parser;
use analizador_sintactico::parse_tree::{ParseToken, print_ascii, to_dot};
use std::io::{self, Write};

fn main() {
    println!("=== TESTER PARSER LL(1) ===");
    print!("Introduce la ruta del archivo .yalp: ");
    io::stdout().flush().unwrap();
    
    let mut input_path = String::new();
    io::stdin().read_line(&mut input_path).unwrap();
    let filepath = input_path.trim();

    match Grammar::parse_from_file(filepath) {
        Ok(grammar) => {
            println!("\nGramática '{}' cargada exitosamente.", filepath);
             
            println!("\n--- PROCEDIMIENTO DE LIMPIEZA LL(1) ---");

            if grammar.transformation_log.is_empty() {
                println!("No se registraron transformaciones.");
            } else {
                for step in &grammar.transformation_log {
                    println!("- {}", step);
                }
            }

            println!("\n--- GRAMÁTICA DESPUÉS DE LIMPIEZA LL(1) ---");

            for prod in &grammar.productions {
                let bodies: Vec<String> = prod.bodies.iter().map(|body| {
                    if body.is_empty() {
                        return "ε".to_string();
                    }

                    body.iter().map(|sym| match sym {
                        Symbol::Terminal(t) => t.clone(),
                        Symbol::NonTerminal(nt) => nt.clone(),
                    }).collect::<Vec<_>>().join(" ")
                }).collect();

                println!("{} -> {}", prod.head, bodies.join(" | "));
            }


            let first_sets = calculate_first(&grammar);
            let follow_sets = calculate_follow(&grammar, &first_sets);
            
            println!("\n--- FIRST ---");

            let mut first_keys: Vec<_> = first_sets.keys().collect();
            first_keys.sort();

            for nt in first_keys {
                let mut values: Vec<_> = first_sets[nt].iter().cloned().collect();
                values.sort();

                println!("FIRST({}) = {{ {} }}", nt, values.join(", "));
            }

            println!("\n--- FOLLOW ---");

            let mut follow_keys: Vec<_> = follow_sets.keys().collect();
            follow_keys.sort();

            for nt in follow_keys {
                let mut values: Vec<_> = follow_sets[nt].iter().cloned().collect();
                values.sort();

                println!("FOLLOW({}) = {{ {} }}", nt, values.join(", "));
            }

            println!("\n--- CONSTRUYENDO TABLA LL(1) ---");
            match LL1Parser::build(&grammar, &first_sets, &follow_sets) {
                Ok(parser) => {
                    println!("Tabla LL(1) construida exitosamente sin conflictos.");
                    
                    println!("\n--- TABLA LL(1) ---");
                    // Obtener todos los terminales (columnas) para mostrar la tabla
                    let mut all_terminals = std::collections::HashSet::new();
                    for row in parser.table.values() {
                        for term in row.keys() {
                            all_terminals.insert(term.clone());
                        }
                    }
                    let mut cols: Vec<_> = all_terminals.into_iter().collect();
                    cols.sort();

                    print!("| {:<15} |", "Non-Terminal");
                    for col in &cols {
                        print!(" {:<15} |", col);
                    }
                    println!();
                    print!("|{:-<17}|", "");
                    for _ in &cols { 
                        print!("{:-<17}|", "");
                    }
                    println!();

                    let mut heads: Vec<_> = parser.table.keys().collect();
                    heads.sort();

                    for head in heads {
                        print!("| {:<15} |", head);
                        let row = &parser.table[head];
                        for col in &cols {
                            if let Some(prod) = row.get(col) {
                                let body = &prod.bodies[0];
                                let mut body_str = String::new();
                                if body.is_empty() {
                                    body_str = "ε".to_string();
                                } else {
                                    for sym in body {
                                        match sym {
                                            Symbol::Terminal(t) => body_str.push_str(&format!("{} ", t)),
                                            Symbol::NonTerminal(nt) => body_str.push_str(&format!("{} ", nt)),
                                        }
                                    }
                                }
                                print!(" {:<15} |", body_str.trim());
                            } else {
                                print!(" {:<15} |", "");
                            }
                        }
                        println!();
                    }

                    // Loop de prueba de parseo
                    loop {
                        println!("\n--- PRUEBA DE PARSEO ---");
                        println!("Introduce una secuencia de tokens separados por espacios (o 'exit' para salir):");
                        print!("> ");
                        io::stdout().flush().unwrap();

                        let mut input_tokens = String::new();
                        io::stdin().read_line(&mut input_tokens).unwrap();
                        let input_tokens = input_tokens.trim();

                        if input_tokens == "exit" {
                            break;
                        }

                        let kinds: Vec<String> = input_tokens.split_whitespace().map(|s| s.to_string()).collect();

                        println!("Parseando: {:?}", kinds);
                        match parser.parse(kinds.clone()) {
                            Ok(_) => {
                                println!("✓ La cadena es VÁLIDA para esta gramática.");

                                // Construir e imprimir árbol de derivación
                                let tokens = ParseToken::from_kinds(kinds);
                                match parser.parse_tree(tokens) {
                                    Ok(tree) => {
                                        println!("\n--- ÁRBOL DE DERIVACIÓN ---");
                                        print_ascii(&tree);

                                        let dot = to_dot(&tree);
                                        if std::fs::create_dir_all("output").is_ok() {
                                            if let Err(e) = std::fs::write("output/parse_tree_ll1.dot", &dot) {
                                                eprintln!("(no se pudo escribir DOT: {})", e);
                                            } else {
                                                println!("\n(DOT escrito en output/parse_tree_ll1.dot)");
                                            }
                                        }
                                    }
                                    Err(e) => eprintln!("✗ Error al construir árbol: {}", e),
                                }
                            },
                            Err(e) => eprintln!("✗ Error de parseo: {}", e),
                        }
                    }
                },
                Err(e) => eprintln!("Error al construir la tabla LL(1): {}", e),
            }
        },
        Err(e) => eprintln!("Error al cargar la gramática: {}", e),
    }
}
