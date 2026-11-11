//! Handoff de las Fases 0–4 a la Fase 5 (ver `PLAN_MIPS.md` §7): comprueba que
//! `codigo_objetivo` entrega lo que necesitan la CLI, la API, el IDE y la
//! batería. Compila programas reales con los dos asignadores, revisa que el
//! `.asm` esté bien formado y, si hay MARS, que imprima lo mismo que el intérprete.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::{generate, generate_with, Allocator};
use lexer_generator::intermedio::tac::{BinOp, Instr, Operand, TacFunction, TacProgram};
use lexer_generator::intermedio::vm::Vm;
use std::collections::BTreeSet;
use std::fs;

const PROGRAMAS: &[&str] = &[
    "workspace/rubrica.cps",
    "workspace/ejemplo_closures.cps",
    "workspace/tac/ok/arithmetic.cps",
    "workspace/tac/ok/control_flow.cps",
    "workspace/tac/ok/functions.cps",
    "workspace/tac/ok/objects_lists.cps",
    "workspace/tac/ok/runtime_errors.cps",
    "workspace/mips/handoff/llamadas.cps",
    "workspace/mips/handoff/registros.cps",
    "workspace/mips/handoff/objetos.cps",
    "workspace/mips/handoff/textos.cps",
    "workspace/mips/handoff/excepciones.cps",
    "workspace/mips/handoff/sin_try.cps",
];

/// Las instrucciones que el `.asm` puede usar: todas existen en MARS.
const MNEMONICOS: &[&str] = &[
    "add", "addi", "addiu", "addu", "and", "andi", "b", "beq", "beqz", "bge", "bgt", "ble", "blt", "bltz", "bne", "bnez",
    "div", "j", "jal", "jalr", "jr", "la", "lb", "lbu", "li", "lw", "mfhi", "mflo", "move", "mul", "neg", "nop", "nor",
    "not", "or", "ori", "rem", "sb", "seq", "sge", "sgt", "sle", "sll", "slt", "slti", "sltu", "sne", "sra", "srl", "sub",
    "subu", "sw", "syscall", "xor", "xori",
];

fn es_etiqueta(texto: &str) -> bool {
    let mut chars = texto.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
}

/// Los problemas de forma de un `.asm`: etiquetas repetidas, sin definir o
/// inválidas, instrucciones desconocidas y restos de `M900`.
fn problemas_de_forma(asm: &str) -> Vec<String> {
    let mut problemas = Vec::new();
    let mut definidas = BTreeSet::new();
    let mut usadas = BTreeSet::new();
    if asm.contains("M900") {
        problemas.push("queda un M900".to_string());
    }
    for linea in asm.lines() {
        // `#` dentro de un texto no aparece: los textos no ASCII van como
        // `.byte`, y los `.asciiz` se saltan abajo.
        let linea = linea.trim();
        if linea.starts_with(".asciiz") {
            continue;
        }
        let linea = linea.split('#').next().unwrap().trim();
        if linea.is_empty() {
            continue;
        }
        if let Some(etiqueta) = linea.strip_suffix(':') {
            if !es_etiqueta(etiqueta) {
                problemas.push(format!("etiqueta inválida: {etiqueta}"));
            }
            if !definidas.insert(etiqueta.to_string()) {
                problemas.push(format!("etiqueta repetida: {etiqueta}"));
            }
            continue;
        }
        if let Some(valor) = linea.strip_prefix(".word") {
            usadas.extend(valor.split(',').map(|v| v.trim().to_string()).filter(|v| es_etiqueta(v)));
            continue;
        }
        if linea.starts_with('.') {
            continue;
        }
        let (op, resto) = linea.split_once(char::is_whitespace).unwrap_or((linea, ""));
        if !MNEMONICOS.contains(&op) {
            problemas.push(format!("instrucción desconocida: {linea}"));
        }
        for arg in resto.split(',').map(str::trim) {
            let simbolo = arg.split('+').next().unwrap();
            if !arg.contains('(') && !arg.starts_with('$') && es_etiqueta(simbolo) {
                usadas.insert(simbolo.to_string());
            }
        }
    }
    for etiqueta in usadas.difference(&definidas) {
        problemas.push(format!("etiqueta sin definir: {etiqueta}"));
    }
    if !definidas.contains("main") {
        problemas.push("falta main".to_string());
    }
    problemas
}

/// Lo que imprime el intérprete. Un error sin `try` agrega la línea que
/// escribe `rt_error` y termina con código 1 (`PLAN_MIPS.md` §4.7).
fn salida_del_interprete(program: &TacProgram) -> (String, i32) {
    let mut vm = Vm::with_max_steps(program, 50_000_000);
    match vm.run() {
        Ok(resultado) => (resultado.output, 0),
        Err(error) => {
            let linea = format!("Error en tiempo de ejecucion: {}\n", error.program_message());
            (format!("{}{linea}", vm.output()), 1)
        }
    }
}

#[test]
fn los_programas_reales_generan_un_asm_bien_formado_con_los_dos_asignadores() {
    let compilar = common::compilador();

    let mut fallos = Vec::new();
    for ruta in PROGRAMAS {
        let program = match compilar(&fs::read_to_string(ruta).unwrap()) {
            Ok(program) => program,
            Err(error) => {
                fallos.push(format!("{ruta}: {error}"));
                continue;
            }
        };
        let esperado = salida_del_interprete(&program);
        for (etiqueta, allocator) in [("pila", Allocator::Pila), ("getreg", Allocator::GetReg)] {
            match generate_with(&program, allocator) {
                Ok(mips) => {
                    let asm = mips.to_string();
                    fallos.extend(problemas_de_forma(&asm).into_iter().map(|p| format!("{ruta} ({etiqueta}): {p}")));
                    if generate_with(&program, allocator).unwrap().to_string() != asm {
                        fallos.push(format!("{ruta} ({etiqueta}): el .asm cambia entre dos generaciones"));
                    }
                    if let Some(obtenido) = common::correr_mars(&asm) {
                        if obtenido != esperado {
                            fallos.push(format!("{ruta} ({etiqueta}): MARS dio {obtenido:?} y el intérprete {esperado:?}"));
                        }
                    }
                }
                Err(errores) => fallos.push(format!("{ruta} ({etiqueta}): {errores:?}")),
            }
        }
    }
    assert!(fallos.is_empty(), "{}", fallos.join("\n"));
}

fn programa_con_una_funcion() -> TacProgram {
    let mut main = TacFunction::new("main", 0, 8);
    main.max_temps = 1;
    main.body = vec![Instr::Call { dst: Some(Operand::Temp(0)), func: "f".into(), nargs: 0 }];
    let mut f = TacFunction::new("f", 1, 8);
    f.body = vec![Instr::Return(Some(Operand::Int(1)))];
    TacProgram { static_size: 0, data: Vec::new(), main: Some(main), functions: vec![f] }
}

#[test]
fn generate_entrega_lo_que_necesitan_la_api_y_el_ide() {
    let mips = generate(&programa_con_una_funcion()).unwrap();
    let json = mips.to_json();
    assert_eq!(json["asm"].as_str().unwrap(), mips.to_string());
    let stats = json["stats"].as_array().unwrap();
    let funciones: Vec<_> = stats.iter().map(|s| s["funcion"].as_str().unwrap()).collect();
    assert_eq!(funciones, ["main", "_f"]);
    for campo in ["registros_usados", "derrames", "s_guardados"] {
        assert!(stats[0][campo].is_u64(), "falta {campo}");
    }
}

#[test]
fn un_float_llega_como_error_m001_con_mensaje() {
    let mut program = programa_con_una_funcion();
    program.main.as_mut().unwrap().body = vec![Instr::Binary {
        dst: Operand::Temp(0),
        op: BinOp::AddF,
        left: Operand::Float(1.0),
        right: Operand::Float(2.0),
    }];
    let errores = generate(&program).unwrap_err();
    assert!(errores.iter().all(|e| e.code == "M001" && !e.message.is_empty()), "{errores:?}");
}
