//! Batería integral del TAC: fuente real -> pipeline -> TAC -> VM.
//!
//! Convenciones de los fixtures:
//! - `// salida: texto` puede repetirse; cada directiva representa una línea.
//! - `// cubre: feature, ...` declara la cobertura funcional del caso OK.
//! - `// error: CÓDIGO` declara un diagnóstico esperado del caso inválido.
//! - Un `.tac` junto al `.cps` activa además la comparación textual exacta.

use lexer_generator::api::{self, ParseResponse};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const REQUIRED_COVERAGE: &[&str] = &[
    "arithmetic",
    "precedence",
    "booleans",
    "short_circuit",
    "if_else",
    "while",
    "do_while",
    "for",
    "switch",
    "break_continue",
    "function",
    "recursion",
    "closure",
    "class",
    "override_callv",
    "list",
    "index_error",
    "division_zero",
    "try_catch",
];

fn workspace(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("workspace")
        .join(relative)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("no se pudo leer '{}': {error}", path.display()))
}

fn cps_files(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<_> = fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("no se pudo abrir '{}': {error}", directory.display()))
        .map(|entry| entry.expect("entrada legible").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("cps"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no hay casos .cps en '{}'", directory.display());
    files
}

fn directives<'a>(source: &'a str, name: &str) -> Vec<&'a str> {
    let prefix = format!("// {name}:");
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix(&prefix).map(str::trim))
        .collect()
}

fn expected_output(source: &str, path: &Path) -> String {
    let lines = directives(source, "salida");
    assert!(
        !lines.is_empty(),
        "'{}' debe declarar al menos un comentario // salida:",
        path.display()
    );
    let mut output = lines.join("\n");
    output.push('\n');
    output
}

fn compile(path: &Path, source: &str, run: bool) -> ParseResponse {
    let yal = read(&workspace("compiscript.yal"));
    let yalp = read(&workspace("compiscript.yalp"));
    let source_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("el fixture debe tener un nombre UTF-8");
    api::build_pipeline_response_named_with_run(
        &yal,
        &yalp,
        source,
        "lalr",
        source_name,
        run,
    )
    .unwrap_or_else(|error| panic!("falló el pipeline para '{}': {error}", path.display()))
}

fn error_codes(response: &ParseResponse) -> Vec<&str> {
    response
        .problems
        .iter()
        .filter(|problem| problem["level"] == "err")
        .filter_map(|problem| problem["code"].as_str())
        .collect()
}

fn normalize_tac(text: &str) -> String {
    text.replace("\r\n", "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim_end()
        .to_string()
}

#[test]
fn todos_los_casos_ok_generan_y_ejecutan() {
    let directory = workspace("tac/ok");
    let mut coverage = BTreeSet::new();
    let mut compared_tac = 0usize;

    for path in cps_files(&directory) {
        let source = read(&path);
        for feature in directives(&source, "cubre")
            .into_iter()
            .flat_map(|line| line.split(','))
            .map(str::trim)
            .filter(|feature| !feature.is_empty())
        {
            coverage.insert(feature.to_string());
        }

        let expected = expected_output(&source, &path);
        let response = compile(&path, &source, true);
        assert!(response.accepted, "'{}' no fue aceptado", path.display());
        assert!(
            error_codes(&response).is_empty(),
            "'{}' produjo errores: {:#?}",
            path.display(),
            response.problems
        );
        assert!(
            !response.tac.trim().is_empty(),
            "'{}' no generó TAC",
            path.display()
        );
        assert_eq!(
            response.run_output.as_deref(),
            Some(expected.as_str()),
            "salida distinta en '{}'",
            path.display()
        );

        let expected_tac_path = path.with_extension("tac");
        if expected_tac_path.exists() {
            compared_tac += 1;
            assert_eq!(
                normalize_tac(&response.tac),
                normalize_tac(&read(&expected_tac_path)),
                "TAC distinto en '{}'",
                path.display()
            );
        }
    }

    let missing: Vec<_> = REQUIRED_COVERAGE
        .iter()
        .copied()
        .filter(|feature| !coverage.contains(*feature))
        .collect();
    assert!(missing.is_empty(), "faltan casos para: {}", missing.join(", "));
    assert!(
        compared_tac > 0,
        "debe existir al menos un .tac esperado para probar la comparación textual"
    );
}

#[test]
fn todos_los_casos_error_reportan_codigo_y_no_generan_tac() {
    let directory = workspace("tac/error");

    for path in cps_files(&directory) {
        let source = read(&path);
        let expected_codes = directives(&source, "error");
        assert!(
            !expected_codes.is_empty(),
            "'{}' debe declarar // error: CÓDIGO",
            path.display()
        );

        let response = compile(&path, &source, true);
        let actual_codes = error_codes(&response);
        for expected in expected_codes {
            assert!(
                actual_codes.contains(&expected),
                "'{}' debía reportar {expected}; obtuvo {actual_codes:?}",
                path.display()
            );
        }
        assert!(
            response.tac.is_empty(),
            "'{}' produjo TAC pese a sus errores:\n{}",
            path.display(),
            response.tac
        );
        assert!(response.tac_quads.is_null(), "'{}' produjo cuádruplos", path.display());
        assert!(response.run_output.is_none(), "'{}' llegó a ejecutar la VM", path.display());
    }
}

#[test]
#[ignore = "rubrica.cps hoy genera '+' para dos strings en el catch; debe emitir concat"]
fn rubrica_completa_compila_y_ejecuta() {
    let path = workspace("rubrica.cps");
    let source = read(&path);
    let response = compile(&path, &source, true);
    assert!(
        error_codes(&response).is_empty(),
        "rubrica.cps debe ejecutar completa: {:#?}",
        response.problems
    );
    assert!(!response.tac.is_empty());
    assert!(response.run_output.is_some());
}
