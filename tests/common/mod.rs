//! Utilidades compartidas por las pruebas del generador de TAC: compilan un
//! fuente de Compiscript con el pipeline real (léxico, LALR, semántico) y
//! devuelven el programa generado.
use lexer_generator::api;
use lexer_generator::intermedio::gen::{generate, CodegenError};
use lexer_generator::intermedio::spec::IntermediateSpec;
use lexer_generator::intermedio::tac::TacProgram;
use lexer_generator::semantico::analyzer::{analyze, AnalysisResult};
use lexer_generator::semantico::spec::SemanticSpec;
use lexer_generator::sintactico::automatas::lalr::merge_by_core;
use lexer_generator::sintactico::automatas::lr1::LR1Automaton;
use lexer_generator::sintactico::gramatica::first::calculate_first;
use lexer_generator::sintactico::gramatica::grammar::Grammar;
use lexer_generator::sintactico::runtime::parse_tree::ParseToken;
use lexer_generator::sintactico::runtime::parser_lr::LRParser;
use lexer_generator::sintactico::tablas::LRTable;
use std::fs;

pub fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

/// Lexea, parsea, analiza y genera `source` con Compiscript. Devuelve el
/// programa (o los problemas de la generación) y el análisis.
pub fn intentar(source: &str) -> (Result<TacProgram, Vec<CodegenError>>, AnalysisResult) {
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
    assert!(analysis.errors.is_empty(), "el programa no debe tener diagnósticos: {:?}", analysis.errors);
    let ispec = IntermediateSpec::from_grammar(&grammar);
    (generate(&tree, &analysis, &sspec, &ispec), analysis)
}

/// Como `intentar`, pero la generación tiene que salir bien.
pub fn compilar(source: &str) -> (TacProgram, AnalysisResult) {
    let (program, analysis) = intentar(source);
    (program.unwrap_or_else(|e| panic!("debe generar: {e:?}")), analysis)
}

pub fn generar(source: &str) -> TacProgram {
    compilar(source).0
}

/// El programa completo (datos, `main` y funciones) sin los espacios al
/// final de cada línea.
pub fn programa(source: &str) -> String {
    generar(source).to_string().lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}
