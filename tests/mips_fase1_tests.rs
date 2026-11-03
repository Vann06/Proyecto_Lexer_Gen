//! Fase 1 de la generación de MIPS (ver `PLAN_MIPS.md`): la traducción de las
//! instrucciones de expresiones, control de flujo, listas y campos, con la
//! asignación por pila (`StackAlloc`).
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::asm::AsmLine;
use lexer_generator::codigo_objetivo::{generate_partial, generate_with, Allocator};
use lexer_generator::intermedio::tac::{Operand, TacProgram};
use std::path::Path;
use std::process::Command;

/// Corre `asm` en MARS. `None` (con aviso) si no hay `java` o `tools/Mars4_5.jar`,
/// salvo que `MIPS_REQUIRED=1`: ahí es un fallo.
fn correr_mars(asm: &str) -> Option<(String, i32)> {
    let hay_java = Command::new("java").arg("-version").output().map(|o| o.status.success()).unwrap_or(false);
    let jar = Path::new("tools/Mars4_5.jar");
    if !hay_java || !jar.exists() {
        assert!(std::env::var("MIPS_REQUIRED").as_deref() != Ok("1"), "MIPS_REQUIRED=1 pero no hay java o tools/Mars4_5.jar");
        eprintln!("AVISO: se omite la prueba con MARS (no hay java o tools/Mars4_5.jar)");
        return None;
    }
    let archivo = std::env::temp_dir().join(format!("mips_fase1_{}_{:?}.asm", std::process::id(), std::thread::current().id()));
    std::fs::write(&archivo, asm).unwrap();
    let o = Command::new("java")
        .args(["-Dfile.encoding=ISO-8859-1", "-jar"])
        .arg(jar)
        .args(["nc", "sm", "se2", "ae3"])
        .arg(&archivo)
        .output()
        .expect("java debe ejecutarse");
    let _ = std::fs::remove_file(&archivo);
    Some((String::from_utf8_lossy(&o.stdout).into_owned(), o.status.code().unwrap_or(-1)))
}

fn texto(lineas: &[AsmLine]) -> String {
    lineas.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n")
}

/// El `.text` generado con `StackAlloc`; el programa no debe tener `M###`.
fn text_de(source: &str) -> String {
    let mips = generate_with(&common::generar(source), Allocator::Pila).unwrap_or_else(|e| panic!("sin errores: {e:?}"));
    texto(&mips.text)
}

#[test]
fn aritmetica_exacta() {
    let asm = text_de(
        "let a: integer = 1; let b: integer = 2; let c: integer = 3; let d: integer = 4;\n\
         let x: integer = a + b * c - d;\n",
    );
    assert_eq!(
        asm,
        r#"main:
    move $fp, $sp
    addiu $sp, $sp, -8
    # a = 1
    li $t0, 1
    sw $t0, G
    # b = 2
    li $t0, 2
    sw $t0, G+4
    # c = 3
    li $t0, 3
    sw $t0, G+8
    # d = 4
    li $t0, 4
    sw $t0, G+12
    # t0 = b * c
    lw $t0, G+4
    lw $t1, G+8
    mul $t2, $t0, $t1
    sw $t2, -8($fp)
    # t0 = a + t0
    lw $t0, G
    lw $t1, -8($fp)
    addu $t2, $t0, $t1
    sw $t2, -8($fp)
    # x = t0 - d
    lw $t0, -8($fp)
    lw $t1, G+12
    subu $t2, $t0, $t1
    sw $t2, G+16
    li $v0, 10
    syscall"#
    );
}

#[test]
fn if_else_exacto() {
    let asm = text_de("let i: integer = 0; if (i == 3) { i = 1; } else { i = 2; }\n");
    assert_eq!(
        asm,
        r#"main:
    move $fp, $sp
    # i = 0
    li $t0, 0
    sw $t0, G
    # if i != 3 goto L1
    lw $t0, G
    li $t1, 3
    bne $t0, $t1, L1
    # i = 1
    li $t0, 1
    sw $t0, G
    # goto L0
    j L0
L1:
    # i = 2
    li $t0, 2
    sw $t0, G
L0:
    li $v0, 10
    syscall"#
    );
}

#[test]
fn while_exacto() {
    let asm = text_de("let i: integer = 0; while (i < 3) { i = i + 1; }\n");
    assert_eq!(
        asm,
        r#"main:
    move $fp, $sp
    # i = 0
    li $t0, 0
    sw $t0, G
L0:
    # if i >= 3 goto L1
    lw $t0, G
    li $t1, 3
    bge $t0, $t1, L1
    # i = i + 1
    lw $t0, G
    addiu $t1, $t0, 1
    sw $t1, G
    # goto L0
    j L0
L1:
    li $v0, 10
    syscall"#
    );
}

#[test]
fn acceso_a_local_parametro_y_campo() {
    let asm = text_de(
        "function f(a: integer) { let l: integer = a + 1; }\n\
         class P { let x: integer; function inc() { this.x = this.x + 1; } }\n",
    );
    // Parámetro `a` (fp+16) y local `l` (fp-12).
    assert!(asm.contains("_f:\n    move $fp, $sp\n    sw $ra, -4($fp)\n    addiu $sp, $sp, -12\n    # l = a + 1\n    lw $t0, 16($fp)\n    addiu $t1, $t0, 1\n    sw $t1, -12($fp)"), "{asm}");
    assert!(asm.contains("_f.0ret:\n    lw $ra, -4($fp)\n    move $sp, $fp\n    lw $fp, 0($fp)\n    jr $ra"), "{asm}");
    // Campo `x` (4) de un objeto cuya dirección vive en el temporal t0: la
    // base se carga en $t9 desde la ranura de t0 (fp-16, marco 16).
    assert!(asm.contains("    # t0 = t0[4] + 1\n    lw $t9, -16($fp)\n    lw $t0, 4($t9)\n    addiu $t1, $t0, 1\n    sw $t1, -16($fp)"), "{asm}");
    assert!(asm.contains("    # t1[4] = t0\n    lw $t0, -16($fp)\n    move $t1, $t0\n    lw $t9, -24($fp)\n    sw $t1, 4($t9)"), "{asm}");
}

#[test]
fn lista_con_indice_constante_y_variable() {
    // `alloc` y `check_bounds` (de B) quedan como M900; el resto se traduce.
    let program = common::generar("let l = [1, 2];\nlet i: integer = 1;\nlet k: integer = l[i];\n");
    let asm = texto(&generate_partial(&program, Allocator::Pila).0.text);
    // Índice constante (al llenar la lista): `off(base)`; variable: `addu $t8, base, índice`.
    assert!(asm.contains("sw $t1, 4($t0)"), "{asm}");
    assert!(asm.contains("addu $t8, "), "{asm}");
    assert!(asm.contains("($t8)"), "{asm}");
}

/// Solo las instrucciones de B dan `M900`.
#[test]
fn m900_solo_en_instrucciones_de_b() {
    for archivo in ["workspace/rubrica.cps", "workspace/tac/ok/control_flow.cps"] {
        let (mips, errores) = generate_partial(&common::generar(&common::read(archivo)), Allocator::Pila);
        assert!(errores.iter().all(|e| e.code == "M900"), "{archivo}: {errores:?}");
        for e in &errores {
            let tac = e.message.split('`').nth(1).expect("la instrucción va entre comillas");
            let de_b = ["param ", "link ", "call ", "callv ", "return", "print_", "check_", "try_begin", "try_end"]
                .iter()
                .any(|p| tac.starts_with(p))
                || [" = call ", " = callv ", " = alloc ", " concat ", " streq ", " strne ", " = exception"].iter().any(|p| tac.contains(p));
            assert!(de_b, "{archivo}: `{tac}` es de A y quedó sin traducir");
        }
        // Cada M900 deja su comentario en el .asm, que sigue ensamblando.
        let asm = mips.to_string();
        assert_eq!(asm.matches("# M900").count(), errores.len());
    }
}

/// Agrega al final de `main` la impresión de las variables globales `nombres`
/// (una por línea) antes del `exit`. Todavía no hay `print` (es de B), así que
/// la prueba lo escribe a mano.
fn con_impresion(program: &TacProgram, asm: &str, nombres: &[&str]) -> String {
    let mut impresion = String::new();
    for nombre in nombres {
        let offset = program
            .main
            .iter()
            .flat_map(|f| f.body.iter())
            .flat_map(|i| i.operands())
            .find_map(|o| match o {
                Operand::Var { name, offset, .. } if name == nombre => Some(*offset),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no hay variable {nombre}"));
        impresion += &format!("    lw $a0, G+{offset}\n    li $v0, 1\n    syscall\n    li $a0, 10\n    li $v0, 11\n    syscall\n");
    }
    let salida = "    li $v0, 10\n    syscall\n";
    assert!(asm.ends_with(salida), "{asm}");
    format!("{}{impresion}{salida}", &asm[..asm.len() - salida.len()])
}

#[test]
fn en_mars_la_suma_da_la_vuelta_de_32_bits() {
    let program = common::generar("let m: integer = 2147483647; let n: integer = m + 1;\n");
    // `check_zero` (de B) no aparece aquí: el programa no divide.
    let (mips, errores) = generate_partial(&program, Allocator::Pila);
    assert!(errores.is_empty(), "{errores:?}");
    let asm = con_impresion(&program, &mips.to_string(), &["n"]);
    if let Some((salida, codigo)) = correr_mars(&asm) {
        assert_eq!((salida.as_str(), codigo), ("-2147483648\n", 0));
    }
}

#[test]
fn en_mars_la_division_y_el_modulo_truncan_como_el_interprete() {
    let program = common::generar(
        "let a: integer = 0 - 7; let b: integer = 2; let c: integer = 7; let d: integer = 0 - 2;\n\
         let q1: integer = a / b; let r1: integer = a % b; let q2: integer = c / d; let r2: integer = c % d;\n",
    );
    // Los `check_zero` (de B) quedan como comentario M900: los divisores no son cero.
    let (mips, _) = generate_partial(&program, Allocator::Pila);
    let asm = con_impresion(&program, &mips.to_string(), &["q1", "r1", "q2", "r2"]);
    let esperado = format!("{}\n{}\n{}\n{}\n", -7i32.wrapping_div(2), -7i32.wrapping_rem(2), 7i32.wrapping_div(-2), 7i32.wrapping_rem(-2));
    if let Some((salida, codigo)) = correr_mars(&asm) {
        assert_eq!((salida, codigo), (esperado, 0));
    }
}
