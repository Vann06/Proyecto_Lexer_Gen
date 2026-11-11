//! Fase 3: rutinas de apoyo, errores en ejecución y `try`.
#[allow(dead_code)]
mod common;

use lexer_generator::codigo_objetivo::{generate_with, Allocator};
use lexer_generator::intermedio::tac::{BinOp, Instr, Operand, PrintKind, TacFunction, TacProgram};

fn programa(body: Vec<Instr>, max_temps: usize) -> TacProgram {
    let mut main = TacFunction::new("main", 0, 0);
    main.body = body;
    main.max_temps = max_temps;
    TacProgram {
        main: Some(main),
        ..Default::default()
    }
}

#[test]
fn agrega_solo_las_rutinas_usadas_y_sus_dependencias() {
    let int_only = programa(
        vec![Instr::Print {
            value: Operand::Int(7),
            kind: PrintKind::Int,
        }],
        0,
    );
    let asm = generate_with(&int_only, Allocator::Pila)
        .unwrap()
        .to_string();
    assert!(asm.contains("rt_print_int:"), "{asm}");
    assert!(
        !asm.contains("rt_print_bool:") && !asm.contains("rt_alloc:"),
        "{asm}"
    );

    let concat = programa(
        vec![Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Concat,
            left: Operand::Str("a".into()),
            right: Operand::Str("b".into()),
        }],
        1,
    );
    let asm = generate_with(&concat, Allocator::Pila).unwrap().to_string();
    assert!(
        asm.contains("rt_concat:") && asm.contains("rt_alloc:"),
        "{asm}"
    );
}

#[test]
fn try_usa_el_registro_de_48_bytes_acordado() {
    let program = programa(
        vec![
            Instr::TryBegin("catch".into()),
            Instr::TryEnd,
            Instr::Goto("end".into()),
            Instr::Label("catch".into()),
            Instr::GetException(Operand::Temp(0)),
            Instr::Label("end".into()),
        ],
        1,
    );
    let asm = generate_with(&program, Allocator::Pila)
        .unwrap()
        .to_string();
    assert!(asm.contains("addiu $sp, $sp, -48"), "{asm}");
    assert!(asm.contains("sw $fp, 8($sp)"), "{asm}");
    assert!(
        asm.contains("sw $s0, 12($sp)") && asm.contains("sw $s7, 40($sp)"),
        "{asm}"
    );
    assert!(asm.contains("sw $sp, rt_try_top"), "{asm}");
    assert!(asm.contains("addiu $sp, $sp, 48"), "{asm}");
}

#[test]
fn todas_las_instrucciones_de_runtime_se_traducen_y_corren_si_hay_mars() {
    let body = vec![
        Instr::Print {
            value: Operand::Int(42),
            kind: PrintKind::Int,
        },
        Instr::Print {
            value: Operand::Bool(true),
            kind: PrintKind::Bool,
        },
        Instr::Print {
            value: Operand::Str("hola".into()),
            kind: PrintKind::Str,
        },
        Instr::Print {
            value: Operand::Null,
            kind: PrintKind::Ref,
        },
        Instr::Binary {
            dst: Operand::Temp(0),
            op: BinOp::Concat,
            left: Operand::Str("a".into()),
            right: Operand::Str("b".into()),
        },
        Instr::Print {
            value: Operand::Temp(0),
            kind: PrintKind::Str,
        },
        Instr::Binary {
            dst: Operand::Temp(1),
            op: BinOp::StrEq,
            left: Operand::Str("igual".into()),
            right: Operand::Str("igual".into()),
        },
        Instr::Print {
            value: Operand::Temp(1),
            kind: PrintKind::Bool,
        },
        Instr::IfRel {
            left: Operand::Str("x".into()),
            op: BinOp::StrNe,
            right: Operand::Str("y".into()),
            label: "distintos".into(),
        },
        Instr::Print {
            value: Operand::Int(0),
            kind: PrintKind::Int,
        },
        Instr::Label("distintos".into()),
        Instr::Alloc {
            dst: Operand::Temp(2),
            size: Operand::Int(8),
        },
        Instr::Store {
            base: Operand::Temp(2),
            index: Operand::Int(0),
            src: Operand::Int(1),
        },
        Instr::CheckBounds {
            array: Operand::Temp(2),
            index: Operand::Int(0),
        },
        Instr::TryBegin("catch".into()),
        Instr::CheckZero(Operand::Int(0)),
        Instr::TryEnd,
        Instr::Goto("end".into()),
        Instr::Label("catch".into()),
        Instr::GetException(Operand::Temp(3)),
        Instr::Print {
            value: Operand::Temp(3),
            kind: PrintKind::Str,
        },
        Instr::Label("end".into()),
    ];
    let asm = generate_with(&programa(body, 4), Allocator::Pila)
        .expect("ningún M900")
        .to_string();
    for routine in [
        "rt_print_int:",
        "rt_print_bool:",
        "rt_print_str:",
        "rt_print_ref:",
        "rt_alloc:",
        "rt_concat:",
        "rt_streq:",
        "rt_check_bounds:",
        "rt_err_div0:",
        "rt_error:",
    ] {
        assert!(asm.contains(routine), "falta {routine}\n{asm}");
    }
    if let Some((output, code)) = common::correr_mars(&asm) {
        assert_eq!(code, 0, "{output}");
        assert_eq!(
            output,
            "42\ntrue\nhola\nnull\nab\ntrue\ndivision por cero\n"
        );
    }
}

#[test]
fn error_sin_try_termina_con_codigo_uno_si_hay_mars() {
    let asm = generate_with(
        &programa(vec![Instr::CheckZero(Operand::Int(0))], 0),
        Allocator::Pila,
    )
    .unwrap()
    .to_string();
    if let Some((output, code)) = common::correr_mars(&asm) {
        assert_eq!(code, 1);
        assert_eq!(output, "Error en tiempo de ejecucion: division por cero\n");
    }
}

#[test]
fn try_anidado_entre_funciones_desapila_el_handler_correcto() {
    let mut failing = TacFunction::new("falla", 1, 8);
    failing.max_temps = 1;
    failing.body = vec![
        Instr::TryBegin("catch_interno".into()),
        Instr::CheckZero(Operand::Int(0)),
        Instr::TryEnd,
        Instr::Return(None),
        Instr::Label("catch_interno".into()),
        Instr::GetException(Operand::Temp(0)),
        Instr::Print {
            value: Operand::Temp(0),
            kind: PrintKind::Str,
        },
        // El handler interno ya fue consumido: este error debe llegar al de main.
        Instr::CheckZero(Operand::Int(0)),
    ];

    let mut main = TacFunction::new("main", 0, 0);
    main.max_temps = 1;
    main.body = vec![
        Instr::TryBegin("catch_externo".into()),
        Instr::Call {
            dst: None,
            func: "falla".into(),
            nargs: 0,
        },
        Instr::TryEnd,
        Instr::Goto("end".into()),
        Instr::Label("catch_externo".into()),
        Instr::GetException(Operand::Temp(0)),
        Instr::Print {
            value: Operand::Temp(0),
            kind: PrintKind::Str,
        },
        Instr::Label("end".into()),
    ];
    let program = TacProgram {
        main: Some(main),
        functions: vec![failing],
        ..Default::default()
    };
    let asm = generate_with(&program, Allocator::Pila)
        .unwrap()
        .to_string();
    if let Some((output, code)) = common::correr_mars(&asm) {
        assert_eq!(code, 0, "{output}");
        assert_eq!(output, "division por cero\ndivision por cero\n");
    }
}

#[test]
fn return_dentro_de_try_desapila_antes_del_epilogo() {
    let mut f = TacFunction::new("f", 1, 8);
    f.body = vec![
        Instr::TryBegin("catch".into()),
        // El generador TAC inserta este TryEnd antes de cada return.
        Instr::TryEnd,
        Instr::Return(Some(Operand::Int(7))),
        Instr::Label("catch".into()),
        Instr::Return(Some(Operand::Int(0))),
    ];
    let mut main = TacFunction::new("main", 0, 0);
    main.max_temps = 1;
    main.body = vec![
        Instr::Call {
            dst: Some(Operand::Temp(0)),
            func: "f".into(),
            nargs: 0,
        },
        Instr::Print {
            value: Operand::Temp(0),
            kind: PrintKind::Int,
        },
    ];
    let program = TacProgram {
        main: Some(main),
        functions: vec![f],
        ..Default::default()
    };
    let asm = generate_with(&program, Allocator::Pila)
        .unwrap()
        .to_string();
    if let Some((output, code)) = common::correr_mars(&asm) {
        assert_eq!((output.as_str(), code), ("7\n", 0));
    }
}
