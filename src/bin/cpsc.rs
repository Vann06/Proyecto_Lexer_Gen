use lexer_generator::api::build_pipeline_response_named_with_run;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const HELP: &str = "Compilador de CompiScript\n\nUSO:\n    cpsc <programa.cps> [--run] [--slr]\n\nOPCIONES:\n    --run       Ejecuta el TAC y muestra la salida del programa\n    --slr       Usa el parser SLR (por defecto se usa LALR)\n    -h, --help  Muestra esta ayuda";

#[derive(Debug, PartialEq, Eq)]
struct Options {
    source: PathBuf,
    run: bool,
    mode: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Help,
    Compile(Options),
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut source = None;
    let mut run = false;
    let mut mode = "lalr";

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "--run" => run = true,
            "--slr" => mode = "slr",
            _ if arg.starts_with('-') => return Err(format!("opción desconocida: {arg}")),
            _ if source.is_some() => {
                return Err(format!("solo se admite un archivo fuente; sobra: {arg}"));
            }
            _ => source = Some(PathBuf::from(arg)),
        }
    }

    let source = source.ok_or_else(|| "falta el archivo .cps".to_string())?;
    Ok(Command::Compile(Options { source, run, mode }))
}

fn read_text(path: &Path, description: &str) -> Result<String, String> {
    fs::read_to_string(path)
        .map_err(|error| format!("no se pudo leer {description} '{}': {error}", path.display()))
}

fn problem_field<'a>(problem: &'a Value, field: &str) -> &'a str {
    problem
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or("?")
}

fn print_problem(problem: &Value) {
    let level = problem_field(problem, "level");
    let code = problem_field(problem, "code");
    let location = problem_field(problem, "loc");
    let message = problem_field(problem, "msg");
    eprintln!("[{level} {code}] {location}: {message}");
}

fn print_section(title: &str, body: &str) {
    println!("{title}");
    println!("{}", "=".repeat(title.chars().count()));
    if body.is_empty() {
        println!();
    } else if body.ends_with('\n') {
        print!("{body}");
    } else {
        println!("{body}");
    }
}

fn compile(options: Options) -> Result<(), String> {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = project_root.join("workspace");
    let yal_path = workspace.join("compiscript.yal");
    let yalp_path = workspace.join("compiscript.yalp");

    let yal = read_text(&yal_path, "la gramática léxica")?;
    let yalp = read_text(&yalp_path, "la gramática sintáctica")?;
    let source = read_text(&options.source, "el programa")?;
    let source_name = options
        .source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("programa.cps");

    let response = build_pipeline_response_named_with_run(
        &yal,
        &yalp,
        &source,
        options.mode,
        source_name,
        options.run,
    )
    .map_err(|error| format!("falló el pipeline de compilación: {error}"))?;

    for problem in &response.problems {
        print_problem(problem);
    }

    let has_errors = response
        .problems
        .iter()
        .any(|problem| problem_field(problem, "level") == "err");
    if has_errors || !response.accepted {
        return Err("la compilación terminó con errores".to_string());
    }
    if response.tac.trim().is_empty() {
        return Err("la compilación no produjo TAC".to_string());
    }

    let symbols = serde_json::to_string_pretty(&response.symbols_ext)
        .map_err(|error| format!("no se pudo presentar la tabla de símbolos: {error}"))?;

    print_section("TAC", &response.tac);
    println!();
    print_section("TABLA DE SÍMBOLOS EXTENDIDA", &symbols);

    if options.run {
        let output = response
            .run_output
            .as_deref()
            .ok_or_else(|| "la VM no devolvió una salida".to_string())?;
        println!();
        print_section("SALIDA", output);
    }

    Ok(())
}

fn main() -> ExitCode {
    match parse_args(env::args().skip(1)) {
        Ok(Command::Help) => {
            println!("{HELP}");
            ExitCode::SUCCESS
        }
        Ok(Command::Compile(options)) => match compile(options) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("cpsc: {error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("cpsc: {error}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn defaults_to_lalr_without_execution() {
        assert_eq!(
            parse_args(args(&["programa.cps"])),
            Ok(Command::Compile(Options {
                source: PathBuf::from("programa.cps"),
                run: false,
                mode: "lalr",
            }))
        );
    }

    #[test]
    fn accepts_run_and_slr_in_any_position() {
        assert_eq!(
            parse_args(args(&["--run", "programa.cps", "--slr"])),
            Ok(Command::Compile(Options {
                source: PathBuf::from("programa.cps"),
                run: true,
                mode: "slr",
            }))
        );
    }

    #[test]
    fn recognizes_help() {
        assert_eq!(parse_args(args(&["--help"])), Ok(Command::Help));
    }

    #[test]
    fn rejects_missing_source() {
        assert_eq!(parse_args(args(&[])), Err("falta el archivo .cps".into()));
    }

    #[test]
    fn rejects_unknown_option_and_extra_source() {
        assert!(parse_args(args(&["programa.cps", "--wat"])).is_err());
        assert!(parse_args(args(&["uno.cps", "dos.cps"])).is_err());
    }
}
