
// Leer el nombre del archivo .yal 
// Llamar cada fase del proceso 
// Coordinar el flujo 
mod error;
mod spec;
mod regex;
mod automata;
mod table;
mod runtime;
mod codegen;
mod graph;

use std::env;
use std::fs;

use crate::spec::parser::parse_yalex;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: cargo run -- <archivo.yal>");
        std::process::exit(1);
    }

    let path = &args[1];

    let input = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("No se pudo leer el archivo '{}': {}", path, err);
            std::process::exit(1);
        }
    };

    match parse_yalex(&input) {
        Ok(spec) => {
            println!("Archivo leído correctamente.");
            println!("{:#?}", spec);
        }
        Err(err) => {
            eprintln!("Error al parsear YALex: {}", err);
            std::process::exit(1);
        }
    }
}