//! Traducción de las instrucciones de expresiones, control de flujo, listas y
//! campos (`PLAN_MIPS.md` §6, Fase 1). Siempre pide los registros a
//! `RegAlloc`; nunca escribe un `$t` por su cuenta salvo `$t8` para sumar
//! base e índice.

use crate::intermedio::tac::{BinOp, Instr, Operand, UnOp};

use super::asm::Reg;
use super::names::vtable_label;
use super::regs::{load_const, AUX_DIR};
use super::FunctionEmitter;

/// Traduce `instr`. Devuelve `false` si no es de esta parte (llamadas,
/// `print`, `check_*`, `try`, `alloc`, `concat`, `streq`/`strne`): el que
/// llama decide qué hacer (hoy, `M900`).
pub fn translate(fe: &mut FunctionEmitter, instr: &Instr) -> bool {
    match instr {
        Instr::Copy { dst, src } => copy(fe, dst, src),
        Instr::Binary { dst, op, left, right } => match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul => aritmetica(fe, dst, *op, left, right),
            BinOp::Div | BinOp::Mod => division(fe, dst, *op, left, right),
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => relacional(fe, dst, *op, left, right),
            _ => return false,
        },
        Instr::Unary { dst, op, src } => {
            if matches!(op, UnOp::NegF | UnOp::IntToFloat) {
                return false;
            }
            let a = fe.use_op(src);
            let d = fe.def_op(dst);
            if *op == UnOp::Neg {
                fe.emit("subu", &[&d, &Reg::Zero, &a]);
            } else {
                fe.emit("xori", &[&d, &a, &1]);
            }
            fe.finish_def(dst, d);
        }
        Instr::Label(l) => {
            fe.alloc.block_boundary(&mut fe.out);
            fe.label(l);
        }
        Instr::Goto(l) => {
            fe.alloc.block_boundary(&mut fe.out);
            fe.emit("j", &[l]);
        }
        Instr::IfTrue { cond, label } | Instr::IfFalse { cond, label } => {
            let c = fe.use_op(cond);
            fe.alloc.block_boundary(&mut fe.out);
            fe.emit(if matches!(instr, Instr::IfTrue { .. }) { "bnez" } else { "beqz" }, &[&c, label]);
        }
        Instr::IfRel { left, op, right, label } => {
            let branch = match op {
                BinOp::Eq => "beq",
                BinOp::Ne => "bne",
                BinOp::Lt => "blt",
                BinOp::Le => "ble",
                BinOp::Gt => "bgt",
                BinOp::Ge => "bge",
                _ => return false, // streq/strne: de B
            };
            let a = fe.use_op(left);
            let b = fe.use_op(right);
            fe.alloc.block_boundary(&mut fe.out);
            fe.emit(branch, &[&a, &b, label]);
        }
        Instr::Load { dst, base, index } => {
            let b = fe.use_op(base);
            let (reg, off) = direccion(fe, b, index);
            let d = fe.def_op(dst);
            fe.emit("lw", &[&d, &format!("{off}({reg})")]);
            fe.finish_def(dst, d);
        }
        Instr::Store { base, index, src } => {
            let b = fe.use_op(base);
            let (reg, off) = direccion(fe, b, index);
            let s = fe.use_op(src);
            fe.emit("sw", &[&s, &format!("{off}({reg})")]);
        }
        Instr::AddrOf { dst, name } => {
            let d = fe.def_op(dst);
            fe.emit("la", &[&d, &vtable_label(name)]);
            fe.finish_def(dst, d);
        }
        _ => return false,
    }
    true
}

/// `x = y`: constante → `li`/`la` directo al destino; si no, `move`.
fn copy(fe: &mut FunctionEmitter, dst: &Operand, src: &Operand) {
    let d;
    if matches!(src, Operand::Int(_) | Operand::Bool(_) | Operand::Null | Operand::Str(_) | Operand::Addr(_)) {
        d = fe.def_op(dst);
        load_const(&mut fe.out, &fe.strings, src, d);
    } else {
        let s = fe.use_op(src);
        d = fe.def_op(dst);
        if s != d {
            fe.emit("move", &[&d, &s]);
        }
    }
    fe.finish_def(dst, d);
}

/// Una constante entera que cabe en el inmediato de 16 bits con signo.
fn inmediato(op: &Operand) -> Option<i64> {
    match op {
        Operand::Int(n) if (-32768..=32767).contains(n) => Some(*n),
        _ => None,
    }
}

fn aritmetica(fe: &mut FunctionEmitter, dst: &Operand, op: BinOp, left: &Operand, right: &Operand) {
    let a = fe.use_op(left);
    // `a - n` es `addiu a, -n`; -(-32768) ya no cabe.
    let imm = inmediato(right).map(|n| if op == BinOp::Sub { -n } else { n }).filter(|n| (-32768..=32767).contains(n));
    let d;
    if let (Some(n), BinOp::Add | BinOp::Sub) = (imm, op) {
        d = fe.def_op(dst);
        fe.emit("addiu", &[&d, &a, &n]);
    } else {
        let b = fe.use_op(right);
        d = fe.def_op(dst);
        fe.emit(if op == BinOp::Add { "addu" } else if op == BinOp::Sub { "subu" } else { "mul" }, &[&d, &a, &b]);
    }
    fe.finish_def(dst, d);
}

fn division(fe: &mut FunctionEmitter, dst: &Operand, op: BinOp, left: &Operand, right: &Operand) {
    let a = fe.use_op(left);
    let b = fe.use_op(right);
    let d = fe.def_op(dst);
    fe.emit("div", &[&a, &b]);
    fe.emit(if op == BinOp::Div { "mflo" } else { "mfhi" }, &[&d]);
    fe.finish_def(dst, d);
}

fn relacional(fe: &mut FunctionEmitter, dst: &Operand, op: BinOp, left: &Operand, right: &Operand) {
    let a = fe.use_op(left);
    let b = fe.use_op(right);
    let d = fe.def_op(dst);
    let mnemonic = match op {
        BinOp::Eq => "seq",
        BinOp::Ne => "sne",
        BinOp::Lt => "slt",
        BinOp::Le => "sle",
        BinOp::Gt => "sgt",
        _ => "sge",
    };
    fe.emit(mnemonic, &[&d, &a, &b]);
    fe.finish_def(dst, d);
}

/// El registro base y el desplazamiento de `base[index]`: con índice
/// constante es `(base, k)`; si no, suma `base + index` en `$t8`.
fn direccion(fe: &mut FunctionEmitter, base: Reg, index: &Operand) -> (Reg, i64) {
    match inmediato(index) {
        Some(k) => (base, k),
        None => {
            let i = fe.use_op(index);
            fe.emit("addu", &[&AUX_DIR, &base, &i]);
            (AUX_DIR, 0)
        }
    }
}

