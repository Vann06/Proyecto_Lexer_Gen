//! Fase 2: registros de activación, llamadas directas/virtuales y retorno.
use lexer_generator::codigo_objetivo::{generate_with, Allocator};
use lexer_generator::intermedio::tac::{Base, Instr, Operand, TacFunction, TacProgram};

fn funcion(nombre: &str, nivel: usize, marco: usize, body: Vec<Instr>) -> TacFunction {
    let mut f = TacFunction::new(nombre, nivel, marco);
    f.body = body;
    f
}

#[test]
fn llamada_con_dos_parametros_y_retorno_exacta() {
    let x = Operand::global("x", 0);
    let main = funcion(
        "main",
        0,
        0,
        vec![
            Instr::Param(Operand::Int(7)),
            Instr::Param(Operand::Int(8)),
            Instr::Call {
                dst: Some(x),
                func: "suma".into(),
                nargs: 2,
            },
        ],
    );
    let suma = funcion(
        "suma",
        1,
        8,
        vec![Instr::Return(Some(Operand::local("a", 16)))],
    );
    let program = TacProgram {
        static_size: 4,
        main: Some(main),
        functions: vec![suma],
        ..Default::default()
    };
    let text = generate_with(&program, Allocator::Pila).unwrap().text;
    let text = text
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(
        text,
        r#"main:
    move $fp, $sp
    addiu $sp, $sp, -24
    # param 7
    li $t0, 7
    sw $t0, 16($sp)
    # param 8
    li $t1, 8
    sw $t1, 20($sp)
    sw $fp, 0($sp)
    # x = call suma, 2
    jal _suma
    lw $t2, 4($sp)
    sw $t2, G
    addiu $sp, $sp, 24
    li $v0, 10
    syscall
_suma:
    move $fp, $sp
    sw $ra, -4($fp)
    addiu $sp, $sp, -8
    # return a
    lw $t0, 16($fp)
    sw $t0, 4($fp)
    j _suma.0ret
_suma.0ret:
    lw $ra, -4($fp)
    move $sp, $fp
    lw $fp, 0($fp)
    jr $ra"#
    );
}

#[test]
fn link_fp_pasa_el_marco_actual_no_su_enlace_de_control() {
    let fp = Operand::Var {
        name: "fp".into(),
        base: Base::Fp,
        offset: 0,
    };
    let main = funcion(
        "main",
        0,
        0,
        vec![
            Instr::Link(fp),
            Instr::Call {
                dst: None,
                func: "interna".into(),
                nargs: 0,
            },
        ],
    );
    let interna = funcion("interna", 1, 8, vec![Instr::Return(None)]);
    let program = TacProgram {
        main: Some(main),
        functions: vec![interna],
        ..Default::default()
    };
    let asm = generate_with(&program, Allocator::Pila)
        .unwrap()
        .to_string();
    assert!(asm.contains("sw $fp, 12($sp)"), "{asm}");
    assert!(
        !asm.contains("# link fp\n    lw"),
        "link fp no debe cargar 0($fp): {asm}"
    );
}

#[test]
fn callv_con_calculo_intermedio_conserva_el_parametro() {
    let mut main = funcion(
        "main",
        0,
        0,
        vec![
            Instr::Param(Operand::Temp(0)),
            Instr::Load {
                dst: Operand::Temp(0),
                base: Operand::Temp(0),
                index: Operand::Int(0),
            },
            Instr::Load {
                dst: Operand::Temp(0),
                base: Operand::Temp(0),
                index: Operand::Int(4),
            },
            Instr::CallV {
                dst: None,
                target: Operand::Temp(0),
                nargs: 1,
            },
        ],
    );
    main.max_temps = 1;
    let program = TacProgram {
        main: Some(main),
        ..Default::default()
    };
    let asm = generate_with(&program, Allocator::Pila)
        .unwrap()
        .to_string();
    let reserva = asm.find("addiu $sp, $sp, -20").unwrap();
    let guarda = asm.find("sw $t0, 16($sp)").unwrap();
    let primer_load = asm.find("# t0 = t0[0]").unwrap();
    let jalr = asm.find("jalr").unwrap();
    assert!(
        reserva < guarda && guarda < primer_load && primer_load < jalr,
        "{asm}"
    );
}
