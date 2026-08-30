// Fase de `semantico::operators`: validación de expresiones binarias y
// unarias sobre gramáticas REALES (compiladas, lexeadas y parseadas de
// verdad), no árboles armados a mano.
//
// Cubre las tres familias —lógicas (`&& || !`), comparaciones
// (`== != < <= > >=`) y unarias (`!`, `-`)— más la regla de sentido semántico
// ("no multiplicar funciones"), y comprueba que ninguna dispara sobre código
// correcto.
use lexer_generator::api;
use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("no se pudo leer {path}: {e}"))
}

/// Corre el pipeline completo y devuelve `(código, línea, columna)` de cada
/// diagnóstico, en orden.
fn diagnose(source: &str, name: &str) -> Vec<(String, u64, u64)> {
    let yal = read("workspace/compiscript.yal");
    let yalp = read("workspace/compiscript.yalp");

    let resp = api::build_pipeline_response_named(&yal, &yalp, source, "lalr", name)
        .expect("el pipeline no debe fallar internamente");
    assert!(
        resp.accepted,
        "{name} debe ser sintácticamente válido — los errores buscados son semánticos: {:?} / {:#?}",
        resp.error, resp.problems
    );

    resp.problems
        .iter()
        .map(|p| {
            (
                p["code"].as_str().unwrap_or_default().to_string(),
                p["line"].as_u64().unwrap_or_default(),
                p["col"].as_u64().unwrap_or_default(),
            )
        })
        .collect()
}

fn codes(diags: &[(String, u64, u64)]) -> Vec<&str> {
    diags.iter().map(|(c, _, _)| c.as_str()).collect()
}

const VALIDO: &str = r#"
function doble(n: integer): integer { return n + n; }

let a: integer = 3;
let b: integer = 4;
let si: boolean = true;
let no: boolean = false;
let texto: string = "hola";

let orden: boolean = a < b;
let igualdad: boolean = a == b;
let textos: boolean = texto == "hola";
let banderas: boolean = si == no;
let conjuncion: boolean = (a > 0) && !(a == b);
let disyuncion: boolean = si || no;
let negado: integer = -a;
let resta: integer = a - b;
let llamada: integer = doble(2) * 2;
"#;

#[test]
fn correct_expressions_produce_no_operator_diagnostics() {
    let diags = diagnose(VALIDO, "operadores_ok.cps");
    assert!(
        diags.is_empty(),
        "el código correcto no debe generar ningún diagnóstico: {diags:#?}"
    );
}

#[test]
fn a_unary_minus_and_a_binary_minus_share_a_token_without_colliding() {
    // MINUS está declarado a la vez en `%arith MINUS subtract` y en
    // `%unary MINUS negate`. Las dos formas se distinguen por la FORMA del
    // nodo (tres hijos con el operador en el medio vs. dos con el operador
    // adelante), así que compartir el token no genera diagnósticos cruzados.
    let diags = diagnose("let a: integer = 5;\nlet b: integer = -a - 2;\n", "minus.cps");
    assert!(diags.is_empty(), "{diags:#?}");

    // Y el unario sigue validando: negar un string es S030 aunque MINUS
    // también sea un operador aritmético válido.
    let diags = diagnose("let s: string = \"x\";\nlet n: integer = -s;\n", "minus_malo.cps");
    assert_eq!(codes(&diags), vec!["S030"], "{diags:#?}");
}

#[test]
fn logical_operators_reject_non_boolean_operands() {
    let source = "\
let n: integer = 1;
let si: boolean = true;
let malo: boolean = n && si;
let peor: boolean = !n;
";
    let diags = diagnose(source, "logicos.cps");
    assert_eq!(codes(&diags), vec!["S028", "S028"], "{diags:#?}");

    // Posiciones reales: el `&&` de la línea 3 y el `!` de la línea 4.
    assert_eq!(diags[0].1, 3, "el && está en la línea 3: {diags:#?}");
    assert_eq!(diags[1].1, 4, "el ! está en la línea 4: {diags:#?}");
    for (_, line, col) in &diags {
        assert!(*line > 0 && *col > 0, "línea/columna reales: {diags:#?}");
    }
}

#[test]
fn comparisons_reject_incompatible_operands() {
    let source = "\
let n: integer = 1;
let s: string = \"x\";
let si: boolean = true;
let a: boolean = si == s;
let b: boolean = s < n;
let c: boolean = si > si;
";
    let diags = diagnose(source, "comparaciones.cps");
    assert_eq!(
        codes(&diags),
        vec!["S029", "S029", "S029"],
        "igualdad entre bool y string, y dos órdenes no numéricos: {diags:#?}"
    );
    assert_eq!(diags[0].1, 4);
    assert_eq!(diags[1].1, 5);
    assert_eq!(diags[2].1, 6);
}

#[test]
fn naming_a_function_or_a_class_is_not_a_value() {
    // El caso "no multiplicar funciones" del enunciado. Sin esta regla pasa
    // desapercibido: el tipo de la hoja `doble` es el tipo de RETORNO de
    // `doble`, así que `doble * 2` se ve idéntico a multiplicar un entero.
    let source = "\
function doble(n: integer): integer { return n + n; }
class Caja { var v: integer = 0; }
let malo: integer = doble * 2;
let peor: boolean = Caja == Caja;
let bien: integer = doble(2) * 2;
";
    let diags = diagnose(source, "no_valores.cps");
    let solo_s031: Vec<&str> = codes(&diags);
    assert_eq!(
        solo_s031,
        vec!["S031", "S031", "S031"],
        "la función una vez y la clase dos (un operando cada lado): {diags:#?}"
    );
    assert_eq!(diags[0].1, 3, "la función mal usada está en la línea 3");
    assert_eq!(diags[1].1, 4);
    assert_eq!(diags[2].1, 4);
    // La línea 5 usa `doble(2)`, una LLAMADA: es un valor y no debe aparecer.
    assert!(
        diags.iter().all(|(_, line, _)| *line != 5),
        "una llamada sí es un valor: {diags:#?}"
    );
}

#[test]
fn typing_comparisons_makes_flow_conditions_actually_check() {
    // Mejora colateral: antes `resolve_expr_type` devolvía `None` para toda
    // comparación, así que `%condition` sobre un `while`/`if` nunca veía el
    // tipo de su condición y se rendía. Ahora una comparación tipa `bool` y
    // la condición se valida de verdad — sin que eso genere falsos positivos
    // sobre una comparación correcta.
    let valido = "let x: integer = 5;\nwhile (x > 0) { x = x - 1; }\nif (x == 0) { print(x); }\n";
    assert!(diagnose(valido, "cond_ok.cps").is_empty());

    // Y la condición no booleana sigue siendo S025, no un S029 derivado.
    let invalido = "let x: integer = 5;\nwhile (x) { x = x - 1; }\n";
    assert_eq!(codes(&diagnose(invalido, "cond_mala.cps")), vec!["S025"]);
}

#[test]
fn compare_directives_work_on_a_grammar_with_different_names() {
    // pascalito.yalp declara `%compare LT lt` / `%compare GT gt` sobre una
    // producción llamada `comparacion` (no `relational_expr`) y con tokens
    // propios: la validación no está atada a los nombres de Compiscript.
    let yal = read("examples/lexer/pascalito.yal");
    let yalp = read("examples/grammar/pascalito.yalp");
    let source = read("examples/source/pascalito.txt");

    let resp = api::build_pipeline_response_named(&yal, &yalp, &source, "lalr", "pascalito.txt")
        .expect("el pipeline no debe fallar internamente");
    assert!(resp.accepted, "{:?}", resp.error);

    // Las comparaciones de esa fuente son `entera > 0`: enteras y válidas, así
    // que la directiva no debe inventar ningún S029.
    let s029 = resp
        .problems
        .iter()
        .filter(|p| p["code"] == "S029")
        .count();
    assert_eq!(s029, 0, "comparaciones válidas: {:#?}", resp.problems);
}
