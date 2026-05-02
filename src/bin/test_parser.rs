#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::Grammar;
use analizador_sintactico::first_follow;
use std::env;
use std::fs;

fn main() {
    println!("--- INICIANDO PRUEBA DEL ANALIZADOR SINTÁCTICO ---");
    
    let args: Vec<String> = env::args().collect();
    
    let (yal_path, yalp_path) = if args.len() >= 3 {
        (args[1].clone(), args[2].clone())
    } else {
        println!("⚠️ No se proporcionaron argumentos. Usando archivos por defecto (ejemplo_c).");
        ("examples/basic/ejemplo_c.yal".to_string(), "examples/basic/ejemplo_c.yalp".to_string())
    };

    println!("\nArchivos a procesar:");
    println!("  - Léxico (.yal): {}", yal_path);
    println!("  - Sintáctico (.yalp): {}", yalp_path);
    
    if let Ok(yal_content) = fs::read_to_string(&yal_path) {
        println!("✅ Archivo léxico ({}) leído exitosamente ({} bytes).", yal_path, yal_content.len());
    } else {
        println!("❌ Error: No se pudo leer el archivo léxico {}", yal_path);
    }

    match Grammar::parse_from_file(&yalp_path) {
        Ok(grammar) => {
            println!("✅ Gramática ({}) cargada exitosamente.", yalp_path);
            println!("Símbolo inicial: {}", grammar.start_symbol);
            println!("Total de tokens declarados: {}", grammar.tokens.len());
            println!("Total de producciones: {}", grammar.productions.len());
            
            println!("\n--- CALCULANDO CONJUNTOS FIRST Y FOLLOW ---");
            let first_sets = first_follow::calculate_first(&grammar);
            let follow_sets = first_follow::calculate_follow(&grammar, &first_sets);
            
            println!("\nConjuntos FIRST:");
            for (nt, set) in &first_sets {
                let mut elements: Vec<_> = set.iter().collect();
                elements.sort();
                println!("  FIRST({}) = {{ {} }}", nt, elements.into_iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
            
            println!("\nConjuntos FOLLOW:");
            for (nt, set) in &follow_sets {
                let mut elements: Vec<_> = set.iter().collect();
                elements.sort();
                println!("  FOLLOW({}) = {{ {} }}", nt, elements.into_iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
            }
        }
        Err(e) => eprintln!("❌ Error al parsear .yalp: {}", e),
    }
}