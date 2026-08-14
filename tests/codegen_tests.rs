// Tests para /api/codegen (api::build_codegen_response) — cobertura nueva:
// antes de esto, codegen::rust_codegen no tenía ningún test en el repo.
//
// Cubre las dos regresiones que motivaron el trabajo: (1) la extracción de
// kinds vieja en rust_codegen solo entendía `Token::X` y devolvía "Unknown"
// para el estilo `{ "X" }` que usan la mayoría de los .yal de ejemplo, y
// (2) el lexer generado no sintetizaba INDENT/DEDENT para gramáticas
// Python-style.
use lexer_generator::api;
use std::fs;

#[test]
fn codegen_for_python_hard_has_no_unknown_kinds_and_supports_indentation() {
    let yal = fs::read_to_string("examples/lexer/python_hard.yal")
        .expect("fixture examples/lexer/python_hard.yal debe existir");
    let yalp = fs::read_to_string("examples/grammar/python_hard.yalp")
        .expect("fixture examples/grammar/python_hard.yalp debe existir");

    let resp = api::build_codegen_response(&yal, &yalp)
        .expect("codegen no debería fallar para python_hard");

    assert!(resp.indent_sensitive, "python_hard declara NEWLINE+INDENT+DEDENT");
    assert!(
        !resp.code.contains("\"UNKNOWN\""),
        "ningún kind debería salir Unknown — regresión directa del bug de extracción \
         (rust_codegen antes solo entendía `Token::X`, y python_hard.yal usa `{{ \"X\" }}`)"
    );
    assert!(
        resp.code.contains("pub fn synthesize_indentation"),
        "gramática indent-sensitive debe emitir el post-procesamiento de indentación"
    );
    assert!(
        resp.code.contains("pub fn tokenize_for_parser"),
        "debe emitir el punto de entrada que filtra ignorables y sintetiza indentación"
    );
    assert!(resp.problems.is_empty(), "no debería haber problemas: {:?}", resp.problems);
    assert!(resp.n_states > 0);
}

#[test]
fn codegen_for_non_indent_sensitive_grammar_omits_indentation_support() {
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n  | '+' { return PLUS }\n  | ' ' { skip }\n";
    let yalp = "%token NUM PLUS\n%%\nS : NUM PLUS NUM ;\n";

    let resp = api::build_codegen_response(yal, yalp)
        .expect("codegen no debería fallar para una gramática simple");

    assert!(!resp.indent_sensitive);
    assert!(
        !resp.code.contains("synthesize_indentation"),
        "una gramática sin NEWLINE+INDENT+DEDENT no debe traer el post-procesamiento"
    );
    assert!(!resp.code.contains("\"UNKNOWN\""));
}

#[test]
fn codegen_works_without_a_yalp_at_all() {
    // El .yalp es opcional — sin gramática, se genera igual (sin filtrado de
    // ignorables ni indentación, que dependen de lo que declara el .yalp).
    let yal = "let digit = ['0'-'9']\nrule tokens =\n  | digit+ { return NUM }\n";

    let resp = api::build_codegen_response(yal, "")
        .expect("codegen debe funcionar sin .yalp");

    assert!(!resp.indent_sensitive);
    assert!(resp.code.contains("\"NUM\""));
    assert!(resp.problems.is_empty());
}

/// La prueba real de que el lexer generado sirve: lo escribe a disco, lo
/// COMPILA con rustc, lo CORRE sobre hardtest.txt, y compara su stream de
/// kinds contra el que produce el intérprete (`api::build_pipeline_response`)
/// para los mismos archivos — incluyendo los INDENT/DEDENT sintetizados. Si
/// coinciden, el lexer generado es equivalente al intérprete para este caso,
/// indentación incluida. Se salta (no falla) si `rustc` no está disponible.
#[test]
fn generated_lexer_compiles_and_matches_the_interpreter_token_stream() {
    if std::process::Command::new("rustc").arg("--version").output().is_err() {
        eprintln!("rustc no disponible en este entorno — test saltado.");
        return;
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let yal = fs::read_to_string(format!("{manifest_dir}/examples/lexer/python_hard.yal")).unwrap();
    let yalp = fs::read_to_string(format!("{manifest_dir}/examples/grammar/python_hard.yalp")).unwrap();
    let source_path = format!("{manifest_dir}/examples/source/hardtest.txt");
    let source = fs::read_to_string(&source_path).unwrap();

    // 1) Stream de kinds esperado, vía el intérprete (mismo camino que /api/pipeline).
    let pipeline = api::build_pipeline_response(&yal, &yalp, &source, "lalr")
        .expect("el intérprete no debería fallar sobre hardtest.txt");
    assert!(pipeline.accepted, "hardtest.txt debe ser aceptado por el intérprete: {:?}", pipeline.error);
    let expected_kinds: Vec<String> = pipeline
        .token_map
        .iter()
        .map(|t| t["kind"].as_str().unwrap().to_string())
        .collect();
    assert!(!expected_kinds.is_empty());

    // 2) Generar, escribir a disco y compilar el lexer standalone.
    let gen = api::build_codegen_response(&yal, &yalp).expect("codegen no debería fallar");
    assert!(gen.indent_sensitive);

    let work_dir = std::env::temp_dir().join(format!("codegen_verify_{}", std::process::id()));
    fs::create_dir_all(&work_dir).unwrap();
    fs::write(work_dir.join("lexer.rs"), &gen.code).unwrap();
    fs::write(
        work_dir.join("main.rs"),
        r#"mod lexer;
fn main() {
    let path = std::env::args().nth(1).expect("uso: main <archivo>");
    let src = std::fs::read_to_string(&path).expect("no se pudo leer el archivo fuente");
    let (tokens, errors) = lexer::tokenize_for_parser(&src).expect("fallo al sintetizar indentación");
    for t in &tokens { println!("{}", t.kind); }
    for e in &errors { eprintln!("LEXERR: {}", e); }
}
"#,
    ).unwrap();

    let binary_path = work_dir.join("codegen_verify_bin");
    let compile = std::process::Command::new("rustc")
        .arg("--edition").arg("2021")
        .arg("-o").arg(&binary_path)
        .arg(work_dir.join("main.rs"))
        .output()
        .expect("no se pudo invocar rustc");
    assert!(
        compile.status.success(),
        "el lexer generado no compiló:\n{}",
        String::from_utf8_lossy(&compile.stderr)
    );

    // 3) Correrlo sobre el mismo hardtest.txt y capturar su stream de kinds.
    let run = std::process::Command::new(&binary_path)
        .arg(&source_path)
        .output()
        .expect("no se pudo ejecutar el binario generado");
    assert!(
        run.status.success(),
        "el binario generado terminó con error:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&run.stdout),
        String::from_utf8_lossy(&run.stderr)
    );
    let actual_kinds: Vec<String> = String::from_utf8_lossy(&run.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();

    assert_eq!(
        actual_kinds, expected_kinds,
        "el stream de kinds del lexer generado debe ser idéntico al del intérprete \
         (incluyendo INDENT/DEDENT sintetizados)"
    );

    let _ = fs::remove_dir_all(&work_dir);
}
