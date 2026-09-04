//! Batería de casos exitosos y fallidos, uno por regla semántica.
//!
//! El archivo `workspace/casos_semanticos.txt` es el MISMO que el usuario
//! carga en el IDE: cada línea es un programa completo e independiente —un
//! bloque `{ ... }`— así que el panel de TEST CASES muestra una entrada por
//! caso y se pueden ejecutar de a uno. Acá se analizan todos en una sola
//! petición al pipeline, aprovechando que la concatenación de bloques también
//! es un programa válido.
//!
//! Cada caso debe producir EXACTAMENTE el diagnóstico de su regla: ni uno de
//! más (nada de errores derivados) ni uno de menos.
use lexer_generator::api;

const YAL: &str = include_str!("../workspace/compiscript.yal");
const YALP: &str = include_str!("../workspace/compiscript.yalp");
const CASES: &str = include_str!("../workspace/casos_semanticos.txt");

/// Lo que debe salir en cada línea, en orden. `None` = caso exitoso.
/// Mantener alineado con `workspace/casos_semanticos.README.md`.
const EXPECTED: [Option<&str>; 44] = [
    None,           // 1  ámbito: declarar y leer
    Some("S002"),   // 2  variable no declarada
    Some("S001"),   // 3  redeclaración en el mismo ámbito
    Some("S005"),   // 4  asignación a constante
    None,           // 5  tipos: aritmética válida
    Some("S006"),   // 6  inicializador incompatible con el tipo declarado
    Some("S015"),   // 7  aritmética entre integer y string
    None,           // 8  clases: atributo accedido con `.`
    Some("S010"),   // 9  miembro inexistente
    Some("S007"),   // 10 clase desconocida en una anotación de tipo
    Some("S008"),   // 11 clase padre inexistente
    Some("S009"),   // 12 `this` fuera del ámbito de una clase
    None,           // 13 constructor correcto
    Some("S011"),   // 14 aridad incorrecta del constructor
    Some("S012"),   // 15 tipo de argumento incorrecto del constructor
    None,           // 16 llamada correcta a función libre
    Some("S013"),   // 17 aridad incorrecta en la llamada
    Some("S014"),   // 18 tipo de argumento incorrecto en la llamada
    Some("S016"),   // 19 return con tipo distinto al declarado
    Some("S017"),   // 20 return sin valor en función tipada
    Some("S018"),   // 21 return con valor en un procedimiento
    Some("S019"),   // 22 return fuera de toda función
    None,           // 23 función anidada que captura su entorno (closure)
    None,           // 24 struct: literal correcto y acceso a campo
    Some("S022"),   // 25 campo de struct mal tipado
    Some("S023"),   // 26 campo de struct faltante
    Some("S024"),   // 27 campo de struct repetido
    None,           // 28 control de flujo: while con condición booleana
    Some("S025"),   // 29 condición que no es booleana
    Some("S026"),   // 30 break fuera de un bucle
    Some("S027"),   // 31 continue fuera de un bucle
    None,           // 32 operadores: lógico sobre booleanos
    Some("S028"),   // 33 operando no booleano en un operador lógico
    Some("S029"),   // 34 comparación entre tipos incompatibles
    Some("S030"),   // 35 operando inválido para un unario
    Some("S031"),   // 36 el nombre de una función no es un valor
    None,           // 37 listas: literal homogéneo e indexado
    Some("S032"),   // 38 elementos heterogéneos en el literal
    Some("S033"),   // 39 índice que no es entero
    Some("S034"),   // 40 se indexa algo que no es un arreglo
    None,           // 41 switch con un case compatible
    Some("S035"),   // 42 case incompatible con el discriminante
    Some("S036"),   // 43 foreach sobre algo que no es una colección
    Some("W002"),   // 44 código inalcanzable tras un return
];

#[test]
fn cada_regla_semantica_tiene_su_caso_exitoso_y_su_caso_fallido() {
    assert_eq!(
        CASES.lines().count(),
        EXPECTED.len(),
        "la tabla de esperados debe cubrir todas las líneas del archivo"
    );

    // Las dos estrategias de parseo que soportan análisis semántico. LL(1)
    // queda fuera a propósito: renombra producciones y el SemanticSpec dejaría
    // de encontrarlas.
    for mode in ["lalr", "slr"] {
        let response =
            api::build_pipeline_response_named(YAL, YALP, CASES, mode, "casos_semanticos.txt")
                .expect("la batería debe correr por el mismo pipeline que usa el IDE");
        assert!(
            response.accepted,
            "{mode}: todos los casos son válidos sintácticamente, los errores son semánticos: {:?}",
            response.error
        );

        let mut por_linea: Vec<Vec<&str>> = vec![Vec::new(); EXPECTED.len() + 1];
        for problem in &response.problems {
            let line = problem["line"].as_u64().unwrap_or(0) as usize;
            assert!(line >= 1 && line <= EXPECTED.len(), "{mode}: línea fuera de rango: {problem:#?}");
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
fn la_bateria_cubre_todos_los_codigos_que_el_analizador_puede_emitir() {
    // Red de seguridad: si alguien agrega una regla nueva con su código, este
    // test obliga a agregarle también su caso a la batería del IDE.
    let cubiertos: Vec<&str> = EXPECTED.iter().flatten().copied().collect();
    for code in [
        "S001", "S002", "S005", "S006", "S007", "S008", "S009", "S010", "S011", "S012", "S013",
        "S014", "S015", "S016", "S017", "S018", "S019", "S022", "S023", "S024", "S025", "S026",
        "S027", "S028", "S029", "S030", "S031", "S032", "S033", "S034", "S035", "S036", "W002",
    ] {
        assert!(cubiertos.contains(&code), "falta un caso para {code} en la batería");
    }
}
