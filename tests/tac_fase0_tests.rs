//! Fase 0 del generador de código intermedio (ver `PLAN_TAC.md`): los ajustes
//! al análisis semántico que el generador necesita y las compuertas de
//! `intermedio::gen::generate`. Todo por el pipeline real, sobre Compiscript.
use lexer_generator::api;
use lexer_generator::intermedio::gen::generate;
use lexer_generator::intermedio::spec::IntermediateSpec;
use lexer_generator::semantico::analyzer::analyze;
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseToken;
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

/// Los códigos de los problemas del pipeline (léxico, sintáctico, semántico).
fn codigos(source: &str) -> Vec<String> {
    let r = api::build_pipeline_response_named(
        &read("workspace/compiscript.yal"),
        &read("workspace/compiscript.yalp"),
        source,
        "lalr",
        "fase0.cps",
    )
    .expect("el pipeline no debe fallar internamente");
    assert!(r.accepted, "debe parsear: {:?}", r.error);
    r.problems.iter().map(|p| p["code"].as_str().unwrap_or("").to_string()).collect()
}

/// Lexea, parsea, analiza y genera; devuelve los códigos de error del
/// generador (vacío si generó).
fn generar(source: &str) -> Result<String, Vec<(String, usize)>> {
    let (_, _, lexer_table) = api::build_lexer_artifacts(&read("workspace/compiscript.yal")).expect("lexer válido");
    let grammar = Grammar::parse_for_lr_from_str(&read("workspace/compiscript.yalp")).expect("gramática válida");

    use lexer_generator::lexico::runtime::simulator::{LexResult, Simulator};
    let mut sim = Simulator::new(&lexer_table, source);
    let mut tokens = Vec::new();
    loop {
        match sim.next_token() {
            LexResult::Token(t) => {
                let kind = t.kind.to_uppercase();
                if !grammar.ignores_kind(&kind) {
                    tokens.push(ParseToken { kind, lexeme: t.lexeme, line: t.line, col: t.col });
                }
            }
            LexResult::Error { lexeme, line, col } => panic!("error léxico: '{lexeme}' en {line}:{col}"),
            LexResult::EOF => break,
        }
    }
    let first = calculate_first(&grammar);
    let table = LRTable::build_from_lalr(&merge_by_core(LR1Automaton::build(&grammar, &first)), &grammar);
    let tree = LRParser::new(&table).parse_tree(tokens).expect("fuente válida");

    let sspec = SemanticSpec::from_grammar(&grammar).expect("compiscript.yalp trae %ident");
    let analysis = analyze(&tree, &sspec);
    let ispec = IntermediateSpec::from_grammar(&grammar);
    generate(&tree, &analysis, &sspec, &ispec)
        .map(|p| p.to_string())
        .map_err(|errs| errs.into_iter().map(|e| (e.code, e.line)).collect())
}

// ─────────────────────────────── null ───────────────────────────────

#[test]
fn null_se_asigna_a_cualquier_referencia() {
    let src = "class P { var x: integer = 0; }\n\
               let s: string = null;\n\
               let p: P = null;\n\
               let l: integer[] = null;\n\
               let d = null;\n\
               d = new P();\n\
               if (p == null) { print(1); }\n";
    assert!(codigos(src).is_empty(), "{:?}", codigos(src));
}

#[test]
fn null_no_se_asigna_a_un_valor() {
    assert_eq!(codigos("let n: integer = null;\n"), vec!["S006"]);
    assert_eq!(codigos("let b: boolean = null;\n"), vec!["S006"]);
}

// ─────────────────────────────── módulo ───────────────────────────────

#[test]
fn el_modulo_es_entre_enteros() {
    assert!(codigos("let r: integer = 7 % 2;\n").is_empty());
    assert_eq!(codigos("let r = \"a\" % 2;\n"), vec!["S015"], "operación aritmética inválida");
}

// ─────────────────────────── compuertas del generador ───────────────────────────

#[test]
fn con_errores_semanticos_no_se_genera_codigo() {
    let errs = generar("let x: integer = \"texto\";\n").unwrap_err();
    assert_eq!(errs, vec![("C001".to_string(), 0)]);
}

#[test]
fn la_rubrica_pasa_las_compuertas() {
    // Sin errores y con todos los marcos completos (null ya tiene tipo):
    // el generador entra a traducir. Mientras falten las fases 1 y 2, lo
    // único que puede devolver es C900 ("todavía no soportada"), nunca C001
    // ni C002.
    match generar(&read("workspace/rubrica.cps")) {
        Ok(_) => {}
        Err(errs) => assert!(errs.iter().all(|(c, _)| c == "C900"), "{errs:?}"),
    }
}

#[test]
fn una_construccion_sin_traducir_se_reporta_con_su_linea() {
    let errs = generar("let x: integer = 1;\nclass C { }\n").unwrap_err();
    assert!(errs.contains(&("C900".to_string(), 2)), "la clase está en la línea 2: {errs:?}");
}
