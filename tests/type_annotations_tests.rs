//! El árbol de análisis ANOTADO: el tipo inferido de cada nodo de expresión.
//!
//! Es lo que recibiría una fase de generación de código intermedio. El libro
//! del dragón (cap. 6) plantea esa fase como una SDD sobre el árbol donde la
//! regla de `E -> E1 + E2` necesita el tipo de cada operando para decidir qué
//! instrucción emitir y dónde hace falta una ampliación; sin estas
//! anotaciones ese dato se calculaba durante el análisis y se tiraba.
//!
//! Estos tests cubren que la anotación exista, que sea correcta por nodo y
//! que el `id` de cada fila apunte al nodo real del DOT. Que anotar no cambie
//! NINGÚN diagnóstico lo cubre la suite completa, no un test de acá.
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

/// Los tipos anotados en una línea concreta del fuente, sin orden.
fn tipos_en_linea(types: &[serde_json::Value], line: u64) -> Vec<String> {
    types
        .iter()
        .filter(|t| t["line"].as_u64() == Some(line))
        .map(|t| t["ty"].as_str().unwrap_or("?").to_string())
        .collect()
}

fn compiscript(source: &str, mode: &str) -> api::ParseResponse {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    api::build_pipeline_response_named(&yal, &yalp, source, mode, "anotaciones.cps")
        .expect("el pipeline no debe fallar internamente")
}

#[test]
fn cada_nodo_de_una_expresion_queda_anotado_con_su_tipo() {
    let r = compiscript("let x: integer = 1 + 2;\n", "lalr");
    assert!(r.accepted, "debe parsear: {:?}", r.error);
    assert!(r.problems.is_empty(), "no debe haber diagnósticos: {:#?}", r.problems);

    let tipos = tipos_en_linea(&r.types, 1);
    assert!(!tipos.is_empty(), "la suma debe quedar anotada: {:#?}", r.types);
    assert!(
        tipos.iter().all(|t| t == "integer"),
        "toda la cadena de la suma es entera: {tipos:?}"
    );

    // Las dos hojas INT_LIT tambien, no solo los nodos internos: codegen
    // necesita el tipo de cada operando, no solo el del resultado.
    let hojas: Vec<&serde_json::Value> =
        r.types.iter().filter(|t| t["symbol"] == "INT_LIT").collect();
    assert_eq!(hojas.len(), 2, "las dos hojas INT_LIT deben estar anotadas: {hojas:#?}");
}

#[test]
fn un_nodo_puede_tener_otro_tipo_que_sus_hijos() {
    // `x > 4`: la comparacion es bool aunque sus dos operandos sean enteros.
    // Es justo la propiedad que hace falta para codegen — el tipo del padre no
    // se deduce mirando solo la forma del nodo, hay que haberlo calculado.
    let r = compiscript("let x: integer = 1;\nlet y: boolean = x > 4;\n", "lalr");
    assert!(r.accepted, "debe parsear: {:?}", r.error);
    assert!(r.problems.is_empty(), "no debe haber diagnósticos: {:#?}", r.problems);

    let tipos = tipos_en_linea(&r.types, 2);
    assert!(
        tipos.iter().any(|t| t == "bool"),
        "la comparación debe anotarse como bool: {tipos:?}"
    );
    assert!(
        tipos.iter().any(|t| t == "integer"),
        "sus operandos siguen siendo enteros: {tipos:?}"
    );
}

#[test]
fn el_id_de_cada_anotacion_apunta_a_ese_nodo_en_el_dot() {
    let r = compiscript("let x: integer = 1 + 2;\n", "lalr");
    assert!(!r.types.is_empty(), "debe haber anotaciones");

    for t in &r.types {
        let id = t["id"].as_str().expect("cada fila trae su id");
        let ty = t["ty"].as_str().expect("cada fila trae su tipo");
        // El nodo tiene que existir en el grafo...
        let declaracion = format!("    {id} [label=\"");
        let linea = r
            .parse_tree_dot
            .lines()
            .find(|l| l.starts_with(&declaracion))
            .unwrap_or_else(|| panic!("{id} debe existir en el DOT:\n{}", r.parse_tree_dot));
        // ...y llevar dibujado el MISMO tipo que dice la tabla.
        assert!(
            linea.contains(&format!("\\n: {ty}")),
            "{id} debe mostrar `: {ty}` en su etiqueta, salió: {linea}"
        );
    }
}

#[test]
fn el_dot_anotado_sigue_siendo_un_grafo_valido() {
    let r = compiscript("let x: integer = 1 + 2;\n", "lalr");
    let dot = &r.parse_tree_dot;

    assert!(dot.starts_with("digraph ParseTree"), "cabecera intacta");
    assert!(dot.trim_end().ends_with('}'), "cierre intacto");
    assert!(dot.contains("->"), "las aristas siguen ahí");
    assert!(dot.contains("programa"), "los símbolos siguen en las etiquetas");
    // El salto de linea de Graphviz es el `\n` LITERAL de dos caracteres, no
    // un salto real: un salto real partiria la declaracion del nodo en dos.
    assert!(
        !dot.lines().any(|l| l.starts_with(": ")),
        "la anotación va dentro de la etiqueta, no en una línea suelta:\n{dot}"
    );
}

#[test]
fn sin_analisis_semantico_no_hay_anotaciones_y_el_arbol_sale_plano() {
    // `miniprog.yalp` no trae `%ident`, así que `SemanticSpec::from_grammar`
    // devuelve `None` y no se corre la fase semántica — mismo criterio que
    // `api_pipeline_tests`. El árbol tiene que salir igual que siempre: es la
    // rama que usa `to_dot` plano en vez de `to_dot_annotated`.
    //
    // El otro camino sin análisis es el modo LL(1) (la factorización renombra
    // las producciones y dejaría al `SemanticSpec` sin encontrarlas), pero no
    // se puede probar acá: ninguna de las gramáticas del proyecto con `%ident`
    // es LL(1), así que el pipeline falla antes de construir el árbol.
    let yal = read("workspace/miniprog.yal");
    let yalp = read("workspace/miniprog.yalp");
    let source = read("workspace/miniprog_test.txt");

    let r = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "miniprog.txt")
        .expect("el pipeline no debe fallar internamente");

    assert!(r.types.is_empty(), "sin %ident no hay anotaciones: {:#?}", r.types);
    assert!(!r.parse_tree_dot.is_empty(), "el árbol se construye igual");
    assert!(
        !r.parse_tree_dot.contains("\\n: "),
        "sin análisis no puede haber anotaciones dibujadas:\n{}",
        r.parse_tree_dot
    );
    assert!(r.parse_tree_dot.starts_with("digraph ParseTree"), "cabecera intacta");
}

#[test]
fn las_anotaciones_son_agnosticas_a_la_gramatica() {
    // La MISMA maquinaria sobre una gramática que no es Compiscript: sin una
    // sola línea de Rust específica de ella. Mismo criterio que
    // `tests/colecciones_tests.rs`.
    let yal = read("workspace/colecciones.yal");
    let yalp = read("workspace/colecciones.yalp");
    let source = "{ var a: entero[] = [1, 2, 3]; imprime(a[0]); }\n";

    let r = api::build_pipeline_response_named(&yal, &yalp, source, "lalr", "anot.txt")
        .expect("el pipeline no debe fallar internamente");
    assert!(r.accepted, "debe parsear: {:?}", r.error);
    assert!(r.problems.is_empty(), "no debe haber diagnósticos: {:#?}", r.problems);

    let tipos: Vec<&str> = r.types.iter().filter_map(|t| t["ty"].as_str()).collect();
    assert!(
        tipos.iter().any(|t| *t == "integer[]"),
        "el literal de arreglo debe anotarse como arreglo de enteros: {tipos:?}"
    );
    assert!(
        tipos.iter().any(|t| *t == "integer"),
        "sus elementos y el acceso indexado, como enteros: {tipos:?}"
    );
}
