// Prueba end-to-end de la gramática Compiscript (subconjunto, Fase 15):
// compila sin conflictos LALR, parsea `ejemplo.cps` de verdad, construye el
// árbol real y corre el análisis semántico declarado por %ident/%declare/
// %scope — verificando que produce exactamente el diagnóstico esperado con
// línea/columna reales (no 0:0) y el nombre de archivo real (no "input.txt").
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

#[test]
fn compiscript_grammar_compiles_without_lalr_conflicts() {
    let yalp = read("workspace/compiscript.yalp");
    let compile = api::build_compile_response(&yalp, "lalr").expect("compiscript.yalp debe compilar");
    let warnings: Vec<&str> = compile.problems.iter().filter(|p| p.level == "warn").map(|p| p.msg.as_str()).collect();
    assert!(warnings.is_empty(), "no debe haber conflictos LALR: {warnings:?}");
}

#[test]
fn ejemplo_cps_parses_and_reports_exactly_one_undeclared_identifier() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/ejemplo.cps");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "ejemplo.cps")
        .expect("el pipeline no debe fallar internamente");

    assert!(resp.accepted, "ejemplo.cps debe ser sintácticamente válido: {:?} / problems: {:#?}", resp.error, resp.problems);
    assert!(!resp.parse_tree_dot.is_empty(), "debe construir el árbol real (ParseNode -> DOT)");
    assert!(!resp.symbol_table.is_empty(), "compiscript.yalp trae %ident: debe haber tabla de símbolos");

    let sem_errors: Vec<&serde_json::Value> =
        resp.problems.iter().filter(|p| p["code"] == "S002").collect();
    assert_eq!(sem_errors.len(), 1, "problemas: {:#?}", resp.problems);

    let err = sem_errors[0];
    assert!(err["msg"].as_str().unwrap().contains("faltante"));
    assert_eq!(err["level"], "err");
    // loc usa el nombre real del archivo, no "input.txt" hardcodeado.
    assert!(err["loc"].as_str().unwrap().starts_with("ejemplo.cps:"));
    assert!(err["line"].as_u64().unwrap() > 0, "línea real, no 0");
    assert!(err["col"].as_u64().unwrap() > 0, "columna real, no 0");
}

#[test]
fn a_grammar_without_ident_directive_still_works_with_no_semantic_analysis() {
    // miniprog.yalp no trae %ident — el pipeline debe seguir funcionando
    // exactamente igual que antes de esta fase, sin tabla de símbolos ni
    // diagnósticos semánticos.
    let yal = read("workspace/miniprog.yal");
    let yalp = read("workspace/miniprog.yalp");
    let source = "fun main() : int { return 0; }";

    let resp = api::build_pipeline_response_named(&yal, &yalp, source, "lalr", "input.txt")
        .expect("el pipeline no debe fallar internamente");

    assert!(resp.accepted, "{:?}", resp.error);
    assert!(!resp.parse_tree_dot.is_empty(), "el árbol se construye igual, con o sin %ident");
    assert!(resp.symbol_table.is_empty(), "sin %ident no debe haber tabla de símbolos");
}
