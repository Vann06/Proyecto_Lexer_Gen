//! Ejecuta un `.asm` en MARS y devuelve exactamente lo que imprimió el programa
//! (`PLAN_MIPS.md` §4.1).
//!
//! Al terminar, MARS siempre escribe un salto de línea propio, aunque el programa
//! no imprima nada. No es salida del programa, así que `run_mars` quita ese único
//! salto; los que imprime el programa se conservan. `-Dline.separator` hace que
//! sea `\n` también en Windows, donde Java usaría `\r\n`.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

/// Tiempo máximo de una ejecución normal.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

const JAR: &str = "tools/Mars4_5.jar";

/// Los códigos que MARS devuelve por `se2` y `ae3`.
const EXIT_ASSEMBLE: i32 = 3;

/// El resultado de un programa que ensambló. `exit_code` es 0 si terminó bien,
/// 1 si fue un error de Compiscript sin `try` y 2 si MARS detuvo la simulación;
/// en ese último caso `stdout` trae también el mensaje de MARS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimResult {
    pub stdout: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SimError {
    #[error("no se pudo ejecutar java")]
    NoJava,
    #[error("no se encontró {JAR}")]
    NoJar,
    #[error("MARS no terminó a tiempo")]
    Timeout,
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

/// Ensambla y ejecuta `asm`, y lo detiene si pasa de `timeout`.
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
        .args(["nc", "sm", "se2", "ae3"])
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
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < timeout => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SimError::Timeout);
            }
        }
    };

    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    let exit_code = status.code().unwrap_or(-1);
    if exit_code == EXIT_ASSEMBLE {
        return Err(SimError::Assemble(format!("{stdout}{stderr}")));
    }
    let stdout = stdout.strip_suffix('\n').map(str::to_string).unwrap_or(stdout);
    Ok(SimResult { stdout, exit_code })
}

fn read_in_background(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        String::from_utf8_lossy(&bytes).into_owned()
    })
}
