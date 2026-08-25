// Prueba end-to-end sobre una TERCERA gramática, "Pascalito", escrita desde
// cero y con una FORMA sintáctica distinta de las otras dos: sin llaves
// (bloques `is ... end`), asignación con `:=`, literal de registro con
// corchetes (`Punto[ x := 1 ]`), `if e then ... end` y `loop e do ... end`.
//
// `objetos_es` prueba que los NOMBRES de tokens y producciones son libres;
// esta prueba que la FORMA también lo es. El código Rust del analizador es el
// mismo: toda la diferencia vive en las directivas del `.yalp`.
//
// Corre por el pipeline REAL (`api::build_pipeline_response_named`), igual que
// `tests/compiscript_tests.rs` y `tests/gramatica_agnostica_tests.rs`.
use lexer_generator::api;
use serde_json::Value;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

fn yal() -> String {
    read("examples/lexer/pascalito.yal")
}

fn yalp() -> String {
    read("examples/grammar/pascalito.yalp")
}

fn analizar(source_path: &str, source_name: &str) -> api::ParseResponse {
    api::build_pipeline_response_named(&yal(), &yalp(), &read(source_path), "lalr", source_name)
        .expect("el pipeline no debe fallar internamente")
}

fn codigos(problems: &[Value]) -> Vec<String> {
    problems
        .iter()
        .map(|p| p["code"].as_str().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn pascalito_grammar_compiles_without_lalr_conflicts() {
    let compile = api::build_compile_response(&yalp(), "lalr").expect("pascalito.yalp debe compilar");
    let warnings: Vec<&str> = compile
        .problems
        .iter()
        .filter(|p| p.level == "warn")
        .map(|p| p.msg.as_str())
        .collect();
    assert!(warnings.is_empty(), "no debe haber conflictos LALR: {warnings:?}");
}

#[test]
fn pascalito_valid_program_has_zero_diagnostics() {
    let resp = analizar("examples/source/pascalito.txt", "pascalito.txt");

    assert!(resp.accepted, "debe ser sintácticamente válido: {:?}", resp.error);
    assert!(
        resp.problems.is_empty(),
        "una gramática con otra forma debe analizarse igual de limpio: {:#?}",
        resp.problems
    );
}

#[test]
fn pascalito_symbol_table_distinguishes_records_from_classes() {
    let resp = analizar("examples/source/pascalito.txt", "pascalito.txt");
    let tabla = &resp.symbol_table;

    // Registros: `Struct`, no `Class`, con sus campos tipados — incluido uno
    // cuyo tipo es OTRO registro.
    assert!(tabla.contains("Punto: Struct"), "Punto debe ser un Struct:\n{tabla}");
    assert!(tabla.contains("Caja: Struct"), "Caja debe ser un Struct:\n{tabla}");
    assert!(tabla.contains("esquina: Variable, Named(\"Punto\")"), "campo de tipo registro:\n{tabla}");

    // Clases: `Class`, con `self` tipado dentro de cada método.
    assert!(tabla.contains("Figura: Class"), "{tabla}");
    assert!(tabla.contains("Circulo: Class"), "{tabla}");
    assert!(tabla.contains("this: Variable, Named(\"Circulo\")"), "self tipado en el método:\n{tabla}");

    // Constante declarada con `let`.
    assert!(tabla.contains("FIJA: Variable, Int, const"), "{tabla}");
}

#[test]
fn pascalito_detects_the_closure_and_not_the_class_attributes() {
    let resp = analizar("examples/source/pascalito.txt", "pascalito.txt");

    // Exactamente UNA closure: `incrementar` cierra sobre `total` de
    // `contador`. Los métodos que leen un atributo de su propia clase
    // (`obtenerRadio` con `radio`) NO son capturas — es un `self` implícito.
    assert_eq!(resp.closures.len(), 1, "closures: {:#?}", resp.closures);
    let closure = &resp.closures[0];
    assert_eq!(closure["function"], "incrementar");
    let capturados: Vec<&str> =
        closure["captures"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap()).collect();
    assert_eq!(capturados, vec!["total"]);
}

#[test]
fn pascalito_reports_the_same_diagnostic_codes_as_the_other_grammars() {
    let resp = analizar("examples/source/pascalito_errores.txt", "pascalito_errores.txt");
    assert!(resp.accepted, "el archivo de errores debe parsear: {:?}", resp.error);

    let codes = codigos(&resp.problems);
    let count = |c: &str| codes.iter().filter(|x| x.as_str() == c).count();

    // Ámbito
    assert_eq!(count("S001"), 1, "redeclaración en el mismo ámbito: {codes:?}");
    assert_eq!(count("S002"), 1, "uso de un nombre no declarado: {codes:?}");
    // Tipos
    assert_eq!(count("S005"), 1, "asignación a una constante: {codes:?}");
    assert_eq!(count("S006"), 2, "inicializador y asignación incompatibles: {codes:?}");
    assert_eq!(count("S015"), 1, "operación aritmética inválida: {codes:?}");
    // Clases
    assert_eq!(count("S007"), 1, "anotación de tipo inexistente: {codes:?}");
    assert_eq!(count("S008"), 1, "herencia de una clase inexistente: {codes:?}");
    assert_eq!(count("S009"), 1, "`self` fuera de un método: {codes:?}");
    assert_eq!(count("S010"), 2, "miembro inexistente y campo inexistente en literal: {codes:?}");
    assert_eq!(count("S011"), 1, "aridad del constructor `init`: {codes:?}");
    assert_eq!(count("S012"), 1, "tipo de argumento del constructor: {codes:?}");
    // Funciones
    assert_eq!(count("S013"), 1, "aridad de la llamada: {codes:?}");
    assert_eq!(count("S014"), 1, "tipo de argumento de la llamada: {codes:?}");
    assert_eq!(count("S016"), 1, "retorno de tipo incompatible: {codes:?}");
    assert_eq!(count("S017"), 1, "`give` vacío en función tipada: {codes:?}");
    assert_eq!(count("S018"), 1, "valor devuelto desde un procedimiento: {codes:?}");
    assert_eq!(count("S019"), 1, "`give` fuera de toda función: {codes:?}");
    // Registros
    assert_eq!(count("S022"), 1, "campo de registro mal tipado: {codes:?}");
    assert_eq!(count("S023"), 1, "campo de registro faltante: {codes:?}");
    assert_eq!(count("S024"), 1, "campo de registro repetido: {codes:?}");

    assert_eq!(codes.len(), 22, "sin diagnósticos de más: {:#?}", resp.problems);

    // Todos ubicados en el archivo real, con línea y columna verdaderas.
    for p in &resp.problems {
        let loc = p["loc"].as_str().unwrap_or_default();
        assert!(loc.starts_with("pascalito_errores.txt:"), "loc inesperado: {loc}");
        assert!(p["line"].as_u64().unwrap() > 0, "línea real, no 0: {p:#?}");
        assert!(p["col"].as_u64().unwrap() > 0, "columna real, no 0: {p:#?}");
    }
}
