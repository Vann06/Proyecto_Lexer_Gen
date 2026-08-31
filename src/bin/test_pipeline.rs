// Pipeline end-to-end por consola: archivo fuente → lexer → parser →
// árbol → análisis semántico. Muestra lo que ENTREGA cada fase a la
// siguiente, que es lo que no se ve ni en los tests (comprueban resultados)
// ni en el IDE (devuelve JSON).
//
// Uso:
//   cargo run --bin test_pipeline -- <gramatica.yalp> <lexer.yal> <fuente> [--ll1|--lalr|--slr]
//
// Por defecto usa LALR(1). Con --ll1 usa LL(1), con --slr usa SLR(1).
//
// Flujo:
//   1. Construye la tabla del lexer compilando el .yal (mismas fases que main.rs).
//   2. Carga la gramática del .yalp y construye la tabla del parser.
//   3. Lee el archivo fuente y tokeniza con la Simulator.
//   4. Filtra tokens ignorables (Whitespace, Comment, Ignored).
//   5. Mapea Vec<Token> a Vec<ParseToken> usando el kind extraído del lexer.
//   6. Llama parse_tree y muestra el árbol en ASCII + escribe DOT a output/.
//   7. Corre el análisis semántico e imprime los ámbitos tal como se fueron
//      cerrando (única forma de ver lo declarado dentro de un bloque), la
//      tabla global, los diagnósticos y las closures.

use lexer_generator::{lexico, semantico, sintactico};

use std::env;
use std::fs;

use sintactico::gramatica::grammar::Grammar;
use sintactico::gramatica::first::calculate_first;
use sintactico::gramatica::follow::calculate_follow;
use sintactico::automatas::lr0::LR0Automaton;
use sintactico::automatas::lr1::LR1Automaton;
use sintactico::automatas::lalr::merge_by_core;
use sintactico::tablas::LRTable;
use sintactico::runtime::parser_lr::LRParser;
use sintactico::runtime::ll1::LL1Parser;
use sintactico::runtime::parse_tree::{ParseToken, print_ascii, to_dot};

use crate::lexico::pipeline;
use crate::semantico::analyzer::analyze;
use crate::semantico::spec::SemanticSpec;
use crate::lexico::runtime::simulator::{Simulator, LexResult, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode { LALR, SLR, LL1 }

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 {
        eprintln!("Uso: {} <gramatica.yalp> <lexer.yal> <fuente> [--ll1|--lalr|--slr]", args[0]);
        std::process::exit(1);
    }
    let yalp_path = &args[1];
    let yal_path  = &args[2];
    let src_path  = &args[3];
    let mode = args.iter().skip(4).find_map(|a| match a.as_str() {
        "--ll1"  => Some(Mode::LL1),
        "--lalr" => Some(Mode::LALR),
        "--slr"  => Some(Mode::SLR),
        _ => None,
    }).unwrap_or(Mode::LALR);

    println!("=== PIPELINE LEXICO → SINTACTICO → SEMANTICO ===");
    println!("  gramática : {}", yalp_path);
    println!("  lexer     : {}", yal_path);
    println!("  fuente    : {}", src_path);
    println!("  modo      : {:?}\n", mode);

    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║  FASE LEXICA — de texto a tokens                         ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    // ── 1. Construir tabla del lexer ────────────────────────────────────────
    let lexer_table = build_lexer_table(yal_path);
    println!("✓ Lexer construido ({} estados).\n", lexer_table.n_states);

    // ── 2. Leer fuente y tokenizar ──────────────────────────────────────────
    let source = fs::read_to_string(src_path).unwrap_or_else(|e| {
        eprintln!("Error al leer fuente: {}", e);
        std::process::exit(1);
    });
    let mut sim = Simulator::new(&lexer_table, &source);
    let mut raw_tokens: Vec<Token> = Vec::new();
    let mut lex_errors: Vec<String> = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => raw_tokens.push(t),
            LexResult::Error { lexeme, line, col } => {
                lex_errors.push(format!("línea {}:{} — '{}'", line, col, lexeme));
            }
            LexResult::EOF => break,
        }
    }
    if !lex_errors.is_empty() {
        eprintln!("⚠ Errores léxicos:");
        for e in &lex_errors { eprintln!("  {}", e); }
    }
    println!("✓ Lexer produjo {} tokens raw.", raw_tokens.len());

    // ── 3. Cargar gramática y filtrar tokens ────────────────────────────────
    let grammar = match mode {
        Mode::LALR | Mode::SLR => Grammar::parse_for_lr(yalp_path),
        Mode::LL1              => Grammar::parse_from_file(yalp_path),
    }.unwrap_or_else(|e| {
        eprintln!("Error al cargar gramática: {}", e);
        std::process::exit(1);
    });

    let mut significant: Vec<(String, String, usize, usize)> = raw_tokens.iter()
        .map(|t| (t.kind.to_uppercase(), t.lexeme.clone(), t.line, t.col))
        .filter(|(k, ..)| !grammar.ignores_kind(k))
        .collect();

    // Gramáticas sensibles a indentación (Python-style) necesitan un
    // post-procesamiento que el DFA del lexer no puede hacer por sí solo —
    // ver src/runtime/indent.rs. Debe correr ANTES de descartar line/col
    // (que es lo que hace la conversión a ParseToken de abajo).
    if lexico::runtime::indent::is_indent_sensitive(&grammar.tokens) {
        match lexico::runtime::indent::synthesize(significant) {
            Ok(synthesized) => significant = synthesized,
            Err(e) => {
                eprintln!("Error de indentación: {}", e);
                std::process::exit(2);
            }
        }
    }

    let parse_tokens: Vec<ParseToken> = significant.into_iter()
        .map(|(kind, lexeme, line, col)| ParseToken { kind, lexeme, line, col })
        .collect();

    println!(
        "✓ Tras filtrar ignorables: {} tokens al parser ({} descartados).",
        parse_tokens.len(),
        raw_tokens.len().saturating_sub(parse_tokens.len())
    );
    // Lo que el parser recibe de verdad, no solo los nombres: es el contrato
    // entre la fase lexica y la sintactica.
    println!("\n  {:<14} {:<18} {}", "KIND", "LEXEMA", "LINEA:COL");
    println!("  {}", "-".repeat(48));
    for t in &parse_tokens {
        let lexeme: String = t.lexeme.chars().take(16).collect();
        println!("  {:<14} {:<18} {}:{}", t.kind, lexeme, t.line, t.col);
    }
    println!();

    println!("== FASE SINTACTICA - de tokens a arbol ==");

    // ── 4. Construir parser y parsear ───────────────────────────────────────
    let tree = match mode {
        Mode::LALR => {
            let first_sets = calculate_first(&grammar);
            let lr1   = LR1Automaton::build(&grammar, &first_sets);
            let lalr  = merge_by_core(lr1);
            let table = LRTable::build_from_lalr(&lalr, &grammar);
            if !table.conflicts.is_empty() {
                println!("⚠ {} conflicto(s) en la tabla:", table.conflicts.len());
                for c in &table.conflicts { println!("   {}", c.describe()); }
            }
            let parser = LRParser::new(&table);
            parser.parse_tree(parse_tokens)
        }
        Mode::SLR => {
            let first_sets  = calculate_first(&grammar);
            let follow_sets = calculate_follow(&grammar, &first_sets);
            let lr0   = LR0Automaton::build(&grammar);
            let table = LRTable::build_from_slr(&lr0, &grammar, &follow_sets);
            if !table.conflicts.is_empty() {
                println!("⚠ {} conflicto(s) en la tabla:", table.conflicts.len());
                for c in &table.conflicts { println!("   {}", c.describe()); }
            }
            let parser = LRParser::new(&table);
            parser.parse_tree(parse_tokens)
        }
        Mode::LL1 => {
            let first_sets  = calculate_first(&grammar);
            let follow_sets = calculate_follow(&grammar, &first_sets);
            let parser = LL1Parser::build(&grammar, &first_sets, &follow_sets)
                .unwrap_or_else(|e| {
                    eprintln!("LL(1) no construible: {}", e);
                    std::process::exit(1);
                });
            parser.parse_tree(parse_tokens)
        }
    };

    match tree {
        Ok(t) => {
            println!("\n--- ÁRBOL DE DERIVACIÓN ---");
            print_ascii(&t);

            let dot = to_dot(&t);
            fs::create_dir_all("output").ok();
            let dot_path = format!("output/parse_tree_{}.dot", match mode {
                Mode::LALR => "lalr",
                Mode::SLR  => "slr",
                Mode::LL1  => "ll1",
            });
            if let Err(e) = fs::write(&dot_path, &dot) {
                eprintln!("✗ No se pudo escribir DOT: {}", e);
            } else {
                println!("\n✓ DOT escrito en {} (genera PNG con: dot -Tpng {} -o tree.png)",
                         dot_path, dot_path);
            }

            run_semantic_phase(&t, &grammar, mode, src_path);
        }
        Err(e) => {
            eprintln!("\n✗ Error de parseo: {}", e);
            std::process::exit(2);
        }
    }
}

/// Compila un .yal a una tabla de transición. La construcción real vive en
/// `lexico::pipeline::build_table`; acá solo queda la política de este
/// binario: ante un error, informar y salir con código distinto de cero.
fn build_lexer_table(yal_path: &str) -> crate::lexico::table::transition_table::TransitionTable {
    let yal_src = fs::read_to_string(yal_path).unwrap_or_else(|e| {
        eprintln!("Error al leer .yal: {}", e);
        std::process::exit(1);
    });
    pipeline::build_table(&yal_src).unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    })
}

/// Tercera fase: recorre el árbol que acaba de construir el parser y muestra
/// todo lo que produce el análisis semántico — los ámbitos tal como se fueron
/// cerrando, la tabla global que queda al final, los diagnósticos y las
/// closures.
fn run_semantic_phase(
    tree: &crate::sintactico::runtime::parse_tree::ParseNode,
    grammar: &Grammar,
    mode: Mode,
    src_path: &str,
) {
    println!("\n== FASE SEMANTICA - del arbol a la tabla de simbolos ==");

    // Dos motivos legítimos para no correrla. Se dicen en voz alta en vez de
    // no imprimir nada, que parecería un error.
    if mode == Mode::LL1 {
        println!("  omitida en modo LL(1): factorizar la gramatica renombra las");
        println!("  producciones, asi que las directivas semanticas -escritas");
        println!("  contra los nombres originales- dejarian de encontrarlas.");
        return;
    }
    let spec = match SemanticSpec::from_grammar(grammar) {
        Some(spec) => spec,
        None => {
            println!("  omitida: la gramatica no declara `%ident`, asi que no hay");
            println!("  forma de saber que token es un identificador.");
            return;
        }
    };

    let result = analyze(tree, &spec);

    // Los ámbitos, en el orden en que se cerraron (el más interno primero).
    // Es la única forma de ver lo declarado dentro de un bloque: la tabla
    // final solo conserva el Global.
    println!("\n--- AMBITOS (en orden de cierre: del mas interno al mas externo) ---");
    if result.scopes.is_empty() {
        println!("  (el programa no abrio ningun ambito propio)");
    } else {
        print!("{}", result.scopes.dump());
    }

    println!("\n--- TABLA DE SIMBOLOS GLOBAL (al terminar el recorrido) ---");
    print!("{}", result.table.dump());
    println!("  nota: los ambitos de arriba ya estan cerrados; aca solo queda el");
    println!("  Global, con lo de funciones y clases anidado en sus miembros.");

    let problems = result.errors.to_problems(src_path);
    println!("\n--- DIAGNOSTICOS SEMANTICOS ---");
    if problems.is_empty() {
        println!("  ninguno");
    } else {
        for p in &problems {
            println!(
                "  [{}] {} -- {}",
                p["code"].as_str().unwrap_or("?"),
                p["msg"].as_str().unwrap_or(""),
                p["loc"].as_str().unwrap_or("")
            );
        }
    }

    println!("\n--- CLOSURES (captura del entorno de definicion) ---");
    if result.closures.is_empty() {
        println!("  (ninguna funcion anidada captura variables de su entorno)");
    } else {
        print!("{}", result.closures.dump());
    }
    println!();
}
