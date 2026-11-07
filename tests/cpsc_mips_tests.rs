//! Fase 5 de la generación de MIPS (ver `PLAN_MIPS.md`): `cpsc` lleva un `.cps` a
//! `.asm` y lo ejecuta en MARS. Corre el binario real.
#[allow(dead_code)]
mod common;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

/// Una carpeta propia para los archivos de una prueba.
fn carpeta(nombre: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cpsc_mips_{}_{nombre}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Corre `cpsc` sobre `fuente` y devuelve lo que hizo.
fn cpsc(dir: &PathBuf, fuente: &str, opciones: &[&str]) -> Output {
    let programa = dir.join("programa.cps");
    fs::write(&programa, fuente).unwrap();
    Command::new(env!("CARGO_BIN_EXE_cpsc")).arg(&programa).args(opciones).output().expect("cpsc debe ejecutarse")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn o_escribe_el_asm_y_pila_cambia_la_asignacion() {
    let dir = carpeta("o");
    let fuente = "let a: integer = 2; let b: integer = a * 3 + a; print(b);\n";
    let con_getreg = dir.join("getreg.asm");
    let con_pila = dir.join("pila.asm");

    let salida = cpsc(&dir, fuente, &["--mips", "-o", con_getreg.to_str().unwrap()]);
    assert!(salida.status.success(), "{}", stderr(&salida));
    let asm = fs::read_to_string(&con_getreg).unwrap();
    assert!(asm.contains("main:"), "{asm}");
    assert!(stdout(&salida).contains(&format!("MIPS\n====\n{asm}")), "--mips imprime el mismo .asm");

    let salida = cpsc(&dir, fuente, &["-o", con_pila.to_str().unwrap(), "--pila"]);
    assert!(salida.status.success(), "{}", stderr(&salida));
    assert_ne!(fs::read_to_string(&con_pila).unwrap(), asm);

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn con_errores_no_se_escribe_el_asm() {
    let dir = carpeta("errores");
    let asm = dir.join("salida.asm");

    let salida = cpsc(&dir, "print(noExiste);\n", &["-o", asm.to_str().unwrap()]);

    assert_eq!(salida.status.code(), Some(1));
    assert!(stderr(&salida).contains("S002"), "{}", stderr(&salida));
    assert!(!asm.exists());

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sim_muestra_la_salida_de_mars() {
    if !common::hay_mars() {
        return;
    }
    let dir = carpeta("sim");

    let salida = cpsc(&dir, "print(2 + 3);\n", &["--sim"]);

    assert!(salida.status.success(), "{}", stderr(&salida));
    assert!(stdout(&salida).ends_with("SALIDA DE MARS\n==============\n5\n"), "{}", stdout(&salida));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn sim_conserva_la_salida_y_falla_si_mars_detiene_el_programa() {
    if !common::hay_mars() {
        return;
    }
    let dir = carpeta("ciclo");

    let salida = cpsc(&dir, "print(1); let x: integer = 0; while (true) { x = x + 1; }\n", &["--sim"]);

    assert_eq!(salida.status.code(), Some(1));
    assert!(stdout(&salida).ends_with("SALIDA DE MARS\n==============\n1\n"), "{}", stdout(&salida));
    assert!(stderr(&salida).contains("M902"), "{}", stderr(&salida));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn run_conserva_la_salida_si_el_programa_falla_al_ejecutarse() {
    let dir = carpeta("run");

    let salida = cpsc(&dir, "let l: integer[] = [1];\nprint(7);\nprint(l[5]);\n", &["--run"]);

    assert_eq!(salida.status.code(), Some(1));
    assert!(stdout(&salida).ends_with("SALIDA\n======\n7\n"), "{}", stdout(&salida));
    assert!(stderr(&salida).contains("R001"), "{}", stderr(&salida));
    assert!(stderr(&salida).contains("falló al ejecutarse"), "{}", stderr(&salida));
    assert!(!stderr(&salida).contains("compilación"), "sí compiló: {}", stderr(&salida));

    let _ = fs::remove_dir_all(dir);
}
