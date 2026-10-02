//! Clases y objetos — Fase 2 (integrante B).
//!
//! Pendiente (Fase 2):
//! - `gen_class`: sus métodos (vía `gen_function`) y su vtable como dato.
//! - `try_gen_object_expr`: `new C(args)`, lectura de un campo, `this`,
//!   llamada a un método con despacho virtual (`callv`).
//! - `try_gen_object_assign`: asignación a un campo (`obj.campo = valor`).

use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::Operand;
use super::Generator;

impl<'a> Generator<'a> {
    /// Una declaración de clase o struct.
    pub(crate) fn gen_class(&mut self, node: &ParseNode) {
        self.unsupported(node, "clase");
    }

    pub(crate) fn try_gen_object_expr(&mut self, _node: &ParseNode) -> Option<Operand> {
        None
    }

    pub(crate) fn try_gen_object_assign(&mut self, _node: &ParseNode) -> bool {
        false
    }
}
