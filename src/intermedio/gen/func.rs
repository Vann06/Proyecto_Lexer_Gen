//! Funciones y llamadas — Fase 2 (integrante B).
//!
//! Pendiente (Fase 2):
//! - `gen_function`: una `TacFunction` propia por función/método (etiqueta,
//!   nivel, marco), con `self.suspended` para las anidadas.
//! - `try_gen_call`: `param` (con su conversión), `link` (siguiendo
//!   `nc - ng + 1` enlaces de acceso), `call`.
//! - `try_gen_return`: `return` con su conversión.

use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::Operand;
use super::Generator;

impl<'a> Generator<'a> {
    /// Una declaración de función o método.
    pub(crate) fn gen_function(&mut self, node: &ParseNode) {
        self.unsupported(node, "función");
    }

    /// Una llamada usada como expresión.
    pub(crate) fn try_gen_call(&mut self, _node: &ParseNode) -> Option<Operand> {
        None
    }

    /// Una sentencia `return`.
    pub(crate) fn try_gen_return(&mut self, _node: &ParseNode) -> bool {
        false
    }
}
