//! Fase 5 de la generación de MIPS (ver `PLAN_MIPS.md`): lo que `/api/pipeline`
//! entrega al IDE, y la ejecución en MARS con `run_mips`.
#[allow(dead_code)]
mod common;

use lexer_generator::api::{self, ParseResponse, PipelineOptions};

fn compiscript(source: &str, options: PipelineOptions) -> ParseResponse {
    api::build_pipeline_response_with_options(
        &common::read("workspace/compiscript.yal"),
        &common::read("workspace/compiscript.yalp"),
        source,
        "lalr",
        "mips.cps",
        options,
    )
    .expect("el pipeline debe responder")
}

fn con_mars(source: &str) -> Option<ParseResponse> {
    common::hay_mars().then(|| compiscript(source, PipelineOptions { run: true, run_mips: true, ..Default::default() }))
}

#[test]
fn un_programa_valido_trae_mips_y_estadisticas_sin_ejecutar() {
    let response = compiscript("function doble(x: integer): integer { return x * 2; }\nprint(doble(4));", Default::default());

    assert!(response.problems.is_empty(), "{:#?}", response.problems);
    assert!(response.mips.contains("main:"), "{}", response.mips);
    let funciones: Vec<_> = response.mips_stats.as_array().unwrap().iter().map(|s| s["funcion"].as_str().unwrap()).collect();
    assert_eq!(funciones, ["main", "_doble"]);

    let json = serde_json::to_value(&response).unwrap();
    assert!(json.get("mips_output").is_none() && json.get("mips_exit_code").is_none(), "sin run_mips no se ejecuta");
}

#[test]
fn con_errores_no_hay_mips() {
    let response = compiscript("print(noExiste);", PipelineOptions { run_mips: true, ..Default::default() });

    assert!(response.problems.iter().any(|problem| problem["code"] == "S002"));
    assert!(response.mips.is_empty() && response.mips_stats.is_null());
    assert!(response.mips_output.is_none() && response.mips_exit_code.is_none());
}

#[test]
fn run_mips_imprime_lo_mismo_que_el_interprete() {
    if let Some(response) = con_mars("let x: integer = 2 + 3; print(x); print(\"listo\");") {
        assert!(response.problems.is_empty(), "{:#?}", response.problems);
        assert_eq!(response.mips_output.as_deref(), Some("5\nlisto\n"));
        assert_eq!(response.mips_output, response.run_output);
        assert_eq!(response.mips_exit_code, Some(0));
    }
}

#[test]
fn un_error_sin_try_termina_con_codigo_uno_y_conserva_la_salida() {
    if let Some(response) = con_mars("let cero: integer = 0; print(1); print(1 / cero);") {
        assert_eq!(response.mips_output.as_deref(), Some("1\nError en tiempo de ejecucion: division por cero\n"));
        assert_eq!(response.mips_exit_code, Some(1));
    }
}

fn codigos(response: &ParseResponse) -> Vec<&str> {
    response.problems.iter().map(|problem| problem["code"].as_str().unwrap()).collect()
}

#[test]
fn un_ciclo_infinito_se_reporta_y_conserva_la_salida() {
    if let Some(response) = con_mars("print(1); let x: integer = 0; while (true) { x = x + 1; }") {
        assert_eq!(codigos(&response), ["R001", "M902"], "{:#?}", response.problems);
        assert!(response.problems.iter().all(|p| p["msg"].as_str().unwrap().contains("posible ciclo infinito")), "{:#?}", response.problems);
        assert_eq!(response.mips_output.as_deref(), Some("1\n"));
        assert_eq!(response.mips_exit_code, Some(2));
    }
}

/// Antes tumbaba todo el servidor: la VM agotaba la pila del proceso.
#[test]
fn una_recursion_infinita_se_reporta_sin_tumbar_el_proceso() {
    if let Some(response) = con_mars("function f(n: integer): integer { return f(n + 1) + 1; }\nprint(1); print(f(0));") {
        assert_eq!(codigos(&response), ["R001", "M904"], "{:#?}", response.problems);
        assert_eq!(response.run_output.as_deref(), Some("1\n"));
        assert_eq!(response.mips_output.as_deref(), Some("1\n"));
    }
}

#[test]
fn agotar_la_memoria_se_reporta_y_conserva_la_salida() {
    if let Some(response) = con_mars("print(1); while (true) { let a: integer[] = [1, 2, 3, 4, 5, 6, 7, 8]; }") {
        assert!(codigos(&response).contains(&"M905"), "{:#?}", response.problems);
        assert_eq!(response.mips_output.as_deref(), Some("1\n"));
    }
}
