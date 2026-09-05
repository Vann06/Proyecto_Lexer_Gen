//! La especificación de Compiscript, ejercitada de punta a punta.
//!
//! `workspace/rubrica.cps` recorre las características que el documento del
//! lenguaje declara soportadas, con la sintaxis de sus propios ejemplos. El
//! test exige CERO diagnósticos: es la prueba reproducible de que el proyecto
//! cumple la especificación, y el archivo sirve de demostración.
//!
//! Dos ejemplos del documento se escriben acá con llaves —`if (n < 60) { continue; }`
//! y `if (n2 <= 1) { return 1; }`— porque la gramática exige un bloque en el
//! cuerpo de todo control de flujo. Es un límite conocido y deliberado: admitir
//! una sentencia suelta reintroduce el *dangling else*, que es una ambigüedad
//! LALR real. Ver `ARQUITECTURA.md`, sección de límites conocidos.
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

fn analizar(source: &str, name: &str) -> api::ParseResponse {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");
    api::build_pipeline_response_named(&yal, &yalp, source, "lalr", name)
        .expect("el pipeline no debe fallar internamente")
}

fn codigos(resp: &api::ParseResponse) -> Vec<String> {
    resp.problems
        .iter()
        .map(|p| {
            format!(
                "{}@L{}",
                p["code"].as_str().unwrap_or("?"),
                p["line"].as_u64().unwrap_or(0)
            )
        })
        .collect()
}

#[test]
fn la_especificacion_de_compiscript_se_analiza_sin_un_solo_diagnostico() {
    let source = read("workspace/rubrica.cps");
    let resp = analizar(&source, "rubrica.cps");

    assert!(resp.accepted, "debe ser sintácticamente válido: {:?}", resp.error);
    assert!(
        resp.problems.is_empty(),
        "la especificación no debe producir ningún diagnóstico, salieron: {:?}",
        codigos(&resp)
    );
    assert!(!resp.types.is_empty(), "debe haber anotaciones de tipo");
}

// ---------------------------------------------------------------------------
// Arreglo A: el constructor sube por la cadena de herencia.
// ---------------------------------------------------------------------------

#[test]
fn una_subclase_sin_constructor_propio_hereda_el_del_padre() {
    // El ejemplo textual del documento: `class Perro : Animal` no declara
    // constructor, así que `new Perro("Toby")` tiene que resolverlo en Animal.
    let source = "\
class Animal {
  let n: string;
  function constructor(x: string) {
    this.n = x;
  }
}
class Perro : Animal {
  function ladrar(): string {
    return this.n;
  }
}
let p: Perro = new Perro(\"Toby\");
print(p.ladrar());
";
    let resp = analizar(source, "herencia_ctor.cps");
    assert!(resp.accepted, "debe parsear: {:?}", resp.error);
    assert!(
        resp.problems.is_empty(),
        "el constructor heredado debe aceptarse: {:?}",
        codigos(&resp)
    );
}

#[test]
fn la_aridad_del_constructor_heredado_se_sigue_validando() {
    // Heredar la firma no significa dejar de comprobarla: sin argumentos,
    // cuando el padre pide uno, sigue siendo S011.
    let source = "\
class Animal {
  let n: string;
  function constructor(x: string) {
    this.n = x;
  }
}
class Perro : Animal {
  function ladrar(): string {
    return this.n;
  }
}
let p: Perro = new Perro();
print(p.ladrar());
";
    let resp = analizar(source, "herencia_ctor_aridad.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S011")).count(),
        1,
        "esperaba una aridad incorrecta: {codes:?}"
    );
}

#[test]
fn el_constructor_propio_gana_sobre_el_heredado() {
    // `Perro` declara el suyo con DOS parámetros; el del padre pide uno. La
    // llamada con dos debe aceptarse, o sea que se valida contra el propio.
    let source = "\
class Animal {
  let n: string;
  function constructor(x: string) {
    this.n = x;
  }
}
class Perro : Animal {
  let raza: string;
  function constructor(x: string, r: string) {
    this.n = x;
    this.raza = r;
  }
}
let p: Perro = new Perro(\"Toby\", \"pastor\");
print(p.raza);
";
    let resp = analizar(source, "ctor_propio.cps");
    assert!(
        resp.problems.is_empty(),
        "el constructor propio debe tener precedencia: {:?}",
        codigos(&resp)
    );
}

// ---------------------------------------------------------------------------
// Arreglo B: `string + string` concatena.
// ---------------------------------------------------------------------------

#[test]
fn el_mas_concatena_dos_textos_y_el_resultado_es_texto() {
    let source = "\
let saludo: string = \"Hola \" + \"Mundo\";
let otro: string = saludo + \"!\";
print(otro);
";
    let resp = analizar(source, "concat.cps");
    assert!(
        resp.problems.is_empty(),
        "concatenar dos textos es válido: {:?}",
        codigos(&resp)
    );

    // Y el resultado se anota como string, no como algo indefinido: es lo que
    // va a necesitar la generación de código intermedio para emitir la
    // concatenación en vez de una suma.
    let hay_string = resp
        .types
        .iter()
        .any(|t| t["ty"].as_str() == Some("string") && t["line"].as_u64() == Some(1));
    assert!(hay_string, "la concatenación debe anotarse como string: {:#?}", resp.types);
}

#[test]
fn los_otros_operadores_siguen_rechazando_texto() {
    // El caso que justifica que la concatenación no sea una fila más de la
    // matriz aritmética, que comparten los cuatro operadores.
    let source = "let mal: string = \"a\" - \"b\";\n";
    let resp = analizar(source, "resta_texto.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S015")).count(),
        1,
        "`\"a\" - \"b\"` debe seguir siendo inválido: {codes:?}"
    );
}

#[test]
fn mezclar_texto_y_numero_sigue_siendo_invalido() {
    let source = "\
let n: integer = 1;
let t: string = \"x\";
print(n + t);
";
    let resp = analizar(source, "mixto.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S015")).count(),
        1,
        "`integer + string` debe seguir reportando S015: {codes:?}"
    );
}
