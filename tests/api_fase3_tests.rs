use lexer_generator::api;

fn compiscript(source: &str, run: bool) -> api::ParseResponse {
    let yal = std::fs::read_to_string("workspace/compiscript.yal").expect("lexer");
    let yalp = std::fs::read_to_string("workspace/compiscript.yalp").expect("gramática");
    api::build_pipeline_response_named_with_run(
        &yal,
        &yalp,
        source,
        "lalr",
        "fase3.cps",
        run,
    )
    .expect("el pipeline debe responder sin panic")
}

#[test]
fn respuesta_expone_tac_cuadruplos_simbolos_y_temporales_sin_ejecutar() {
    let response = compiscript("let x: integer = 2 + 3; print(x);", false);

    assert!(response.problems.is_empty(), "{:#?}", response.problems);
    assert!(response.tac.contains("func main"));
    assert!(response.tac.contains("print x"));
    assert!(response.tac_quads["main"]["quads"].is_array());
    assert!(response.symbols_ext["symbols"].is_array());
    assert!(response.symbols_ext["frames"].is_array());
    assert_eq!(response.temps[0]["function"], "main");
    assert!(response.temps[0]["requested"].is_u64());
    assert!(response.temps[0]["max_live"].is_u64());
    assert!(response.run_output.is_none());

    let serialized = serde_json::to_value(&response).unwrap();
    assert!(
        serialized.get("run_output").is_none(),
        "run_output se omite para peticiones antiguas/run=false"
    );
}

#[test]
fn run_true_ejecuta_la_vm_y_devuelve_su_salida() {
    let response = compiscript("print(2 + 3);", true);

    assert!(response.problems.is_empty(), "{:#?}", response.problems);
    assert_eq!(response.run_output.as_deref(), Some("5\n"));
    let serialized = serde_json::to_value(&response).unwrap();
    assert_eq!(serialized["run_output"], "5\n");
}

#[test]
fn un_error_semantico_impide_generar_y_ejecutar_tac() {
    let response = compiscript("print(noExiste);", true);

    assert!(response.problems.iter().any(|problem| problem["code"] == "S002"));
    assert!(response.tac.is_empty());
    assert!(response.tac_quads.is_null());
    assert!(response.symbols_ext.is_null());
    assert!(response.temps.is_empty());
    assert!(response.run_output.is_none());
}

#[test]
fn un_runtime_error_se_reporta_como_r001_sin_panic() {
    let response = compiscript("print(1 / 0);", true);

    assert!(!response.tac.is_empty(), "el TAC válido sí debe conservarse");
    let runtime = response
        .problems
        .iter()
        .find(|problem| problem["code"] == "R001")
        .expect("el error de VM debe distinguirse de los errores de compilación");
    assert!(runtime["msg"].as_str().unwrap().contains("cero"));
    assert_eq!(response.run_output.as_deref(), Some(""));
}

#[test]
fn un_fallo_de_generacion_agrega_su_codigo_c_y_no_ejecuta() {
    let yal = "let letter = ['a'-'z']\nrule tokens =\n  | letter+ { return ID }\n  | ';' { return SEMI }\n  | ' ' { skip }";
    // La declaración queda deliberadamente sin tipo: semánticamente existe,
    // pero storage no puede asignarle tamaño y el generador devuelve C002.
    let yalp = "%token ID SEMI\n%ident ID\n%declare decl variable\n%%\nprograma : decl SEMI ;\ndecl : ID ;\n";
    let response = api::build_pipeline_response_named_with_run(
        yal,
        yalp,
        "x;",
        "lalr",
        "incompleto.txt",
        true,
    )
    .expect("un CodegenError forma parte de la respuesta");

    assert!(response.problems.iter().any(|problem| problem["code"] == "C002"));
    assert!(response.tac.is_empty());
    assert!(response.run_output.is_none());
}
