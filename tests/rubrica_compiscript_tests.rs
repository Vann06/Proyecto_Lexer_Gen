//! La especificación de Compiscript, ejercitada de punta a punta.
//!
//! `workspace/rubrica.cps` recorre las características que el documento del
//! lenguaje declara soportadas, con la sintaxis de sus propios ejemplos. El
//! test exige CERO diagnósticos: es la prueba reproducible de que el proyecto
//! cumple la especificación, y el archivo sirve de demostración.
//!
//! Dos ejemplos del documento se escriben acá con llaves —`if (n < 60) { continue; }`
//! y `if (n2 <= 1) { return 1; }`— porque el cuerpo de todo control de flujo
//! tiene que ser un bloque. Eso NO es una limitación de este proyecto: la
//! gramática oficial exige lo mismo (`Compiscript.g4:51-55` usa `block`, y
//! `block: '{' statement* '}'` en `Compiscript.g4:30`). Los que no siguen la
//! gramática oficial son esos dos ejemplos en prosa del enunciado.
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

// ---------------------------------------------------------------------------
// Arreglo C: las llamadas a funcion y `new` ya tienen tipo.
//
// Antes de esto `resolve_expr_type` no tenia rama para ninguna de las dos, asi
// que toda llamada valia `None`. Como las reglas que dependen del tipo se
// callan ante `None` en vez de adivinar, eso apagaba en cadena cuatro reglas
// del enunciado: asignacion contra el tipo declarado, tipo de retorno,
// condiciones booleanas y acceso a miembros sobre un tipo inferido.
// ---------------------------------------------------------------------------

#[test]
fn el_tipo_de_retorno_de_una_llamada_se_verifica_en_la_asignacion() {
    let source = "function f(): integer {
  return 1;
}
let x: string = f();
print(x);
";
    let resp = analizar(source, "llamada_asignacion.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S006")).count(),
        1,
        "asignar un `integer` devuelto por `f()` a un `string` debe reportarse: {codes:?}"
    );
}

#[test]
fn el_tipo_de_una_instanciacion_se_infiere_sin_anotacion() {
    // Sin anotacion de tipo, `p` se infiere del inicializador. Si `new Punto()`
    // no tipara, `p` quedaria sin tipo y el acceso a miembro callaria.
    let source = "class Punto {
  let x: integer;
  function constructor(a: integer) {
    this.x = a;
  }
}
let p = new Punto(1);
print(p.noExiste);
";
    let resp = analizar(source, "new_inferido.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S010")).count(),
        1,
        "el miembro inexistente debe detectarse sobre el tipo inferido: {codes:?}"
    );
}

#[test]
fn un_return_que_devuelve_una_llamada_se_verifica() {
    let source = "function h(): string {
  return \"x\";
}
function g(): integer {
  return h();
}
print(g());
";
    let resp = analizar(source, "return_llamada.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S016")).count(),
        1,
        "devolver un `string` desde una funcion `integer` debe reportarse: {codes:?}"
    );
}

#[test]
fn una_llamada_como_condicion_debe_ser_booleana() {
    let source = "function f(): integer {
  return 1;
}
if (f()) {
  print(1);
}
";
    let resp = analizar(source, "condicion_llamada.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S025")).count(),
        1,
        "una condicion `integer` debe reportarse: {codes:?}"
    );
}

#[test]
fn una_llamada_como_argumento_se_verifica() {
    let source = "function g(): string {
  return \"x\";
}
function f(a: integer): integer {
  return a;
}
print(f(g()));
";
    let resp = analizar(source, "argumento_llamada.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S014")).count(),
        1,
        "pasar un `string` donde se pide `integer` debe reportarse: {codes:?}"
    );
}

#[test]
fn una_llamada_bien_tipada_no_reporta_nada() {
    // La otra mitad: tipar las llamadas no debe inventar diagnosticos donde
    // los tipos si coinciden.
    let source = "function suma(a: integer, b: integer): integer {
  return a + b;
}
let x: integer = suma(2, 3);
print(x);
";
    let resp = analizar(source, "llamada_ok.cps");
    assert!(
        resp.problems.is_empty(),
        "una llamada bien tipada no debe reportar nada: {:?}",
        codigos(&resp)
    );
}

#[test]
fn instanciar_una_clase_inexistente_no_encadena_un_segundo_error() {
    // Una clase que no existe reporta S007 y NADA MAS: tipar `new NoExiste()`
    // como `NoExiste` haria que la asignacion a `Punto` fallara ademas con un
    // S006 derivado. Es el mismo criterio que hace neutro a un tipo
    // desconocido — un hueco no debe cascadear en diagnosticos falsos.
    let source = "class Punto {
  let x: integer;
}
let p: Punto = new NoExiste();
";
    let resp = analizar(source, "clase_inexistente.cps");
    let codes = codigos(&resp);
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S007")).count(),
        1,
        "la clase desconocida debe reportarse: {codes:?}"
    );
    assert_eq!(
        codes.iter().filter(|c| c.starts_with("S006")).count(),
        0,
        "y NO debe encadenar una incompatibilidad de asignacion: {codes:?}"
    );
}
