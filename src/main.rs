
// Punto de entrada del generador de lexers.
//
// Este programa implementa el pipeline completo para generar un analizador léxico
// a partir de una especificación en formato YALex (.yal).
//
// El proceso se divide en las siguientes fases principales:
//
//   FASE 1: Parsing de la Especificación
//     - Lee el archivo .yal.
//     - Parsea la cabecera, las definiciones (macros), las reglas y el tráiler.
//     - Módulos: `spec::parser`
//
//   FASE 2: Expansión de Macros
//     - Sustituye las macros definidas en las expresiones regulares de las reglas.
//     - Módulo: `spec::expand`
//
//   FASE 3: Construcción de AST para Regex
//     - Convierte cada expresión regular (ya expandida) en un Árbol de Sintaxis Abstracta (AST).
//     - Módulo: `regex::parser`
//
//   FASE 7: Construcción de NFA por Regla
//     - Transforma cada AST de regex en un Automata Finito No Determinista (NFA).
//     - A cada NFA se le asigna la acción y prioridad de su regla.
//     - Módulo: `automata::nfa`
//
//   FASE 7 (Combinación): Creación del "Super-NFA"
//     - Une todos los NFAs individuales en un único NFA gigante.
//     - Módulo: `automata::nfa`
//
//   FASE 8 y 9: Construcción del DFA (Subset Construction)
//     - Convierte el "Super-NFA" en un Automata Finito Determinista (DFA)
//       utilizando el algoritmo de construcción de subconjuntos.
//     - Módulo: `automata::subset`
//
//   FASE 10: Minimización del DFA
//     - Optimiza el DFA para reducir al mínimo el número de estados.
//     - Módulo: `automata::minimize`
//
//   FASE 11: Generación de Tabla de Transición
//     - Exporta la tabla transicional del AFD en memoria.
//     - Módulo: `table::transition_table`
//
//   FASE 12 y 13: Generación de Código y Simulación
//     - Emite código fuente Rust (`lexer.rs`).
//     - Opcionalmente simula la ejecución en memoria.
//     - Módulo: `codegen::rust_codegen` / `runtime::simulator`
//
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
use std::path::Path;
use std::process::Command;

use crate::spec::expand::expand_definitions;
use crate::spec::parser::parse_yalex;
use crate::regex::parser::parse_regex;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Uso: cargo run -- <archivo.yal> [archivo_entrada.txt]");
        std::process::exit(1);
    }

    let path = &args[1];
    
    // Si se provee un segundo argumento, lo usamos como archivo de entrada de prueba
    let test_input_path = args.get(2).cloned();

    // ── Fase 1: Leer y parsear el archivo .yal ──────────────────────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 1: PARSEANDO ESPECIFICACIÓN    ║");
    println!("╚══════════════════════════════════════════╝");
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

    println!("✓ Especificación '{}' parseada con éxito.", path);
    if let Some(h) = &spec.header {
        println!("  - Cabecera detectada ({} bytes)", h.len());
    }
    println!("  - {} definiciones encontradas.", spec.definitions.len());
    println!("  - {} reglas encontradas.", spec.rules.len());
    if let Some(t) = &spec.trailer {
        println!("  - Tráiler detectado ({} bytes)", t.len());
    }

    // ── Fase 2: Expansión de macros ─────────────────────────────────────────
    let expanded = expand_definitions(&spec);

    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 2: EXPANSIÓN DE MACROS         ║");
    println!("╚══════════════════════════════════════════╝");

    println!("✓ {} reglas expandidas.", expanded.len());
    for rule in &expanded {
        println!("  - [{}] {}  =>  {{...}}",
            rule.priority, rule.pattern_expanded);
    }

    // ── Fase 3 y 7: Construir AST de cada regex y su Autómata ─────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║ FASE 3 Y 7: ASTs y CONSTRUCCIÓN DE NFAs  ║");
    println!("╚══════════════════════════════════════════╝");
    
    let mut grandote_ast: Option<crate::regex::ast::RegexAst> = None;
    let mut all_ok = true;
    let mut id_counter = 0; 
    let mut nfas_list = Vec::new();

    for rule in expanded.iter() {
        match parse_regex(&rule.pattern_expanded) {
            Ok(ast) => {
                grandote_ast = match grandote_ast {
                    None => Some(ast.clone()),
                    Some(prev) => Some(crate::regex::ast::RegexAst::Union(Box::new(prev), Box::new(ast.clone())))
                };

                let mut rule_nfa = crate::automata::nfa::build_nfa_from_ast(&ast, &mut id_counter);
                
                if let Some(final_state) = rule_nfa.states.get_mut(&rule_nfa.end_state) {
                    final_state.accept_action = Some((rule.priority, rule.action_code.clone()));
                }
                
                nfas_list.push(rule_nfa);
            },
            Err(e) => {
                eprintln!("  ✗ Error al parsear regex '{}': {}", rule.pattern_expanded, e);
                all_ok = false;
            }
        }
    }

    if !all_ok {
        eprintln!("\nError: Se encontraron problemas en las expresiones regulares. Abortando.");
        std::process::exit(1);
    }

    println!("✓ Todos los ASTs y NFAs por regla fueron construidos.");

    // ── Generación de Gráficos (Opcional pero útil) ────────────────────────
    fs::create_dir_all("graphs").ok();
    if let Some(big_ast) = &grandote_ast {
        let dot_path = "graphs/ast_grandote.dot";
        if crate::graph::dot::write_ast_dot(dot_path, big_ast).is_ok() {
            println!("  - AST consolidado exportado a '{}'.", dot_path);
            println!("    (Puedes generar PNG usando: dot -Tpng {} -o graphs/ast_grandote.png)", dot_path);
        }
    }
        
    // ── FASE 7 (Final): Juntamos los NFAs en uno solo ───────────────────────
    let master_nfa = crate::automata::nfa::combine_nfas(nfas_list, &mut id_counter);
    println!("✓ Super-NFA maestro construido con {} estados.", master_nfa.states.len());
    
    // ── FASE 8 Y 9: Construcción del DFA ────────────────────────────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 8 Y 9: CONSTRUCCIÓN DE DFA     ║");
    println!("╚══════════════════════════════════════════╝");
    
    let dfa = crate::automata::subset::build_dfa_from_nfa(&master_nfa);
    println!("✓ DFA construido con {} estados mediante construcción de subconjuntos.", dfa.states.len());
    
    // ── FASE 10: Minimizamos el AFD ─────────────────────────────────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 10: MINIMIZACIÓN DE DFA        ║");
    println!("╚══════════════════════════════════════════╝");
    
    let min_dfa = crate::automata::minimize::minimize_dfa(&dfa);
    println!("✓ DFA minimizado a {} estados.", min_dfa.states.len());

    // ── Generar gráfico del DFA final ──────────────────────────────────────
    let dfa_dot_path = "graphs/dfa.dot";
    if crate::graph::dot::write_dfa_dot(dfa_dot_path, &min_dfa).is_ok() {
        println!("  - DFA final exportado a '{}'.", dfa_dot_path);
        println!("    (Puedes generar PNG usando: dot -Tpng {} -o graphs/dfa.png)", dfa_dot_path);
    }

    // ── Fase 11: Tabla de transición ────────────────────────────────────
    let table = crate::table::transition_table::build(&min_dfa);
    println!("\n✓ Fase 11: Tabla de transición construida ({} estados).", table.n_states);

    // ── Fase 13: Generar lexer.rs ───────────────────────────────────────
    println!("\n╔══════════════════════════════════════════╗");
    println!("║      FASE 13: GENERACIÓN DE CÓDIGO       ║");
    println!("╚══════════════════════════════════════════╝");
    
    fs::create_dir_all("generated/src").expect("No se pudo crear el directorio 'generated/src'");
    let output_path = "generated/src/lexer.rs";
    if let Err(e) = crate::codegen::rust_codegen::emit_file(
        output_path,
        &table,
        &expanded,
        spec.header.as_deref(),
        spec.trailer.as_deref(),
    ) {
        eprintln!("Error al generar lexer: {}", e);
        std::process::exit(1);
    }
    println!("✓ Código del lexer guardado en '{}'", output_path);

    // ── Fase 12 (opcional): Probar simulador en memoria ─────────────────
    if let Some(input_file) = test_input_path {
        println!("\n╔══════════════════════════════════════════╗");
        println!("║      FASE 12: SIMULANDO LEXER EN MEMORIA ║");
        println!("╚══════════════════════════════════════════╝");
        
        let test_input_content = fs::read_to_string(&input_file).unwrap_or_else(|e| {
            eprintln!("⚠ No se pudo leer el archivo de entrada de prueba '{}': {}", input_file, e);
            String::new()
        });
        
        let test_input = test_input_content.as_str();

        let mut sim = crate::runtime::simulator::Simulator::new(&table, test_input);
        let (tokens, errors) = sim.tokenize();
        println!("✓ Analizador Léxico (Simulador) probado con entrada de {} bytes.", test_input.len());
        
        println!("\n--- ERRORES LÉXICOS ({} detectado(s)) ---", errors.len());
        for e in &errors {
            println!("  ❌ {}", e);
        }

        println!("\n--- TOKENS IDENTIFICADOS ({} token(s)) ---", tokens.len());
        for t in &tokens {
            println!("  Token: kind={:?} lexeme={:?} line={} col={}", t.kind, t.lexeme, t.line, t.col);
        }

        // --- COMPILACIÓN DEL LEXER GENERADO ---
        println!("\n╔══════════════════════════════════════════╗");
        println!("║      FASE 14: COMPILANDO LEXER GENERADO  ║");
        println!("╚══════════════════════════════════════════╝");
        
        let crate_dir = Path::new("generated");
        let src_dir = crate_dir.join("src");
        let main_rs_path = src_dir.join("main.rs");
        let cargo_toml_path = crate_dir.join("Cargo.toml");

        let cargo_toml_content = r#"[package]
name = "generated_lexer"
version = "0.1.0"
edition = "2021"
"#;
        fs::write(&cargo_toml_path, cargo_toml_content).expect("No se pudo crear Cargo.toml para el lexer");

        // Crear un main.rs que use el lexer para procesar el archivo de entrada.
        // Hacemos que utilice escape en las rutas.
        let main_rs_content = format!(r#"
mod lexer;
use std::fs;

fn main() {{
    let input_path = "{}";
    let content = fs::read_to_string(format!("../{{}}", input_path)).unwrap_or_else(|_| "".to_string());
    
    println!("--- Ejecutando lexer.rs sobre: {{}} ---", input_path);
    let chars: Vec<char> = content.chars().collect();
    let mut pos = 0;
    let mut line = 1;
    let mut col = 1;
    
    let mut tokens_count = 0;
    let mut errors_count = 0;

    while let Some(res) = lexer::next_token(&chars, &mut pos, &mut line, &mut col) {{
        match res {{
            Ok(tok) => {{
                if tok.kind != "Whitespace" && tok.kind != "Comment" && tok.kind != "Ignored" {{
                    tokens_count += 1;
                    // println!("Token: {{}} -> {{:?}}", tok.lexeme, tok.kind);
                }}
            }},
            Err(e) => {{
                errors_count += 1;
                println!("Error léxico en la línea {{}}: {{}}", line, e);
            }}
        }}
    }}
    println!("--- Fin del análisis ({{}} tokens, {{}} errores) ---", tokens_count, errors_count);
}}
"#, input_file.replace('\\', "/")); 

        let main_rs_clean = main_rs_content.replace("\\tokens_count", "tokens_count").replace("\\errors_count", "errors_count");

        fs::write(&main_rs_path, main_rs_clean).expect("No se pudo crear main.rs para el lexer");

        println!("  - Compilando proyecto en 'generated/'...");
        let build_status = Command::new("cargo")
            .arg("build")
            .current_dir(crate_dir)
            .output()
            .expect("Fallo al ejecutar 'cargo build'");

        if build_status.status.success() {
            println!("  ✓ Compilación exitosa. Ejecutando el lexer nativo...");
            let run_status = Command::new("cargo")
                .arg("run")
                .arg("-q")
                .current_dir(crate_dir)
                .output()
                .expect("Fallo al ejecutar 'cargo run'");
            
            println!("{}", String::from_utf8_lossy(&run_status.stdout));
        } else {
            eprintln!("  ⚠ Error al compilar lexer.rs. Revisa el código generado.");
        }
    }
    
    println!("\n============================================================");
    println!("Salidas Esperadas Generadas:");
    println!("  - Árbol de Expresión grandote unificado (Ver graphs/ast_grandote.png)");
    println!("  - Un Autómata graficado (Ver graphs/dfa.png)");
    println!("  - Un programa fuente del analizador léxico (Ver generated/lexer.rs)");
    println!("============================================================");

    println!("\n¡Proceso completado correctamente!");
}
