//! Ejecuta un `.asm` en MARS y devuelve exactamente lo que imprimió el programa
//! (`PLAN_MIPS.md` §4.1).
//!
//! Al terminar, MARS siempre escribe un salto de línea propio, aunque el programa
//! no imprima nada. No es salida del programa, así que `run_mars` quita ese único
//! salto; los que imprime el programa se conservan. `-Dline.separator` hace que
//! sea `\n` también en Windows, donde Java usaría `\r\n`.
//!
//! Si MARS detiene el programa, sus avisos tampoco son salida del programa: se
//! quitan y el motivo queda en `SimResult::stop`. No se puede saber si un programa
//! terminaría (problema de la parada), así que un ciclo infinito solo se reporta
//! como posible: lo que se sabe es qué límite alcanzó.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

/// Tiempo máximo de una ejecución normal.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Instrucciones que puede ejecutar un programa. Se alcanza antes que
/// `DEFAULT_TIMEOUT`, y a diferencia de él da el mismo resultado en cualquier máquina.
pub const MAX_STEPS: u32 = 10_000_000;

const JAR: &str = "tools/Mars4_5.jar";

/// Los códigos que MARS devuelve por `se2` y `ae3`.
const EXIT_SIMULATION: i32 = 2;
const EXIT_ASSEMBLE: i32 = 3;

/// El resultado de un programa que ensambló. `exit_code` es 0 si terminó bien,
/// 1 si fue un error de Compiscript sin `try` y 2 si no llegó a terminar: en ese
/// caso `stop` dice por qué. `stdout` es solo lo que imprimió el programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimResult {
    pub stdout: String,
    pub exit_code: i32,
    pub stop: Option<SimStop>,
}

/// Por qué MARS no dejó terminar al programa.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimStop {
    #[error("posible ciclo infinito: el programa no terminó en {MAX_STEPS} instrucciones")]
    StepLimit,
    #[error("posible ciclo infinito: el programa no terminó a tiempo")]
    Timeout,
    #[error("pila agotada: posible recursión infinita")]
    StackOverflow,
    #[error("memoria agotada: el programa reservó más memoria de la disponible")]
    OutOfMemory,
    #[error("MARS detuvo el programa: {0}")]
    Fault(String),
}

impl SimStop {
    /// El código con el que se reporta en `problems`.
    pub fn code(&self) -> &'static str {
        match self {
            SimStop::StepLimit => "M902",
            SimStop::Timeout => "M903",
            SimStop::StackOverflow => "M904",
            SimStop::OutOfMemory => "M905",
            SimStop::Fault(_) => "M906",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimError {
    #[error("no se pudo ejecutar java")]
    NoJava,
    #[error("no se encontró {JAR}")]
    NoJar,
    #[error("MARS no pudo ensamblar el programa:\n{0}")]
    Assemble(String),
}

/// El `.jar` de la carpeta actual o, si no está ahí, el del repositorio.
fn jar_path() -> Option<PathBuf> {
    [PathBuf::from(JAR), Path::new(env!("CARGO_MANIFEST_DIR")).join(JAR)].into_iter().find(|path| path.exists())
}

/// Si se puede ejecutar MARS: hay `java` y está el `.jar`.
pub fn mars_available() -> bool {
    let java = Command::new("java").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    java && jar_path().is_some()
}

/// Ensambla y ejecuta `asm`, y lo detiene si pasa de `timeout` o de `MAX_STEPS`.
pub fn run_mars(asm: &str, timeout: Duration) -> Result<SimResult, SimError> {
    static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

    let jar = jar_path().ok_or(SimError::NoJar)?;
    let id = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
    let file = std::env::temp_dir().join(format!("cpsc_mars_{}_{id}.asm", std::process::id()));
    std::fs::write(&file, asm).map_err(|e| SimError::Assemble(format!("no se pudo escribir {}: {e}", file.display())))?;
    let result = run_file(&jar, &file, timeout);
    let _ = std::fs::remove_file(&file);
    result
}

fn run_file(jar: &Path, file: &Path, timeout: Duration) -> Result<SimResult, SimError> {
    let mut child = Command::new("java")
        .args([
            "-Dline.separator=\n",
            "-Dfile.encoding=ISO-8859-1",
            "-Dstdout.encoding=ISO-8859-1",
            "-Dstderr.encoding=ISO-8859-1",
            "-jar",
        ])
        .arg(jar)
        .args(["nc", "sm", "se2", "ae3", &MAX_STEPS.to_string()])
        .arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| SimError::NoJava)?;

    // Se leen aparte: si nadie vacía la tubería, MARS se bloquea al llenarla.
    let stdout = read_in_background(child.stdout.take().expect("stdout es una tubería"));
    let stderr = read_in_background(child.stderr.take().expect("stderr es una tubería"));

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };

    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    let Some(status) = status else {
        return Ok(stopped(&stdout, SimStop::Timeout));
    };
    let exit_code = status.code().unwrap_or(-1);
    if exit_code == EXIT_ASSEMBLE {
        return Err(SimError::Assemble(format!("{stdout}{stderr}")));
    }
    Ok(interpret(&stdout, exit_code))
}

/// Separa lo que imprimió el programa de lo que agregó MARS.
fn interpret(stdout: &str, exit_code: i32) -> SimResult {
    let stdout = stdout.strip_suffix('\n').unwrap_or(stdout);
    let step_notice = format!("\nProgram terminated when maximum step limit {MAX_STEPS} reached.\n");
    if let Some(printed) = stdout.strip_suffix(&step_notice) {
        return stopped(printed, SimStop::StepLimit);
    }
    if exit_code == EXIT_SIMULATION {
        let (printed, error) = stdout.rsplit_once("Error in ").unwrap_or((stdout, ""));
        return stopped(printed, classify(error));
    }
    SimResult { stdout: stdout.to_string(), exit_code, stop: None }
}

fn stopped(printed: &str, stop: SimStop) -> SimResult {
    SimResult { stdout: printed.to_string(), exit_code: EXIT_SIMULATION, stop: Some(stop) }
}

/// Clasifica un error de MARS: `<archivo> line N: Runtime exception at 0x...: <motivo>`.
fn classify(error: &str) -> SimStop {
    let reason = error.split_once("Runtime exception at ").and_then(|(_, rest)| rest.split_once(": ")).map_or(error, |(_, reason)| reason);
    let reason = reason.lines().next().unwrap_or_default();
    if reason.contains("exceeds available heap storage") {
        SimStop::OutOfMemory
    } else if reason.contains("address out of range 0x7f") {
        // Justo debajo del segmento de la pila: `$sp` se salió por el fondo.
        SimStop::StackOverflow
    } else {
        SimStop::Fault(reason.to_string())
    }
}

fn read_in_background(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}
