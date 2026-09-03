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
fn ejemplo_closures_cps_detects_the_expected_capture_and_typed_fields() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    let source = read("workspace/ejemplo_closures.cps");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "ejemplo_closures.cps")
        .expect("el pipeline no debe fallar internamente");

    assert!(resp.accepted, "ejemplo_closures.cps debe ser válido: {:?} / problems: {:#?}", resp.error, resp.problems);
    // Sin diagnósticos semánticos: todo lo usado está declarado correctamente.
    let sem_errors: Vec<&serde_json::Value> = resp.problems.iter().filter(|p| p["code"].as_str().unwrap_or("").starts_with('S')).collect();
    assert!(sem_errors.is_empty(), "no debería haber errores semánticos: {sem_errors:#?}");

    // "incrementar" es una función anidada dentro de "contador" que reasigna
    // y lee "total" (declarada en el entorno de "contador", no en el suyo
    // propio) — debe aparecer como la ÚNICA closure detectada, capturando
    // exactamente "total" (su propio parámetro "paso" NO es una captura).
    assert_eq!(resp.closures.len(), 1, "closures: {:#?}", resp.closures);
    let closure = &resp.closures[0];
    assert_eq!(closure["function"], "incrementar");
    let captured_names: Vec<&str> = closure["captures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(captured_names, vec!["total"]);
    // La captura trae línea/columna reales del uso que la reveló (no 0:0).
    let first_capture = &closure["captures"][0];
    assert!(first_capture["line"].as_u64().unwrap() > 0);
    assert!(first_capture["col"].as_u64().unwrap() > 0);

    // Records/structs definidos por el usuario (reutilizan class_decl):
    // Punto se declara como Class y sus campos x/y quedan tipados como Int
    // reales (no solo declarados) vía la misma directiva %type_of que
    // cualquier otra declaración.
    assert!(resp.symbol_table.contains("Punto: Class"), "{}", resp.symbol_table);
    assert!(resp.symbol_table.contains("x: Variable, Int"), "{}", resp.symbol_table);
    assert!(resp.symbol_table.contains("y: Variable, Int"), "{}", resp.symbol_table);
}

/// "Detección de redeclaración de funciones" era el único punto del
/// entregable de funciones sin cobertura: `SymbolTable::declare` rechaza
/// cualquier nombre repetido en el mismo ámbito sin mirar el `SymbolKind`, así
/// que las funciones lo heredan gratis — pero nada lo verificaba, y "funciona
/// por herencia de otro mecanismo" es exactamente el tipo de supuesto que se
/// rompe en silencio. Redeclarar en un ÁMBITO DISTINTO sigue siendo legal
/// (shadowing), y eso también se comprueba acá para no convertir el chequeo en
/// uno demasiado agresivo.
#[test]
fn redeclaring_a_function_in_the_same_scope_is_reported_but_shadowing_is_not() {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");

    let duplicada = "\
function calcular(a: integer): integer {
  return a;
}

function calcular(b: integer): integer {
  return b;
}
";

    let resp = api::build_pipeline_response_named(&yal, &yalp, duplicada, "lalr", "dup.cps")
        .expect("el pipeline no debe fallar internamente");
    assert!(resp.accepted, "debe ser sintácticamente válido: {:?}", resp.error);

    let redeclaraciones: Vec<&serde_json::Value> =
        resp.problems.iter().filter(|p| p["code"] == "S001").collect();
    assert_eq!(
        redeclaraciones.len(),
        1,
        "la segunda `calcular` debe reportar S001: {:#?}",
        resp.problems
    );
    let err = redeclaraciones[0];
    assert_eq!(err["line"].as_u64().unwrap(), 5, "debe señalar la SEGUNDA declaración");
    assert!(err["loc"].as_str().unwrap().starts_with("dup.cps:"));

    // Misma firma, distinto ámbito: la interna hace shadowing de la global.
    let anidada = "\
function externa(): integer {
  function interna(a: integer): integer {
    return a;
  }
  return 0;
}

function interna(b: integer): integer {
  return b;
}
";

    let resp = api::build_pipeline_response_named(&yal, &yalp, anidada, "lalr", "shadow.cps")
        .expect("el pipeline no debe fallar internamente");
    assert!(resp.accepted, "debe ser sintácticamente válido: {:?}", resp.error);
    let redeclaraciones: Vec<&serde_json::Value> =
        resp.problems.iter().filter(|p| p["code"] == "S001").collect();
    assert!(
        redeclaraciones.is_empty(),
        "declarar el mismo nombre en ámbitos distintos es shadowing, no redeclaración: {:#?}",
        resp.problems
    );
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

/// Listas y arreglos (Proyecto 2): tipo homogéneo de los elementos,
/// validación de índices, arrays multidimensionales y tipado de literales.
/// Ver `src/semantico/collections/mod.rs`.
mod arrays {
    use super::*;

    fn run(source: &str) -> lexer_generator::api::ParseResponse {
        let yal = read("workspace/compiscript.yal");
        let yalp = read("workspace/compiscript.yalp");
        api::build_pipeline_response_named(&yal, &yalp, source, "lalr", "arr.cps")
            .expect("el pipeline no debe fallar internamente")
    }

    fn semantic_codes(resp: &lexer_generator::api::ParseResponse) -> Vec<String> {
        resp.problems
            .iter()
            .filter(|p| p["code"].as_str().unwrap_or("").starts_with('S'))
            .map(|p| p["code"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn homogeneous_literal_declared_and_indexed_without_errors() {
        let resp = run(
            "let arr: integer[] = [1, 2, 3];\n\
             let primero: integer = arr[0];\n",
        );
        assert!(resp.accepted, "{:?}", resp.error);
        assert!(semantic_codes(&resp).is_empty(), "no debería haber errores semánticos: {:#?}", resp.problems);
    }

    #[test]
    fn heterogeneous_literal_reports_s032() {
        let resp = run("let arr = [1, \"dos\"];\n");
        assert!(resp.accepted, "{:?}", resp.error);
        assert_eq!(semantic_codes(&resp), vec!["S032"], "problemas: {:#?}", resp.problems);
    }

    #[test]
    fn mixed_integer_and_float_literal_widens_without_error() {
        // El literal debe tipar como float[] (ensanchado), no reportar S032.
        let resp = run("let arr: integer[] = [1, 2, 3];\nlet otro = [1, 2];\n");
        assert!(resp.accepted, "{:?}", resp.error);
        assert!(semantic_codes(&resp).is_empty(), "{:#?}", resp.problems);
    }

    #[test]
    fn indexing_with_a_non_integer_reports_s033() {
        let resp = run(
            "let arr: integer[] = [1, 2, 3];\n\
             let x = arr[\"a\"];\n",
        );
        assert!(resp.accepted, "{:?}", resp.error);
        assert_eq!(semantic_codes(&resp), vec!["S033"], "problemas: {:#?}", resp.problems);
    }

    #[test]
    fn indexing_a_non_array_reports_s034() {
        let resp = run(
            "let x: integer = 5;\n\
             let y = x[0];\n",
        );
        assert!(resp.accepted, "{:?}", resp.error);
        assert_eq!(semantic_codes(&resp), vec!["S034"], "problemas: {:#?}", resp.problems);
    }

    #[test]
    fn two_dimensional_array_indexes_twice_to_the_element_type() {
        let resp = run(
            "let m: integer[][] = [[1, 2], [3, 4]];\n\
             let x: integer = m[0][1];\n",
        );
        assert!(resp.accepted, "{:?}", resp.error);
        assert!(semantic_codes(&resp).is_empty(), "{:#?}", resp.problems);
    }

    #[test]
    fn assigning_a_mistyped_literal_to_a_declared_array_type_reports_s006() {
        let resp = run("let arr: integer[] = [\"a\", \"b\"];\n");
        assert!(resp.accepted, "{:?}", resp.error);
        assert_eq!(semantic_codes(&resp), vec!["S006"], "problemas: {:#?}", resp.problems);
    }
}
