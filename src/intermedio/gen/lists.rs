//! Listas — Fase 2 (integrante B).
//!
//! Pendiente (Fase 2):
//! - `try_gen_list_expr`: literal (`alloc` + longitud + elementos) y acceso
//!   indexado (`check_bounds` + `x = a[i]`).
//! - `try_gen_list_assign`: asignación a un elemento.

use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::Operand;
use super::Generator;

impl<'a> Generator<'a> {
    pub(crate) fn try_gen_list_expr(&mut self, _node: &ParseNode) -> Option<Operand> {
        None
    }

    pub(crate) fn try_gen_list_assign(&mut self, _node: &ParseNode) -> bool {
        false
    }
}
