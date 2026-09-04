//! Código muerto sobre el pipeline real: instrucciones inalcanzables tras
//! `return`/`break`/`continue`, y el corte de evaluación del bloque.
//!
//! Los programas viven en `workspace/` para que sean cargables tal cual desde
//! el IDE: la batería que corre el usuario y la que corre `cargo test` son el
//! mismo archivo. La lógica pura (aplanado de la secuencia, qué cuenta como
//! sentencia terminal) está cubierta aparte en `semantico::deadcode::tests`.
use lexer_generator::api;

const DEAD_CODE: &str = include_str!("../workspace/codigo_muerto.cps");
const DEAD_CODE_CUT: &str = include_str!("../workspace/codigo_muerto_corte.cps");

fn analizar(source: &str, name: &str) -> lexer_generator::api::ParseResponse {
    let yal = include_str!("../workspace/compiscript.yal");
    let yalp = include_str!("../workspace/compiscript.yalp");
    api::build_pipeline_response_named(yal, yalp, source, "lalr", name)
        .expect("el pipeline no debe fallar")
}

fn codigos(response: &lexer_generator::api::ParseResponse) -> Vec<&str> {
    response
        .problems
        .iter()
        .filter_map(|problem| problem["code"].as_str())
        .collect()
}

#[test]
fn cada_sentencia_terminal_mata_lo_que_le_sigue() {
    let response = analizar(DEAD_CODE, "codigo_muerto.cps");
    assert!(
        response.accepted,
        "el código muerto es válido sintácticamente: {:?}",
        response.error
    );

    let codigos = codigos(&response);
    assert_eq!(
        codigos,
        vec!["W002", "W002", "W002", "W002"],
        "uno por bloque: tras return, tras break, tras continue y tras el break de un case: {:#?}",
        response.problems
    );

    for problem in &response.problems {
        assert_eq!(problem["level"], "warn", "el código muerto es advertencia, no error");
        assert!(problem["loc"].as_str().unwrap().starts_with("codigo_muerto.cps:"));
        assert!(problem["line"].as_u64().unwrap() > 0);
        assert!(problem["col"].as_u64().unwrap() > 0);
    }
}

#[test]
fn lo_inalcanzable_no_genera_diagnosticos_derivados() {
    let response = analizar(DEAD_CODE_CUT, "codigo_muerto_corte.cps");
    assert!(response.accepted, "{:?}", response.error);

    // Las dos líneas muertas usan cuatro nombres que no existen. Sin el corte
    // de evaluación aparecerían como S002 y el panel de problemas se llenaría
    // de errores sobre código que nunca se ejecuta.
    assert_eq!(
        codigos(&response),
        vec!["W002"],
        "solo el aviso de código muerto, ni un S002 derivado: {:#?}",
        response.problems
    );
}
