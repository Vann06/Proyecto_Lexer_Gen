//! Fase 4: vida de temporales y asignación `getReg()`.
#[allow(dead_code)]
mod common;

use std::collections::BTreeSet;

use lexer_generator::codigo_objetivo::liveness::Liveness;
use lexer_generator::codigo_objetivo::{generate_with, Allocator};
use lexer_generator::intermedio::tac::{BinOp, Instr, Operand, TacFunction, TacProgram};

fn set(values: &[usize]) -> BTreeSet<usize> {
    values.iter().copied().collect()
}

fn function(
    name: &str,
    level: usize,
    frame: usize,
    max_temps: usize,
    body: Vec<Instr>,
) -> TacFunction {
    let mut f = TacFunction::new(name, level, frame);
    f.max_temps = max_temps;
    f.body = body;
    f
}

fn program(main: TacFunction, functions: Vec<TacFunction>) -> TacProgram {
    TacProgram {
        main: Some(main),
        functions,
        ..Default::default()
    }
}

#[test]
fn liveness_exacta_de_while() {
    let f = function(
        "main",
        0,
        0,
        1,
        vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(0),
            },
            Instr::Label("L0".into()),
            Instr::IfRel {
                left: Operand::Temp(0),
                op: BinOp::Ge,
                right: Operand::Int(3),
                label: "L1".into(),
            },
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Add,
                left: Operand::Temp(0),
                right: Operand::Int(1),
            },
            Instr::Goto("L0".into()),
            Instr::Label("L1".into()),
            Instr::Return(Some(Operand::Temp(0))),
        ],
    );
    let live = Liveness::analyze(&f);
    assert_eq!(
        live.live_in,
        vec![
            set(&[]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0])
        ]
    );
    assert_eq!(
        live.live_out,
        vec![
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[0]),
            set(&[])
        ]
    );
    assert_eq!(
        live.blocks
            .iter()
            .map(|b| (b.start, b.end, b.successors.clone()))
            .collect::<Vec<_>>(),
        vec![
            (0, 0, vec![1]),
            (1, 2, vec![2, 3]),
            (3, 4, vec![1]),
            (5, 6, vec![])
        ]
    );
}

#[test]
fn liveness_exacta_de_if_else() {
    let f = function(
        "main",
        0,
        0,
        2,
        vec![
            Instr::IfTrue {
                cond: Operand::Temp(0),
                label: "L1".into(),
            },
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Int(1),
            },
            Instr::Goto("L2".into()),
            Instr::Label("L1".into()),
            Instr::Copy {
                dst: Operand::Temp(1),
                src: Operand::Int(2),
            },
            Instr::Label("L2".into()),
            Instr::Return(Some(Operand::Temp(1))),
        ],
    );
    let live = Liveness::analyze(&f);
    assert_eq!(
        live.live_in,
        vec![
            set(&[0]),
            set(&[]),
            set(&[1]),
            set(&[]),
            set(&[]),
            set(&[1]),
            set(&[1])
        ]
    );
    assert_eq!(
        live.live_out,
        vec![
            set(&[]),
            set(&[1]),
            set(&[1]),
            set(&[]),
            set(&[1]),
            set(&[1]),
            set(&[])
        ]
    );
}

#[test]
fn expresion_simple_no_guarda_temporales_en_la_pila() {
    let main = function(
        "main",
        0,
        0,
        1,
        vec![
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Mul,
                left: Operand::global("b", 4),
                right: Operand::global("c", 8),
            },
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Add,
                left: Operand::global("a", 0),
                right: Operand::Temp(0),
            },
            Instr::Binary {
                dst: Operand::global("x", 16),
                op: BinOp::Sub,
                left: Operand::Temp(0),
                right: Operand::global("d", 12),
            },
        ],
    );
    let mips = generate_with(&program(main, vec![]), Allocator::GetReg).unwrap();
    let asm = mips.to_string();
    assert!(
        !asm.contains("-8($fp)"),
        "el temporal no debe tocar su ranura:\n{asm}"
    );
    assert!(asm.contains("# t0 -> $t"), "{asm}");
    assert_eq!(mips.stats[0].derrames, 0);
}

#[test]
fn mas_de_ocho_temporales_vivos_produce_derrame() {
    let mut body = Vec::new();
    for temp in 0..9 {
        body.push(Instr::Copy {
            dst: Operand::Temp(temp),
            src: Operand::Int(temp as i64),
        });
    }
    body.push(Instr::Binary {
        dst: Operand::Temp(9),
        op: BinOp::Add,
        left: Operand::Temp(0),
        right: Operand::Temp(1),
    });
    for temp in 2..9 {
        body.push(Instr::Binary {
            dst: Operand::Temp(9),
            op: BinOp::Add,
            left: Operand::Temp(9),
            right: Operand::Temp(temp),
        });
    }
    let main = function("main", 0, 0, 10, body);
    let mips = generate_with(&program(main, vec![]), Allocator::GetReg).unwrap();
    let asm = mips.to_string();
    assert!(asm.contains("# derrame t"), "{asm}");
    assert!(mips.stats[0].derrames > 0, "{:?}", mips.stats[0]);
}

#[test]
fn temporal_vivo_durante_call_usa_s_y_se_preserva() {
    let f = function("f", 1, 8, 0, vec![Instr::Return(Some(Operand::Int(5)))]);
    let g = function("g", 1, 8, 0, vec![Instr::Return(Some(Operand::Int(6)))]);
    let h = function(
        "h",
        1,
        8,
        3,
        vec![
            Instr::Call {
                dst: Some(Operand::Temp(0)),
                func: "f".into(),
                nargs: 0,
            },
            Instr::Call {
                dst: Some(Operand::Temp(1)),
                func: "g".into(),
                nargs: 0,
            },
            Instr::Binary {
                dst: Operand::Temp(2),
                op: BinOp::Add,
                left: Operand::Temp(0),
                right: Operand::Temp(1),
            },
            Instr::Return(Some(Operand::Temp(2))),
        ],
    );
    let main = function(
        "main",
        0,
        0,
        0,
        vec![Instr::Call {
            dst: None,
            func: "h".into(),
            nargs: 0,
        }],
    );
    let mips = generate_with(&program(main, vec![f, g, h]), Allocator::GetReg).unwrap();
    let asm = mips.to_string();
    assert!(asm.contains("# t0 -> $s0"), "{asm}");
    assert!(
        asm.contains("sw $s0, -36($fp)") && asm.contains("lw $s0, -36($fp)"),
        "{asm}"
    );
    let h_stats = mips.stats.iter().find(|s| s.funcion == "_h").unwrap();
    assert_eq!(h_stats.s_guardados, 1);
}

#[test]
fn llamada_con_nueve_temporales_conserva_todos_los_argumentos() {
    let mut body = Vec::new();
    for temp in 0..9 {
        body.push(Instr::Copy {
            dst: Operand::Temp(temp),
            src: Operand::Int((temp + 1) as i64),
        });
    }
    for temp in 0..9 {
        body.push(Instr::Param(Operand::Temp(temp)));
    }
    body.push(Instr::Call {
        dst: None,
        func: "nueve".into(),
        nargs: 9,
    });
    let main = function("main", 0, 0, 9, body);
    let callee = function("nueve", 1, 8, 0, vec![Instr::Return(None)]);
    let asm = generate_with(&program(main, vec![callee]), Allocator::GetReg)
        .unwrap()
        .to_string();
    for offset in (16..=48).step_by(4) {
        assert!(
            asm.contains(&format!("{offset}($sp)")),
            "falta argumento en {offset}:\n{asm}"
        );
    }
}

#[test]
fn getreg_emite_menos_accesos_a_memoria_que_pila() {
    let main = function(
        "main",
        0,
        0,
        2,
        vec![
            Instr::Copy {
                dst: Operand::Temp(0),
                src: Operand::Int(10),
            },
            Instr::Binary {
                dst: Operand::Temp(1),
                op: BinOp::Add,
                left: Operand::Temp(0),
                right: Operand::Int(2),
            },
            Instr::Binary {
                dst: Operand::Temp(0),
                op: BinOp::Mul,
                left: Operand::Temp(1),
                right: Operand::Int(3),
            },
            Instr::Copy {
                dst: Operand::global("resultado", 0),
                src: Operand::Temp(0),
            },
        ],
    );
    let p = program(main, vec![]);
    let stack = generate_with(&p, Allocator::Pila).unwrap().to_string();
    let getreg = generate_with(&p, Allocator::GetReg).unwrap().to_string();
    let memory_ops = |asm: &str| {
        asm.lines()
            .filter(|line| {
                line.trim_start().starts_with("lw ") || line.trim_start().starts_with("sw ")
            })
            .count()
    };
    assert!(
        memory_ops(&getreg) < memory_ops(&stack),
        "Pila:\n{stack}\ngetReg:\n{getreg}"
    );
}

#[test]
fn rubrica_completa_genera_con_getreg_sin_errores() {
    let tac = common::generar(&common::read("workspace/rubrica.cps"));
    let stack = generate_with(&tac, Allocator::Pila).expect("rubrica con pila");
    let getreg = generate_with(&tac, Allocator::GetReg).expect("rubrica con getReg");
    assert_eq!(stack.stats.len(), getreg.stats.len());
    assert!(
        getreg.stats.iter().any(|s| s.registros_usados > 0),
        "{:?}",
        getreg.stats
    );
    assert!(!getreg.to_string().contains("# M900"));
}
