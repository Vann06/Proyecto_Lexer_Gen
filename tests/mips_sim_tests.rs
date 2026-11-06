//! Fase 5 de la generación de MIPS (ver `PLAN_MIPS.md`): `run_mars` entrega lo
//! que imprimió el programa, ni más ni menos, y distingue cada forma de fallar.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::sim::{run_mars, SimError, SimResult, DEFAULT_TIMEOUT};
use std::time::Duration;

/// Un programa que imprime `texto` y termina con `salida` (`syscall` 17).
fn imprime(texto: &str, salida: i32) -> String {
    format!(
        ".data\ns: .asciiz \"{texto}\"\n.text\n.globl main\nmain:\n  la $a0, s\n  li $v0, 4\n  syscall\n  li $a0, {salida}\n  li $v0, 17\n  syscall\n"
    )
}

fn corre(asm: &str) -> Option<Result<SimResult, SimError>> {
    common::hay_mars().then(|| run_mars(asm, DEFAULT_TIMEOUT))
}

fn resultado(stdout: &str, exit_code: i32) -> Result<SimResult, SimError> {
    Ok(SimResult { stdout: stdout.to_string(), exit_code })
}

#[test]
fn un_programa_que_no_imprime_da_una_salida_vacia() {
    if let Some(r) = corre(".text\n.globl main\nmain:\n  li $v0, 10\n  syscall\n") {
        assert_eq!(r, resultado("", 0));
    }
}

#[test]
fn conserva_tal_cual_lo_que_imprime_el_programa() {
    for (texto, esperado) in [("hola", "hola"), ("hola\\n", "hola\n"), ("a\\n\\n", "a\n\n"), ("\\n", "\n")] {
        if let Some(r) = corre(&imprime(texto, 0)) {
            assert_eq!(r, resultado(esperado, 0), "{texto}");
        }
    }
}

#[test]
fn entrega_el_codigo_de_salida_del_programa() {
    if let Some(r) = corre(&imprime("fallo\\n", 1)) {
        assert_eq!(r, resultado("fallo\n", 1));
    }
}

#[test]
fn un_asm_invalido_es_un_error_de_ensamblado_con_el_mensaje_de_mars() {
    if let Some(r) = corre(".text\nmain:\n  noexiste $t0\n") {
        match r {
            Err(SimError::Assemble(mensaje)) => assert!(mensaje.contains("noexiste"), "{mensaje}"),
            otro => panic!("{otro:?}"),
        }
    }
}

#[test]
fn un_fallo_durante_la_simulacion_da_codigo_dos() {
    if let Some(r) = corre(".text\n.globl main\nmain:\n  li $t0, 1\n  lw $t1, 0($t0)\n") {
        let r = r.unwrap();
        assert_eq!(r.exit_code, 2);
        assert!(r.stdout.contains("Runtime exception"), "{}", r.stdout);
    }
}

#[test]
fn un_ciclo_infinito_se_detiene_por_tiempo() {
    if common::hay_mars() {
        let r = run_mars(".text\n.globl main\nmain:\n  j main\n", Duration::from_secs(3));
        assert_eq!(r, Err(SimError::Timeout));
    }
}
