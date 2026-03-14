
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

    // ── Fase 3 y 7: Construir AST de cada regex y su Autómata ─────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║   FASE 3 Y 7 — AST Y CONSTRUCCIÓN NFA    ║");
    println!("╚══════════════════════════════════════════╝");

    let mut all_ok = true;
    let mut id_counter = 0; // El contador maestro de bolitas que le pasaremos al autómata
    
    // Aquí vamos a ir guardando los mini-autómatas de cada regla
    let mut nfas_list = Vec::new();

    for rule in &expanded {
        println!("\n  Regla [{}] — acción: {{ {} }}", rule.priority, rule.action_code);
        println!("  Regex expandida: {}", rule.pattern_expanded);
        
        match parse_regex(&rule.pattern_expanded) {
            Ok(ast) => {
                println!("  [AST Construido Correctamente]");
                
                // Opcional: Generar gráfica DOT para la Fase 6
                // let dot_filename = format!("graphs/ast_regex_rule_{}.dot", rule.priority);
                // if let Err(e) = crate::graph::dot::generate_dot(&ast, &dot_filename) {
                //     eprintln!("  ⚠ No se pudo generar la gráfica para {}: {}", dot_filename, e);
                // }
                
                // 🔴 FASE 7: Creamos el Autómata chiquito y lo guardamos
                let mut rule_nfa = crate::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
                
                // Le damos su premio a esta regla (Para saber qué acción era, al final)
                if let Some(final_state) = rule_nfa.states.get_mut(&rule_nfa.end_state) {
                    final_state.accept_action = Some((rule.priority, rule.action_code.clone()));
                }
                
                println!("  [AFN Construido: {} estados generados]", rule_nfa.states.len());
                nfas_list.push(rule_nfa);
            },
            Err(e) => {
                eprintln!("  ✗ Error al parsear regex: {}", e);
                all_ok = false;
            }
        }
    }

    // ── Resumen y Pegamento ──────────────────────────────────────────────────
    println!();
    if all_ok {
        println!("✓ Fase 1 a 3 completadas.");
        
        // 🔴 FASE 7 (Final): Juntamos los 25 autómatas en uno solo gigante
        let master_nfa = crate::automata::nfa::combine_nfas(nfas_list, &mut id_counter);
        
        println!("✓ Fase 7 completada: ¡Super AFN maestro construido!");
        println!("  Total de Estados (Bolitas) en memoria: {}", master_nfa.states.len());
        println!("  Estado Inicial de Entrada: {}", master_nfa.start_state);
        
        // 🔴 FASE 8 Y 9: Construimos el Autómata Finito Determinista
        println!("\n╔══════════════════════════════════════════╗");
        println!("║      FASE 8 Y 9 — CONSTRUCCIÓN AFD       ║");
        println!("╚══════════════════════════════════════════╝");
        
        let dfa = crate::automata::subset::build_dfa_from_nfa(&master_nfa);
        println!("✓ Fase 8 y 9 completadas: ¡AFD compacto construido usando Subconjuntos!");
        println!("  De {} mágicos, sobrevivieron: {} Estados Deterministas Seguros.", master_nfa.states.len(), dfa.states.len());
        println!("  Estado Inicial del AFD: {}", dfa.start_state);
        
        // 🔴 FASE 10: Minimizamos el AFD
        println!("\n╔══════════════════════════════════════════╗");
        println!("║      FASE 10 — MINIMIZACIÓN DE AFD       ║");
        println!("╚══════════════════════════════════════════╝");
        
        let min_dfa = crate::automata::minimize::minimize_dfa(&dfa);
        println!("✓ Fase 10 completada: ¡AFD Minimizado con éxito!");
        println!("  Estados antes (Subset AFD): {}", dfa.states.len());
        println!("  Estados finales (Min AFD) : {}", min_dfa.states.len());
        println!("  Estado Inicial Minimizado : {}", min_dfa.start_state);

        // ── Fase 6: Exportar DFA a DOT ───────────────────────────────────────
        fs::create_dir_all("graphs").ok();
        if let Err(e) = crate::graph::dot::write_dfa_dot("graphs/dfa.dot", &min_dfa) {
            eprintln!("⚠ No se pudo escribir graphs/dfa.dot: {}", e);
        } else {
            println!("\n✓ Fase 6: graphs/dfa.dot generado.");
        }

        // ── Fase 11: Tabla de transición ────────────────────────────────────
        let table = crate::table::transition_table::build(&min_dfa);
        println!("✓ Fase 11: Tabla de transición construida ({} estados).", table.n_states);

        // ── Fase 13: Generar lexer.rs ───────────────────────────────────────
        if let Err(e) = crate::codegen::rust_codegen::emit_file(
            "generated/lexer.rs",
            &table,
            &expanded,
            spec.header.as_deref(),
            spec.trailer.as_deref(),
        ) {
            eprintln!("Error al generar lexer: {}", e);
            std::process::exit(1);
        }
        println!("✓ Fase 13: generated/lexer.rs generado.");

        // ── Fase 12 (opcional): Probar simulador en memoria ─────────────────
        let test_input = "42 abc";
        let mut sim = crate::runtime::simulator::Simulator::new(&table, test_input);
        let (tokens, errors) = sim.tokenize();
        println!("\n✓ Fase 12: Simulador probado con \"{}\": {} token(s), {} error(es).", test_input, tokens.len(), errors.len());
        for t in &tokens {
            println!("    Token: kind={:?} lexeme={:?} line={} col={}", t.kind, t.lexeme, t.line, t.col);
        }
        
    } else {
        eprintln!("✗ Hubo errores en las fases previas.");
        std::process::exit(1);
    }
}