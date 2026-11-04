//! Traducción de las instrucciones TAC que usan las rutinas de apoyo de
//! `runtime.asm` (`PLAN_MIPS.md`, Fase 3).

use std::collections::HashSet;

use crate::intermedio::tac::{BinOp, Instr, PrintKind, TacProgram};

use super::asm::{AsmLine, Reg};
use super::FunctionEmitter;

const RUNTIME_ASM: &str = include_str!("runtime.asm");

pub fn data_section() -> Vec<AsmLine> {
    vec![
        AsmLine::Directive(".align 2".into()),
        AsmLine::Label("rt_try_top".into()),
        AsmLine::Directive(".word 0".into()),
        AsmLine::Label("rt_exception".into()),
        AsmLine::Directive(".word 0".into()),
        AsmLine::Label("rt_true".into()),
        AsmLine::Directive(r#".asciiz "true""#.into()),
        AsmLine::Label("rt_false".into()),
        AsmLine::Directive(r#".asciiz "false""#.into()),
        AsmLine::Label("rt_null".into()),
        AsmLine::Directive(r#".asciiz "null""#.into()),
        AsmLine::Label("rt_error_prefix".into()),
        AsmLine::Directive(".byte 69, 114, 114, 111, 114, 32, 101, 110, 32, 116, 105, 101, 109, 112, 111, 32, 100, 101, 32, 101, 106, 101, 99, 117, 99, 105, 195, 179, 110, 58, 32, 0".into()),
        AsmLine::Label("rt_msg_index".into()),
        AsmLine::Directive(".byte 195, 173, 110, 100, 105, 99, 101, 32, 102, 117, 101, 114, 97, 32, 100, 101, 32, 114, 97, 110, 103, 111, 0".into()),
        AsmLine::Label("rt_msg_div0".into()),
        AsmLine::Directive(".byte 100, 105, 118, 105, 115, 105, 195, 179, 110, 32, 112, 111, 114, 32, 99, 101, 114, 111, 0".into()),
    ]
}

/// Bloques de `runtime.asm` que necesita el programa, con dependencias.
pub fn text_section(program: &TacProgram) -> Vec<AsmLine> {
    let mut needed = HashSet::new();
    for f in program.main.iter().chain(&program.functions) {
        for instr in &f.body {
            match instr {
                Instr::Print { kind, .. } => {
                    needed.insert(match kind {
                        PrintKind::Int => "rt_print_int",
                        PrintKind::Bool => "rt_print_bool",
                        PrintKind::Str => "rt_print_str",
                        PrintKind::Ref => "rt_print_ref",
                        PrintKind::Float => continue,
                    });
                }
                Instr::Alloc { .. } => {
                    needed.insert("rt_alloc");
                }
                Instr::Binary {
                    op: BinOp::Concat, ..
                } => {
                    needed.insert("rt_concat");
                    needed.insert("rt_alloc");
                }
                Instr::Binary {
                    op: BinOp::StrEq | BinOp::StrNe,
                    ..
                }
                | Instr::IfRel {
                    op: BinOp::StrEq | BinOp::StrNe,
                    ..
                } => {
                    needed.insert("rt_streq");
                }
                Instr::CheckBounds { .. } => {
                    needed.insert("rt_check_bounds");
                    needed.insert("rt_error");
                }
                Instr::CheckZero(_) => {
                    needed.insert("rt_err_div0");
                    needed.insert("rt_error");
                }
                _ => {}
            }
        }
    }
    parse_selected(&needed)
}

pub fn translate(fe: &mut FunctionEmitter, instr: &Instr) -> bool {
    match instr {
        Instr::Print { value, kind } if *kind != PrintKind::Float => {
            let value = fe.use_op(value);
            fe.emit("move", &[&Reg::A(0), &value]);
            let routine = match kind {
                PrintKind::Int => "rt_print_int",
                PrintKind::Bool => "rt_print_bool",
                PrintKind::Str => "rt_print_str",
                PrintKind::Ref => "rt_print_ref",
                PrintKind::Float => unreachable!(),
            };
            fe.emit("jal", &[&routine]);
        }
        Instr::Alloc { dst, size } => {
            let size = fe.use_op(size);
            fe.emit("move", &[&Reg::A(0), &size]);
            fe.emit("jal", &[&"rt_alloc"]);
            define_from_v0(fe, dst);
        }
        Instr::Binary {
            dst,
            op: op @ (BinOp::Concat | BinOp::StrEq | BinOp::StrNe),
            left,
            right,
        } => {
            let left = fe.use_op(left);
            let right = fe.use_op(right);
            fe.emit("move", &[&Reg::A(0), &left]);
            fe.emit("move", &[&Reg::A(1), &right]);
            fe.emit(
                "jal",
                &[&if *op == BinOp::Concat {
                    "rt_concat"
                } else {
                    "rt_streq"
                }],
            );
            let dst_reg = fe.def_op(dst);
            if *op == BinOp::StrNe {
                fe.emit("xori", &[&dst_reg, &Reg::V0, &1]);
            } else {
                fe.emit("move", &[&dst_reg, &Reg::V0]);
            }
            fe.finish_def(dst, dst_reg);
        }
        Instr::IfRel {
            left,
            op: op @ (BinOp::StrEq | BinOp::StrNe),
            right,
            label,
        } => {
            let left = fe.use_op(left);
            let right = fe.use_op(right);
            fe.emit("move", &[&Reg::A(0), &left]);
            fe.emit("move", &[&Reg::A(1), &right]);
            fe.emit("jal", &[&"rt_streq"]);
            fe.alloc.block_boundary(&mut fe.out);
            fe.emit(
                if *op == BinOp::StrEq { "bnez" } else { "beqz" },
                &[&Reg::V0, label],
            );
        }
        Instr::CheckBounds { array, index } => {
            let array = fe.use_op(array);
            let index = fe.use_op(index);
            fe.emit("move", &[&Reg::A(0), &array]);
            fe.emit("move", &[&Reg::A(1), &index]);
            fe.emit("jal", &[&"rt_check_bounds"]);
        }
        Instr::CheckZero(value) => {
            let value = fe.use_op(value);
            fe.emit("beqz", &[&value, &"rt_err_div0"]);
        }
        Instr::TryBegin(label) => try_begin(fe, label),
        Instr::TryEnd => try_end(fe),
        Instr::GetException(dst) => {
            let reg = fe.def_op(dst);
            fe.emit("lw", &[&reg, &"rt_exception"]);
            fe.finish_def(dst, reg);
        }
        _ => return false,
    }
    true
}

fn define_from_v0(fe: &mut FunctionEmitter, dst: &crate::intermedio::tac::Operand) {
    let reg = fe.def_op(dst);
    fe.emit("move", &[&reg, &Reg::V0]);
    fe.finish_def(dst, reg);
}

fn try_begin(fe: &mut FunctionEmitter, label: &str) {
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &-48]);
    fe.emit("lw", &[&Reg::T(8), &"rt_try_top"]);
    fe.emit("sw", &[&Reg::T(8), &format!("0({})", Reg::Sp)]);
    fe.emit("la", &[&Reg::T(9), &label]);
    fe.emit("sw", &[&Reg::T(9), &format!("4({})", Reg::Sp)]);
    fe.emit("sw", &[&Reg::Fp, &format!("8({})", Reg::Sp)]);
    for i in 0..8u8 {
        fe.emit("sw", &[&Reg::S(i), &format!("{}({})", 12 + 4 * i, Reg::Sp)]);
    }
    fe.emit("sw", &[&Reg::Sp, &"rt_try_top"]);
}

fn try_end(fe: &mut FunctionEmitter) {
    fe.emit("lw", &[&Reg::T(8), &"rt_try_top"]);
    fe.emit("lw", &[&Reg::T(9), &format!("0({})", Reg::T(8))]);
    fe.emit("sw", &[&Reg::T(9), &"rt_try_top"]);
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &48]);
}

fn parse_selected(needed: &HashSet<&str>) -> Vec<AsmLine> {
    let mut out = Vec::new();
    let mut selected = false;
    for raw in RUNTIME_ASM.lines() {
        let line = raw.trim();
        if let Some(name) = line.strip_prefix("# BEGIN ") {
            selected = needed.contains(name);
            continue;
        }
        if line.starts_with("# END ") {
            selected = false;
            continue;
        }
        if !selected || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(label) = line.strip_suffix(':') {
            out.push(AsmLine::Label(label.into()));
            continue;
        }
        let (op, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let op: &'static str = match op {
            "addiu" => "addiu",
            "addu" => "addu",
            "beqz" => "beqz",
            "bge" => "bge",
            "bltz" => "bltz",
            "bne" => "bne",
            "j" => "j",
            "jal" => "jal",
            "jr" => "jr",
            "la" => "la",
            "lbu" => "lbu",
            "li" => "li",
            "lw" => "lw",
            "move" => "move",
            "sb" => "sb",
            "sw" => "sw",
            "syscall" => "syscall",
            other => panic!("instrucción desconocida en runtime.asm: {other}"),
        };
        out.push(AsmLine::Instr {
            op,
            args: args
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect(),
        });
    }
    out
}
