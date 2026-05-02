#[path = "../analizador_sintactico/mod.rs"]
mod analizador_sintactico;

use analizador_sintactico::grammar::Grammar;
use analizador_sintactico::first::calculate_first;
use analizador_sintactico::follow::calculate_follow;

fn main() {
    let filepath = "examples/basic/ejemplo_c.yalp"; // O tu archivo de prueba
    
    match Grammar::parse_from_file(filepath) {
        Ok(grammar) => {
            println!("Gramática validada y cargada.");
            
            let first_sets = calculate_first(&grammar);
            println!("\n--- CONJUNTOS FIRST ---");
            for (nt, set) in &first_sets {
                println!("FIRST({}) = {:?}", nt, set);
            }

            let follow_sets = calculate_follow(&grammar, &first_sets);
            println!("\n--- CONJUNTOS FOLLOW ---");
            for (nt, set) in &follow_sets {
                println!("FOLLOW({}) = {:?}", nt, set);
            }
        }
        Err(e) => eprintln!("{}", e),
    }
}