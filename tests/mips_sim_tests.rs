//! Fase 5 de la generación de MIPS (ver `PLAN_MIPS.md`): `run_mars` entrega lo
//! que imprimió el programa, ni más ni menos, y distingue cada forma de fallar.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::sim::{run_mars, SimError, SimResult, SimStop, DEFAULT_TIMEOUT};
use std::time::Duration;

/// Un programa que imprime `texto` y termina con `salida` (`syscall` 17).
fn imprime(texto: &str, salida: i32) -> String {
    format!(
        ".data\ns: .asciiz \"{texto}\"\n.text\n.globl main\nmain:\n  la $a0, s\n  li $v0, 4\n  syscall\n  li $a0, {salida}\n  li $v0, 17\n  syscall\n"
    )
}

/// Un programa que imprime `1` y sigue con `resto`, que nunca termina bien.
fn imprime_y_sigue(resto: &str) -> String {
    format!(".text\n.globl main\nmain:\n  li $a0, 1\n  li $v0, 1\n  syscall\nsigue:\n{resto}\n")
}

fn corre(asm: &str) -> Option<Result<SimResult, SimError>> {
    common::hay_mars().then(|| run_mars(asm, DEFAULT_TIMEOUT))
}

fn resultado(stdout: &str, exit_code: i32) -> Result<SimResult, SimError> {
    Ok(SimResult { stdout: stdout.to_string(), exit_code, stop: None })
}

fn detenido(stdout: &str, stop: SimStop) -> Result<SimResult, SimError> {
    Ok(SimResult { stdout: stdout.to_string(), exit_code: 2, stop: Some(stop) })
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
fn un_ciclo_infinito_alcanza_el_limite_y_conserva_la_salida() {
    if let Some(r) = corre(&imprime_y_sigue("  j sigue")) {
        assert_eq!(r, detenido("1", SimStop::StepLimit));
    }
}

#[test]
fn una_recursion_infinita_agota_la_pila_y_conserva_la_salida() {
    if let Some(r) = corre(&imprime_y_sigue("  addi $sp, $sp, -4\n  sw $ra, 0($sp)\n  j sigue")) {
        assert_eq!(r, detenido("1", SimStop::StackOverflow));
    }
}

#[test]
fn reservar_sin_fin_agota_la_memoria_y_conserva_la_salida() {
    if let Some(r) = corre(&imprime_y_sigue("  li $a0, 1048576\n  li $v0, 9\n  syscall\n  j sigue")) {
        assert_eq!(r, detenido("1", SimStop::OutOfMemory));
    }
}

#[test]
fn otro_fallo_de_la_simulacion_trae_el_motivo_de_mars() {
    if let Some(r) = corre(&imprime_y_sigue("  li $t0, 1\n  lw $t1, 0($t0)")) {
        let r = r.unwrap();
        assert_eq!((r.stdout.as_str(), r.exit_code), ("1", 2));
        match r.stop {
            Some(SimStop::Fault(motivo)) => assert!(motivo.contains("not aligned") && !motivo.contains(".asm"), "{motivo}"),
            otro => panic!("{otro:?}"),
        }
    }
}

#[test]
fn si_se_acaba_el_tiempo_se_detiene() {
    if common::hay_mars() {
        let r = run_mars(".text\n.globl main\nmain:\n  j main\n", Duration::from_millis(100)).unwrap();
        assert_eq!((r.exit_code, r.stop), (2, Some(SimStop::Timeout)));
    }
}
