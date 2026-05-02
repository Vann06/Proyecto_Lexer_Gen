#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::Grammar;

fn main() {
    println!("--- INICIANDO PRUEBA DEL ANALIZADOR SINTÁCTICO ---");
    
    let filepath = "examples/basic/ejemplo_c.yalp";
    
    match Grammar::parse_from_file(filepath) {
        Ok(grammar) => {
            println!(" Gramática de C cargada exitosamente.");
            println!("Símbolo inicial: {}", grammar.start_symbol);
            println!("Total de producciones: {}", grammar.productions.len());
            
            // Aquí en el futuro llamaremos a:
            // let first_sets = first_follow::calcular_first(&grammar);
        }
        Err(e) => eprintln!(" Error: {}", e),
    }
}