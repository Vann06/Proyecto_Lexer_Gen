
// Punto de entrada del generador de lexers.
// Coordina las fases del pipeline:
//   Fase 1 — Lectura y parsing del .yal  (spec/parser)
//   Fase 2 — Expansión de macros         (spec/expand)
//   Fase 3 — Construcción del AST regex  (regex/parser)
//   Fases siguientes: NFA, DFA, minimización, codegen (pendientes)
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

use crate::spec::expand::expand_definitions;
use crate::spec::parser::parse_yalex;
use crate::regex::parser::parse_regex;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: cargo run -- <archivo.yal>");
        std::process::exit(1);
    }

    let path = &args[1];

    // ── Fase 1: Leer y parsear el archivo .yal ──────────────────────────────
    let input = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) => {
            eprintln!("Error: no se pudo leer el archivo '{}': {}", path, err);
            std::process::exit(1);
        }
    };

    let spec = match parse_yalex(&input) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Error al parsear '{}': {}", path, err);
            std::process::exit(1);
        }
    };

    // ── Imprimir SPEC leída ─────────────────────────────────────────────────
    println!("╔══════════════════════════════════════════╗");
    println!("║         FASE 1 — SPEC LEÍDA              ║");
    println!("╚══════════════════════════════════════════╝");

    if let Some(h) = &spec.header {
        println!("\n[Header]\n{}", h.trim());
    }

    println!("\n[Definiciones ({})]", spec.definitions.len());
    for def in &spec.definitions {
        println!("  let {} = {}", def.name, def.regex);
    }

    println!("\n[Reglas ({})]", spec.rules.len());
    for rule in &spec.rules {
        println!("  [{}] pattern: {}  =>  action: {{ {} }}",
            rule.priority, rule.pattern_raw, rule.action_code);
    }

    if let Some(t) = &spec.trailer {
        println!("\n[Trailer]\n{}", t.trim());
    }

    // ── Fase 2: Expansión de macros ─────────────────────────────────────────
    let expanded = expand_definitions(&spec);

    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 2 — REGLAS EXPANDIDAS          ║");
    println!("╚══════════════════════════════════════════╝");

    for rule in &expanded {
        println!("  [{}] {}  =>  {{ {} }}",
            rule.priority, rule.pattern_expanded, rule.action_code);
    }

    // ── Fase 3: Construir AST de cada regex ─────────────────────────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 3 — AST DE CADA REGLA          ║");
    println!("╚══════════════════════════════════════════╝");

    let mut all_ok = true;
    for rule in &expanded {
        println!("\n  Regla [{}] — acción: {{ {} }}", rule.priority, rule.action_code);
        println!("  Regex expandida: {}", rule.pattern_expanded);
        println!("  AST:");
        match parse_regex(&rule.pattern_expanded) {
            Ok(ast) => println!("{}", ast.pretty_print(2)),
            Err(e) => {
                eprintln!("  ✗ Error al parsear regex: {}", e);
                all_ok = false;
            }
        }
    }

    // ── Resumen ──────────────────────────────────────────────────────────────
    println!();
    if all_ok {
        println!("✓ Fase 1 completada: {} definición(es), {} regla(s) leídas.",
            spec.definitions.len(), spec.rules.len());
        println!("✓ Fase 2 completada: macros expandidas correctamente.");
        println!("✓ Fase 3 completada: AST construido para todas las reglas.");
    } else {
        eprintln!("✗ Hubo errores en la construcción del AST.");
        std::process::exit(1);
    }
}