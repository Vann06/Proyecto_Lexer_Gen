// Prueba EMPÍRICA de que el análisis semántico de clases y objetos
// (src/semantico/{analyzer,classes}) es agnóstico a la gramática.
//
// `examples/grammar/objetos_es.yalp` + `examples/lexer/objetos_es.yal`
// definen un lenguaje de objetos donde NADA se llama como en Compiscript:
// PROPIO/PUNTO/CREAR/OBJETO/METODO/IDENT en vez de THIS/DOT/NEW/CLASS/
// FUNCTION/ID, producciones con nombres distintos, el atributo declara el
// nombre ANTES del tipo (orden inverso), y el constructor es un método
// llamado "iniciar" en vez de "constructor".
//
// El mismo `analyze()` — sin una sola línea de Rust distinta — debe
// resolver miembros con herencia, `this`, y la invocación del constructor
// exactamente igual, y emitir los MISMOS códigos de diagnóstico.
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

fn yal() -> String {
    read("examples/lexer/objetos_es.yal")
}
fn yalp() -> String {
    read("examples/grammar/objetos_es.yalp")
}

#[test]
fn alternate_grammar_compiles_without_lalr_conflicts() {
    let compile = api::build_compile_response(&yalp(), "lalr").expect("objetos_es.yalp debe compilar");
    let warnings: Vec<&str> = compile
        .problems
        .iter()
        .filter(|p| p.level == "warn")
        .map(|p| p.msg.as_str())
        .collect();
    assert!(warnings.is_empty(), "no debe haber conflictos LALR: {warnings:?}");
}

#[test]
fn alternate_grammar_valid_program_has_zero_diagnostics() {
    let source = read("examples/source/objetos_es.txt");
    let resp = api::build_pipeline_response_named(&yal(), &yalp(), &source, "lalr", "objetos_es.txt")
        .expect("el pipeline no debe fallar internamente");

    assert!(
        resp.accepted,
        "objetos_es.txt debe ser sintácticamente válido: {:?} / problems: {:#?}",
        resp.error, resp.problems
    );
    assert!(
        resp.problems.is_empty(),
        "una gramática con otros nombres debe analizarse igual de limpio: {:#?}",
        resp.problems
    );

    // La tabla de símbolos se construyó (el .yalp trae %ident).
    assert!(!resp.symbol_table.is_empty());
    assert!(resp.symbol_table.contains("Figura"));
    assert!(resp.symbol_table.contains("Circulo"));
}

#[test]
fn alternate_grammar_reports_the_same_diagnostic_codes() {
    let source = read("examples/source/objetos_es_errores.txt");
    let resp = api::build_pipeline_response_named(&yal(), &yalp(), &source, "lalr", "objetos_es_errores.txt")
        .expect("el pipeline no debe fallar internamente");

    assert!(
        resp.accepted,
        "el archivo de errores debe ser sintácticamente válido (los errores son semánticos): {:?} / {:#?}",
        resp.error, resp.problems
    );

    let codes: Vec<&str> = resp.problems.iter().map(|p| p["code"].as_str().unwrap()).collect();
    let count = |c: &str| codes.iter().filter(|x| **x == c).count();

    // Exactamente los mismos códigos que produce la gramática de
    // Compiscript ante los mismos errores conceptuales — con tokens y
    // producciones que no comparten un solo nombre con aquella.
    assert_eq!(count("S006"), 3, "tipos incompatibles: propiedad, inicializador y asignación: {codes:?}");
    assert_eq!(count("S007"), 2, "objeto desconocido en `crear` y como anotación de tipo: {codes:?}");
    assert_eq!(count("S009"), 1, "`propio` fuera de un método: {codes:?}");
    assert_eq!(count("S010"), 4, "miembros inexistentes (1 lectura, 2 asignaciones y 1 campo de literal): {codes:?}");
    assert_eq!(count("S011"), 1, "aridad incorrecta del constructor `iniciar`: {codes:?}");
    assert_eq!(count("S012"), 1, "tipo de argumento incorrecto en `crear`: {codes:?}");
    assert_eq!(count("S013"), 3, "aridad incorrecta en llamada a método, a función libre y recursiva: {codes:?}");
    assert_eq!(count("S014"), 1, "tipo de argumento incorrecto en llamada: {codes:?}");
    assert_eq!(count("S015"), 1, "operación aritmética inválida: {codes:?}");
    assert_eq!(count("S016"), 1, "retorno de tipo incompatible: {codes:?}");
    assert_eq!(count("S017"), 1, "retorno vacío en un método con tipo declarado: {codes:?}");
    assert_eq!(count("S018"), 1, "valor retornado desde un procedimiento: {codes:?}");
    assert_eq!(count("S019"), 1, "`retorna` fuera de todo método: {codes:?}");
    assert_eq!(count("S022"), 1, "campo de registro mal tipado: {codes:?}");
    assert_eq!(count("S023"), 1, "campo de registro faltante: {codes:?}");
    assert_eq!(count("S024"), 1, "campo de registro repetido: {codes:?}");
    assert_eq!(codes.len(), 24, "sin diagnósticos de más: {:#?}", resp.problems);

    for p in &resp.problems {
        assert!(p["loc"].as_str().unwrap().starts_with("objetos_es_errores.txt:"));
        assert!(p["line"].as_u64().unwrap() > 0, "línea real, no 0");
    }
}
