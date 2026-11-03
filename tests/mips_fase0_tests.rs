//! Fase 0 de la generación de MIPS (ver `PLAN_MIPS.md`): la sección `.data`,
//! los nombres de las etiquetas, el error `M001` y el esqueleto que ensambla.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::asm::AsmLine;
use lexer_generator::codigo_objetivo::names::{func_label, label_of, ret_label, StringTable};
use lexer_generator::codigo_objetivo::{generate, generate_partial, Allocator};
use lexer_generator::intermedio::tac::{BinOp, Instr, Operand, TacFunction, TacProgram};
use std::collections::HashSet;
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
    let archivo = std::env::temp_dir().join(format!("mips_fase0_{}.asm", std::process::id()));
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

fn funcion(nombre: &str, level: usize, body: Vec<Instr>) -> TacFunction {
    let mut f = TacFunction::new(nombre, level, 0);
    f.body = body;
    f
}

#[test]
fn el_data_de_rubrica_trae_g_las_vtables_y_los_textos_sin_repetir() {
    let program = common::generar(&common::read("workspace/rubrica.cps"));
    let (mips, _) = generate_partial(&program, Allocator::Pila);
    let data = texto(&mips.data);

    assert!(data.contains(&format!("G:\n    .space {}", program.static_size.max(4))), "{data}");
    for item in &program.data {
        let esperado: String = item.entries.iter().map(|e| format!("\n    .word {}", func_label(e))).collect();
        assert!(data.contains(&format!("{}:{esperado}", item.name)), "{data}");
    }
    assert!(data.contains("vt_Animal:\n    .word "), "{data}");

    // Un texto por valor distinto: "hola" aparece una vez aunque se use varias.
    let distintos: HashSet<String> = program
        .main
        .iter()
        .chain(&program.functions)
        .flat_map(|f| f.body.iter())
        .flat_map(|i| i.operands())
        .filter_map(|o| if let Operand::Str(s) = o { Some(s.clone()) } else { None })
        .collect();
    let etiquetas = mips.data.iter().filter(|l| matches!(l, AsmLine::Label(n) if n.starts_with('S'))).count();
    assert_eq!(etiquetas, distintos.len(), "{data}");
    assert!(data.contains(".asciiz \"Compiscript\""), "{data}");
}

#[test]
fn los_nombres_siguen_las_reglas_del_plan() {
    assert_eq!(func_label("contador.sumar"), "_contador.sumar");
    assert_eq!(func_label("Animal.0init"), "_Animal.0init");
    let f = funcion("f", 1, vec![]);
    assert_eq!(label_of(&f), "_f");
    assert_eq!(ret_label(&f), "_f.0ret");
    // El nivel de programa es `main`; un `main` del usuario es `_main`.
    assert_eq!(label_of(&funcion("main", 0, vec![])), "main");
    assert_eq!(label_of(&funcion("main", 1, vec![])), "_main");
}

#[test]
fn un_texto_con_comillas_y_acentos_sale_escapado() {
    let mut t = StringTable::default();
    assert_eq!(t.intern("di \"hola\" \\ a\nb"), "S0");
    assert_eq!(t.intern("índice"), "S1");
    assert_eq!(t.intern("di \"hola\" \\ a\nb"), "S0", "no se repite");
    let e = t.entries();
    assert_eq!(e[0], ("S0".to_string(), r#".asciiz "di \"hola\" \\ a\nb""#.to_string()));
    // í = 0xC3 0xAD en UTF-8, más el 0 final.
    assert_eq!(e[1], ("S1".to_string(), ".byte 195, 173, 110, 100, 105, 99, 101, 0".to_string()));
}

#[test]
fn un_float_da_m001_y_no_hay_asm() {
    let body = vec![Instr::Binary { dst: Operand::Temp(0), op: BinOp::AddF, left: Operand::Float(1.5), right: Operand::Float(2.0) }];
    let mut main = funcion("main", 0, body);
    main.max_temps = 1;
    let program = TacProgram { main: Some(main), ..Default::default() };
    let errores = generate(&program).expect_err("un float no se traduce");
    assert_eq!(errores.len(), 1);
    assert_eq!(errores[0].code, "M001");
}

#[test]
fn un_programa_vacio_ensambla_y_termina_con_cero() {
    let program = TacProgram { main: Some(funcion("main", 0, vec![])), ..Default::default() };
    let asm = generate(&program).expect("programa vacío").to_string();
    assert!(asm.contains(".globl main") && asm.contains("main:") && asm.contains("syscall"), "{asm}");
    if let Some((salida, codigo)) = correr_mars(&asm) {
        assert_eq!((salida.as_str(), codigo), ("", 0));
    }
}
