//! Mapa, conjunto y tupla sobre una gramática que NO es Compiscript.
//!
//! Compiscript no tiene sintaxis de diccionario, conjunto ni tupla, así que
//! `workspace/colecciones.yal`/`.yalp` definen un lenguaje mínimo que sí la
//! tiene. Es la prueba empírica de que el soporte de colecciones del
//! analizador es **configuración**, no código por lenguaje: las mismas reglas
//! de `src/semantico/collections` validan estas cuatro colecciones sin una
//! sola línea de Rust específica de esta gramática.
//!
//! `workspace/colecciones_casos.txt` es el MISMO archivo que se carga en el
//! IDE: cada línea es un programa independiente, así que el panel de TEST
//! CASES muestra una entrada por caso.
use lexer_generator::api;

const YAL: &str = include_str!("../workspace/colecciones.yal");
const YALP: &str = include_str!("../workspace/colecciones.yalp");
const CASES: &str = include_str!("../workspace/colecciones_casos.txt");

/// Lo que debe salir en cada línea, en orden. `None` = caso exitoso.
/// Mantener alineado con `workspace/colecciones_casos.README.md`.
const EXPECTED: [Option<&str>; 22] = [
    None,           // 1  arreglo: literal homogéneo e indexado
    Some("S032"),   // 2  arreglo con elementos heterogéneos
    Some("S033"),   // 3  arreglo indexado con algo que no es entero
    None,           // 4  arreglo bidimensional, indexado dos veces
    None,           // 5  conjunto: literal homogéneo
    Some("S032"),   // 6  conjunto con elementos heterogéneos
    Some("S034"),   // 7  un conjunto NO es indexable
    Some("S006"),   // 8  conjunto de enteros asignado a conjunto de textos
    None,           // 9  mapa: literal y acceso por clave correcta
    Some("S037"),   // 10 mapa indexado con una clave del tipo equivocado
    Some("S032"),   // 11 mapa con claves heterogéneas
    Some("S032"),   // 12 mapa con valores heterogéneos
    Some("S006"),   // 13 el acceso devuelve el tipo del VALOR, no el de la clave
    None,           // 14 tupla: literal heterogéneo, que es lo normal en una tupla
    None,           // 15 tupla indexada por posición: t[1] es entero
    Some("S006"),   // 16 la MISMA tupla en otra posición es otro tipo: t[0] es texto
    Some("S038"),   // 17 índice literal fuera del rango de la tupla
    None,           // 18 iterar un conjunto recorre sus elementos
    None,           // 19 iterar un mapa recorre sus CLAVES
    Some("S036"),   // 20 una tupla no es iterable: es heterogénea
    None,           // 21 mapa de arreglos: colecciones anidadas
    Some("S006"),   // 22 dos conjuntos de tipos distintos no son compatibles
];

#[test]
fn las_cuatro_colecciones_se_validan_en_una_gramatica_ajena_a_compiscript() {
    assert_eq!(
        CASES.lines().count(),
        EXPECTED.len(),
        "la tabla de esperados debe cubrir todas las líneas del archivo"
    );

    for mode in ["lalr", "slr"] {
        let response =
            api::build_pipeline_response_named(YAL, YALP, CASES, mode, "colecciones_casos.txt")
                .expect("la batería debe correr por el mismo pipeline que usa el IDE");
        assert!(
            response.accepted,
            "{mode}: todos los casos son válidos sintácticamente: {:?}",
            response.error
        );

        let mut por_linea: Vec<Vec<&str>> = vec![Vec::new(); EXPECTED.len() + 1];
        for problem in &response.problems {
            let line = problem["line"].as_u64().unwrap_or(0) as usize;
            assert!(line >= 1 && line <= EXPECTED.len(), "{mode}: fuera de rango: {problem:#?}");
            por_linea[line].push(problem["code"].as_str().unwrap_or("?"));
        }

        for (index, expected) in EXPECTED.iter().enumerate() {
            let numero = index + 1;
            let obtenidos = &por_linea[numero];
            let caso = CASES.lines().nth(index).unwrap_or("");
            match expected {
                Some(code) => assert_eq!(
                    obtenidos,
                    &vec![*code],
                    "{mode}: caso {numero} debía reportar solo {code}\n  {caso}"
                ),
                None => assert!(
                    obtenidos.is_empty(),
                    "{mode}: caso {numero} es correcto y no debía reportar nada, salió {obtenidos:?}\n  {caso}"
                ),
            }
        }
    }
}

#[test]
fn la_gramatica_de_colecciones_compila_sin_conflictos_lalr() {
    let compile = api::build_compile_response(YALP, "lalr").expect("debe compilar");
    let warnings: Vec<&str> = compile
        .problems
        .iter()
        .filter(|p| p.level == "warn")
        .map(|p| p.msg.as_str())
        .collect();
    assert!(warnings.is_empty(), "no debe haber conflictos LALR: {warnings:?}");
}
