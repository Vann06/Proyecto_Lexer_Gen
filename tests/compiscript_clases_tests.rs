// Prueba end-to-end de clases y objetos (Fase 16) sobre la gramática real de
// Compiscript: entorno de clase, atributos/métodos accedidos con `.` (con
// herencia), invocación de constructor (aridad + tipos simples), y `this`
// dentro/fuera de un método — corrido por el pipeline REAL
// (`api::build_pipeline_response_named`), mismo estilo que
// `tests/compiscript_tests.rs`.
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

#[test]
fn clases_ok_cps_has_zero_semantic_diagnostics() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/clases_ok.cps");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "clases_ok.cps")
        .expect("el pipeline no debe fallar internamente");

    assert!(resp.accepted, "clases_ok.cps debe ser sintácticamente válido: {:?} / problems: {:#?}", resp.error, resp.problems);
    assert!(resp.problems.is_empty(), "no debería haber ningún diagnóstico: {:#?}", resp.problems);

    // La clase Circulo quedó con su padre Figura registrado.
    assert!(resp.symbol_table.contains("Circulo"));
    assert!(resp.symbol_table.contains("Figura"));
}

#[test]
fn clases_errores_cps_reports_expected_codes() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/clases_errores.cps");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "clases_errores.cps")
        .expect("el pipeline no debe fallar internamente");

    assert!(resp.accepted, "clases_errores.cps debe ser sintácticamente válido: {:?} / problems: {:#?}", resp.error, resp.problems);

    let codes: Vec<&str> = resp.problems.iter().map(|p| p["code"].as_str().unwrap()).collect();

    let count = |c: &str| codes.iter().filter(|x| **x == c).count();

    // S005 AssignmentToConst — FIJA = 11
    assert_eq!(count("S005"), 1, "esperaba 1 asignación a constante: {codes:?}");
    // S006 AssignmentTypeMismatch — `this.radio = "..."` (propiedad),
    // `let malTipada: integer = "..."` (inicializador) y `entera = "..."`
    // (asignación a variable suelta).
    assert_eq!(count("S006"), 3, "esperaba 3 tipos incompatibles en asignación: {codes:?}");
    // S007 UnknownClass — `new NoDeclarada()` y la anotación de tipo
    // `let fantasma: ClaseInexistente` (esta última se valida al terminar el
    // recorrido, porque una clase puede declararse más abajo en el archivo).
    assert_eq!(count("S007"), 2, "esperaba 2 clases desconocidas: {codes:?}");
    // S009 ThisOutsideClass — this.area a nivel de programa
    assert_eq!(count("S009"), 1, "esperaba 1 uso de this fuera de clase: {codes:?}");
    // S010 UnknownMember — c.noExiste (lectura), this.noExisteEnLaClase y
    // c.noExisteTampoco (asignación: el destino tampoco puede inventarse).
    assert_eq!(count("S010"), 4, "esperaba 4 miembros inexistentes (3 accesos + 1 campo de literal): {codes:?}");
    // S011 ConstructorArityMismatch — new Circulo(1, 2)
    assert_eq!(count("S011"), 1, "esperaba 1 aridad incorrecta de constructor: {codes:?}");
    // S012 ConstructorArgTypeMismatch — new Circulo("texto")
    assert_eq!(count("S012"), 1, "esperaba 1 tipo de argumento incorrecto en constructor: {codes:?}");
    // S013 CallArityMismatch — c.escalar(1,2,3) (método heredado) y
    // duplicar(1,2) (función libre).
    assert_eq!(count("S013"), 3, "esperaba 3 aridades incorrectas de llamada (metodo, funcion libre y recursiva): {codes:?}");
    // S014 CallArgTypeMismatch — c.escalar("texto")
    assert_eq!(count("S014"), 1, "esperaba 1 tipo de argumento incorrecto en llamada: {codes:?}");
    // S015 InvalidArithmetic — entera + texto
    assert_eq!(count("S015"), 1, "esperaba 1 operación aritmética inválida: {codes:?}");
    assert_eq!(count("S016"), 1, "esperaba 1 retorno de tipo incompatible: {codes:?}");
    assert_eq!(count("S017"), 1, "esperaba 1 retorno vacío en función con tipo declarado: {codes:?}");
    assert_eq!(count("S018"), 1, "esperaba 1 valor retornado desde un procedimiento: {codes:?}");
    assert_eq!(count("S019"), 1, "esperaba 1 return fuera de toda función: {codes:?}");
    assert_eq!(count("S022"), 1, "esperaba 1 campo de struct mal tipado: {codes:?}");
    assert_eq!(count("S023"), 1, "esperaba 1 campo de struct faltante: {codes:?}");
    assert_eq!(count("S024"), 1, "esperaba 1 campo de struct repetido: {codes:?}");

    assert_eq!(codes.len(), 25, "no debería haber diagnósticos de más: {:#?}", resp.problems);

    // Todos con línea/columna reales y el nombre real del archivo.
    for p in &resp.problems {
        assert!(p["loc"].as_str().unwrap().starts_with("clases_errores.cps:"));
        assert!(p["line"].as_u64().unwrap() > 0);
        assert!(p["col"].as_u64().unwrap() > 0);
    }
}
