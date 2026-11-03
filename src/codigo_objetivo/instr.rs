//! Traducción de las instrucciones de expresiones, control de flujo, listas y
//! campos (`PLAN_MIPS.md` §6, Fase 1).

use crate::intermedio::tac::Instr;

use super::FunctionEmitter;

/// Traduce `instr`. Devuelve `false` si no es de esta parte.
pub fn translate(_fe: &mut FunctionEmitter, _instr: &Instr) -> bool {
    false
}
