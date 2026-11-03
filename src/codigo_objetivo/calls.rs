//! Secuencias de llamada y retorno (`PLAN_MIPS.md`, Fase 2).
//!
//! Los `param*`, el `link` opcional y el `call`/`callv` se traducen como un
//! solo grupo: los operandos no se dejan pendientes entre instrucciones y el
//! asignador recibe un único punto `before_call`/`after_call`.

use crate::intermedio::tac::{Base, Instr, Operand};

use super::asm::Reg;
use super::names::{func_label, ret_label};
use super::FunctionEmitter;

/// Traduce una instrucción de retorno. Las llamadas se manejan con
/// [`translate_call_group`] porque necesitan ver los `param` anteriores.
pub fn translate(fe: &mut FunctionEmitter, instr: &Instr, return_label: &str) -> bool {
    let Instr::Return(value) = instr else {
        return false;
    };

    if let Some(value) = value {
        let reg = fe.use_op(value);
        fe.emit("sw", &[&reg, &format!("4({})", Reg::Fp)]);
    }
    fe.emit("j", &[&return_label]);
    fe.alloc.block_boundary(&mut fe.out);
    true
}

/// Traduce un grupo contiguo `param*`, `link?`, `call|callv`.
///
/// `call_index` es el índice de la llamada (no el del primer `param`), para
/// que el asignador de la Fase 4 calcule la vida justo después del salto.
pub fn translate_call_group(fe: &mut FunctionEmitter, group: &[Instr], call_index: usize) -> bool {
    let Some(call) = group.last() else {
        return false;
    };
    let (dst, nargs) = match call {
        Instr::Call { dst, nargs, .. } | Instr::CallV { dst, nargs, .. } => (dst.as_ref(), *nargs),
        _ => return false,
    };

    let mut params = Vec::new();
    let mut link = None;
    for instr in &group[..group.len() - 1] {
        match instr {
            Instr::Param(value) if link.is_none() => params.push(value),
            Instr::Link(value) if link.is_none() => link = Some(value),
            _ => return false,
        }
    }
    if params.len() != nargs {
        return false;
    }

    fe.alloc.begin_instr(call_index);
    fe.alloc.before_call(&mut fe.out);

    let call_frame = 16 + 4 * nargs;
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &-(call_frame as i64)]);

    for (i, value) in params.iter().enumerate() {
        fe.comment(format!("param {value}"));
        let reg = fe.use_op(value);
        fe.emit("sw", &[&reg, &format!("{}({})", 16 + 4 * i, Reg::Sp)]);
    }

    if let Some(value) = link {
        fe.comment(format!("link {value}"));
        // El TAC escribe el valor especial `fp` con la forma de un operando
        // direccionable. Aquí significa el puntero de marco actual, no la
        // palabra guardada en 0($fp) (que es el enlace de control).
        let reg = if is_frame_pointer(value) {
            Reg::Fp
        } else {
            fe.use_op(value)
        };
        fe.emit("sw", &[&reg, &format!("12({})", Reg::Sp)]);
    }
    fe.emit("sw", &[&Reg::Fp, &format!("0({})", Reg::Sp)]);

    fe.comment(call.to_string());
    match call {
        Instr::Call { func, .. } => fe.emit("jal", &[&func_label(func)]),
        Instr::CallV { target, .. } => {
            let reg = fe.use_op(target);
            fe.emit("jalr", &[&reg]);
        }
        _ => unreachable!(),
    }

    if let Some(dst) = dst {
        let reg = fe.def_op(dst);
        fe.emit("lw", &[&reg, &format!("4({})", Reg::Sp)]);
        fe.finish_def(dst, reg);
    }
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &(call_frame as i64)]);
    fe.alloc.after_call();
    fe.alloc.end_instr(&mut fe.out);
    true
}

/// Variante necesaria para el TAC de despacho virtual actual: después de
/// emitir `param this`, calcula el método con uno o más `load` antes del
/// `callv`. El argumento se copia al marco del llamado antes de que esos
/// temporales puedan reutilizarse.
pub fn translate_virtual_group(
    fe: &mut FunctionEmitter,
    group: &[Instr],
    first_index: usize,
) -> bool {
    let Some(call @ Instr::CallV { dst, target, nargs }) = group.last() else {
        return false;
    };
    if group[..group.len() - 1]
        .iter()
        .filter(|i| matches!(i, Instr::Param(_)))
        .count()
        != *nargs
    {
        return false;
    }

    let call_frame = 16 + 4 * nargs;
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &-(call_frame as i64)]);
    let mut param_index = 0;

    for (offset, instr) in group[..group.len() - 1].iter().enumerate() {
        fe.alloc.begin_instr(first_index + offset);
        if !matches!(instr, Instr::Label(_)) {
            fe.comment(instr.to_string());
        }
        match instr {
            Instr::Param(value) => {
                let reg = fe.use_op(value);
                fe.emit(
                    "sw",
                    &[&reg, &format!("{}({})", 16 + 4 * param_index, Reg::Sp)],
                );
                param_index += 1;
            }
            Instr::Link(value) => {
                let reg = if is_frame_pointer(value) {
                    Reg::Fp
                } else {
                    fe.use_op(value)
                };
                fe.emit("sw", &[&reg, &format!("12({})", Reg::Sp)]);
            }
            other if super::instr::translate(fe, other) => {}
            _ => return false,
        }
        fe.alloc.end_instr(&mut fe.out);
    }

    let call_index = first_index + group.len() - 1;
    fe.alloc.begin_instr(call_index);
    fe.alloc.before_call(&mut fe.out);
    fe.emit("sw", &[&Reg::Fp, &format!("0({})", Reg::Sp)]);
    fe.comment(call.to_string());
    let target_reg = fe.use_op(target);
    fe.emit("jalr", &[&target_reg]);
    if let Some(dst) = dst {
        let reg = fe.def_op(dst);
        fe.emit("lw", &[&reg, &format!("4({})", Reg::Sp)]);
        fe.finish_def(dst, reg);
    }
    fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &(call_frame as i64)]);
    fe.alloc.after_call();
    fe.alloc.end_instr(&mut fe.out);
    true
}

fn is_frame_pointer(op: &Operand) -> bool {
    matches!(op, Operand::Var { name, base: Base::Fp, offset: 0 } if name == "fp")
}

/// Emite el prólogo y epílogo definitivos después de conocer los `$s` usados.
pub fn wrap_function(fe: &mut FunctionEmitter, f: &crate::intermedio::tac::TacFunction) {
    let saved = fe.alloc.saved_registers();
    let total = f.total_frame_size();
    let mips_frame = total + 4 * saved.len();
    let mut body = std::mem::take(&mut fe.out);

    fe.label(&super::names::label_of(f));
    fe.emit("move", &[&Reg::Fp, &Reg::Sp]);
    if f.level != 0 {
        fe.emit("sw", &[&Reg::Ra, &format!("-4({})", Reg::Fp)]);
    }
    if mips_frame > 0 {
        fe.emit("addiu", &[&Reg::Sp, &Reg::Sp, &-(mips_frame as i64)]);
    }
    if f.level != 0 {
        for (i, reg) in saved.iter().enumerate() {
            fe.emit(
                "sw",
                &[reg, &format!("-{}({})", total + 4 * (i + 1), Reg::Fp)],
            );
        }
    }
    fe.out.append(&mut body);

    if f.level == 0 {
        if f.body.iter().any(|i| matches!(i, Instr::Return(_))) {
            fe.label(&ret_label(f));
        }
        fe.emit("li", &[&Reg::V0, &10]);
        fe.emit("syscall", &[]);
        return;
    }

    fe.label(&ret_label(f));
    for (i, reg) in saved.iter().enumerate().rev() {
        fe.emit(
            "lw",
            &[reg, &format!("-{}({})", total + 4 * (i + 1), Reg::Fp)],
        );
    }
    fe.emit("lw", &[&Reg::Ra, &format!("-4({})", Reg::Fp)]);
    fe.emit("move", &[&Reg::Sp, &Reg::Fp]);
    fe.emit("lw", &[&Reg::Fp, &format!("0({})", Reg::Fp)]);
    fe.emit("jr", &[&Reg::Ra]);
}
