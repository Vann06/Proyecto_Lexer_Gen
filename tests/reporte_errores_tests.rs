//! Errores que el compilador debe encontrar sin ejecutar el programa: los caminos
//! de una función sin `return`, y varios errores de sintaxis en un mismo archivo.
#[allow(dead_code)]
mod common;

use lexer_generator::api;

const S017: &str = "\"S017\"";

fn funcion(cuerpo: &str) -> String {
    format!("function f(n: integer): integer {{\n{cuerpo}\n}}\nprint(f(1));\n")
}

#[test]
fn una_funcion_con_algun_camino_sin_return_es_un_error() {
    let compilar = common::compilador();
    let casos = [
        ("sin return", "print(n);"),
        ("if sin else", "if (n > 0) { return 1; }"),
        ("una rama del if sin return", "if (n > 0) { return 1; } else { print(n); }"),
        ("else if sin else final", "if (n > 0) { return 1; } else if (n < 0) { return 2; }"),
        ("while con condicion", "while (n > 0) { return 1; }"),
        ("for con condicion", "for (let i: integer = 0; i < n; i = i + 1) { return i; }"),
        ("while true con break", "while (true) { if (n > 0) { break; } return 1; }"),
        ("do while con break", "do { if (n > 0) { break; } return 1; } while (n > 0);"),
        ("do while con continue", "do { if (n > 0) { continue; } return 1; } while (n > 0);"),
        ("foreach", "foreach (x in [1, 2]) { return x; }"),
        ("switch sin default", "switch (n) { case 1: return 1; case 2: return 2; }"),
        ("switch con break", "switch (n) { case 1: break; default: return 2; }"),
        ("switch que cae a un case sin return", "switch (n) { default: return 1; case 2: print(n); }"),
        ("try sin return en el catch", "try { return 1; } catch (e) { print(e); }"),
        ("catch sin return en el try", "try { print(n); } catch (e) { return 1; }"),
        ("return de una funcion anidada", "function g(): integer { return 1; }\nprint(g());"),
    ];
    for (nombre, cuerpo) in casos {
        let error = compilar(&funcion(cuerpo)).err().unwrap_or_else(|| format!("{nombre}: compiló"));
        assert!(error.contains(S017), "{nombre}: {error}");
    }
}

#[test]
fn una_funcion_que_retorna_en_todos_sus_caminos_compila() {
    let compilar = common::compilador();
    let casos = [
        ("return directo", "return 1;"),
        ("if y else", "if (n > 0) { return 1; } else { return 2; }"),
        ("else if con else", "if (n > 0) { return 1; } else if (n < 0) { return 2; } else { return 3; }"),
        ("return despues del if", "if (n > 0) { return 1; }\nreturn 2;"),
        ("bloque suelto", "{ return 1; }"),
        ("while true", "while (true) { if (n > 0) { return 1; } n = n + 1; }"),
        ("break de un ciclo interno", "while (true) { while (n > 0) { break; } return 1; }"),
        ("break de un switch interno", "while (true) { switch (n) { case 1: break; } return 1; }"),
        ("continue en while true", "while (true) { if (n > 0) { continue; } return 1; }"),
        ("for true", "for (let i: integer = 0; true; i = i + 1) { if (i > n) { return i; } }"),
        ("do while", "do { return 1; } while (n > 0);"),
        ("switch con default", "switch (n) { case 1: return 1; default: return 2; }"),
        ("switch que cae", "switch (n) { case 1: print(n); case 2: print(n); default: return 2; }"),
        ("default antes de un case", "switch (n) { default: print(n); case 2: return 2; }"),
        ("try y catch", "try { return 1; } catch (e) { return 2; }"),
    ];
    for (nombre, cuerpo) in casos {
        if let Err(error) = compilar(&funcion(cuerpo)) {
            panic!("{nombre}: {error}");
        }
    }
}

#[test]
fn solo_se_exige_return_si_la_funcion_declara_un_tipo() {
    let compilar = common::compilador();
    let fuente = "function p(n: integer) { print(n); }\n\
                  class A { var x: integer; function constructor(v: integer) { this.x = v; } function m() { print(1); } }\n\
                  p(1);\n";
    if let Err(error) = compilar(fuente) {
        panic!("{error}");
    }
}

#[test]
fn los_metodos_tambien_deben_retornar() {
    let compilar = common::compilador();
    let error = compilar("class A { function m(): integer { print(1); } }\n").unwrap_err();
    assert!(error.contains(S017), "{error}");
}

#[test]
fn un_error_en_la_primera_sentencia_no_oculta_los_siguientes() {
    let fuente = "let a: integer = 1\nprint(a + );\nlet b: integer = 2;\nprint(b + );\nlet c: integer = ;\n";
    let respuesta = api::build_pipeline_response(
        &common::read("workspace/compiscript.yal"),
        &common::read("workspace/compiscript.yalp"),
        fuente,
        "lalr",
    )
    .expect("el pipeline debe responder");

    let lineas: Vec<_> = respuesta.problems.iter().filter(|p| p["code"] == "P001").map(|p| p["line"].as_u64().unwrap()).collect();
    // El `;` que falta en la línea 1 se nota al llegar al `print` de la línea 2.
    assert_eq!(lineas, [2, 2, 4, 5], "{:#?}", respuesta.problems);
}

fn problemas(fuente: &str) -> Vec<serde_json::Value> {
    let respuesta = api::build_pipeline_response(
        &common::read("workspace/compiscript.yal"),
        &common::read("workspace/compiscript.yalp"),
        fuente,
        "lalr",
    )
    .expect("el pipeline debe responder");
    respuesta.problems
}

#[test]
fn un_error_de_sintaxis_dice_que_falto_sin_detalles_del_parser() {
    let mensajes: Vec<String> = problemas("let a: integer = 1\nprint(a + );\nif (a > 1 { print(a); }\nlet b: integer = (1;\nlet c")
        .iter()
        .map(|p| p["msg"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(mensajes[0], "Error sintáctico: no se esperaba 'print'. Esperado: ';'");
    assert!(mensajes[1].starts_with("Error sintáctico: no se esperaba ')'. Esperado: "), "{}", mensajes[1]);
    assert!(mensajes[1].ends_with(" más"), "resume una lista larga: {}", mensajes[1]);
    assert_eq!(mensajes[2], "Error sintáctico: no se esperaba '{'. Esperado: ')'");
    assert_eq!(mensajes[3], "Error sintáctico: no se esperaba ';'. Esperado: ')'");
    assert!(mensajes[4].contains("no se esperaba el fin del archivo"), "{}", mensajes[4]);
    assert!(mensajes.iter().all(|m| !m.contains("estado I") && !m.contains("SEMI")), "{mensajes:?}");
}

/// `W003` y su línea, para cada aviso de `fuente`.
fn avisos_sin_valor(fuente: &str) -> Vec<u64> {
    problemas(fuente).iter().filter(|p| p["code"] == "W003").map(|p| p["line"].as_u64().unwrap()).collect()
}

#[test]
fn leer_una_variable_sin_valor_avisa_una_vez_y_no_impide_compilar() {
    let fuente = "let u: integer;\nprint(u + 1);\nprint(u);\n";
    let todos = problemas(fuente);
    assert_eq!(todos.len(), 1, "{todos:#?}");
    assert_eq!((todos[0]["code"].as_str(), todos[0]["level"].as_str(), todos[0]["line"].as_u64()), (Some("W003"), Some("warn"), Some(2)));
}

#[test]
fn no_avisa_de_una_variable_que_tiene_valor() {
    let casos = [
        "let a: integer = 1;\nprint(a);\n",
        "let a: integer;\na = 1;\nprint(a);\n",
        "let a: integer;\nif (true) { a = 1; }\nprint(a);\n",
        "for (let i: integer = 0; i < 2; i = i + 1) { print(i); }\n",
        "foreach (x in [1, 2]) { print(x); }\n",
        "try { print(1); } catch (e) { print(e); }\n",
        "function f(n: integer): integer { return n; }\nprint(f(1));\n",
        "let a: integer;\nfunction f(): integer { return a; }\na = 1;\nprint(f());\n",
        "class A { var x: integer; function m(): integer { return this.x; } }\n",
    ];
    for fuente in casos {
        assert!(avisos_sin_valor(fuente).is_empty(), "{fuente}");
    }
}

fn pipeline(fuente: &str) -> api::ParseResponse {
    api::build_pipeline_response(
        &common::read("workspace/compiscript.yal"),
        &common::read("workspace/compiscript.yalp"),
        fuente,
        "lalr",
    )
    .expect("el pipeline debe responder")
}

/// La traza guardaba una copia de la entrada por paso: 500 líneas de `print`
/// agotaban la memoria del servidor.
#[test]
fn un_programa_largo_compila_y_su_traza_no_crece_con_el_cuadrado() {
    let respuesta = pipeline(&"print(1);\n".repeat(3000));

    assert!(respuesta.problems.is_empty(), "{:#?}", respuesta.problems);
    assert!(respuesta.accepted && !respuesta.mips.is_empty());
    assert_eq!(respuesta.trace.last().unwrap()["action"], "acc", "el último paso siempre se conserva");
    let traza = serde_json::to_string(&respuesta.trace).unwrap();
    assert!(traza.len() < 30_000_000, "la traza pesa {} bytes", traza.len());
}

#[test]
fn demasiado_anidamiento_es_un_error_y_no_tumba_el_proceso() {
    for fuente in [
        format!("print({}1{});\n", "(".repeat(300), ")".repeat(300)),
        format!("{}print(1);{}\n", "{".repeat(300), "}".repeat(300)),
        format!("print({}1{});\n", "[".repeat(100_000), "]".repeat(100_000)),
    ] {
        let respuesta = pipeline(&fuente);
        assert_eq!(respuesta.problems.len(), 1, "{:#?}", respuesta.problems);
        assert_eq!(respuesta.problems[0]["code"], "P002");
        assert!(!respuesta.accepted);
    }
}

#[test]
fn un_anidamiento_dentro_del_limite_compila() {
    let respuesta = pipeline(&format!("print({}1{});\n", "(".repeat(190), ")".repeat(190)));
    assert!(respuesta.problems.is_empty(), "{:#?}", respuesta.problems);
    assert!(!respuesta.mips.is_empty());
}

#[test]
fn un_programa_con_demasiados_tokens_es_un_error() {
    let respuesta = pipeline(&"print(1);\n".repeat(11_000));
    assert_eq!(respuesta.problems.len(), 1, "{:#?}", respuesta.problems);
    assert_eq!(respuesta.problems[0]["code"], "P002");
}

/// El árbol de una suma es tan profundo como larga sea, y todo lo que lo recorre es
/// recursivo. No usa el máximo de tokens porque ese tamaño tarda minutos.
#[test]
fn una_expresion_muy_larga_no_agota_la_pila() {
    let respuesta = pipeline(&format!("print(1{});
", " + 1".repeat(3000)));
    assert!(respuesta.problems.is_empty(), "{:#?}", respuesta.problems);
    assert!(respuesta.accepted);
}
