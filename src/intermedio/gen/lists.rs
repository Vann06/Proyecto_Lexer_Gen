//! Listas — Fase 2 (integrante B).
//!
//! Una lista en el heap es `[longitud][e0][e1]…` (ver `LIST_HEADER`), cada
//! elemento redondeado a la palabra: el mismo formato que recorre `foreach`.
//!
//! - Literal: `alloc`, la longitud y un `store` por elemento.
//! - Acceso `a[i]`: `check_bounds` y luego la lectura en
//!   `LIST_HEADER + i × ancho`.
//! - La asignación a un elemento (`a[i] = x`) NO existe en Compiscript: la
//!   gramática solo asigna a `ID` o a `primary . ID`.

use crate::semantico::collections::{find_array_literal, find_index_access, flatten_array_elements};
use crate::semantico::types::Type;
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::{BinOp, Instr, Operand};
use super::{Generator, LIST_HEADER};

impl<'a> Generator<'a> {
    pub(crate) fn try_gen_list_expr(&mut self, node: &ParseNode) -> Option<Operand> {
        if let Some(elements) = find_array_literal(node, self.sspec) {
            return Some(self.gen_list_literal(node, &flatten_array_elements(elements, self.sspec)));
        }
        if let Some((base, index)) = find_index_access(node, self.sspec) {
            return Some(self.gen_list_index(node, base, index));
        }
        None
    }

    /// ```text
    ///     t = alloc <4 + n × ancho>
    ///     t[0] = n
    ///     t[4] = <e0>               ; t[4 + i × ancho] = <ei>
    /// ```
    fn gen_list_literal(&mut self, node: &ParseNode, elements: &[&ParseNode]) -> Operand {
        // Una lista vacía no tiene tipo de elemento, pero tampoco lo necesita.
        let slot = match self.type_of(node) {
            Some(Type::Array(elem)) => self.list_slot(elem),
            _ if elements.is_empty() => Some(0),
            _ => None,
        };
        let Some(slot) = slot else {
            return self.unsupported(node, "una lista cuyo tipo de elemento no se conoce");
        };
        let list = self.new_temp();
        let size = LIST_HEADER + (elements.len() * slot) as isize;
        self.emit(Instr::Alloc { dst: list.clone(), size: Operand::Int(size as i64) });
        self.emit(Instr::Store { base: list.clone(), index: Operand::Int(0), src: Operand::Int(elements.len() as i64) });
        for (i, element) in elements.iter().enumerate() {
            let v = self.gen_widened(element);
            let offset = LIST_HEADER + (i * slot) as isize;
            self.emit(Instr::Store { base: list.clone(), index: Operand::Int(offset as i64), src: v.clone() });
            self.release(&v);
        }
        list
    }

    /// ```text
    ///     check_bounds a, i
    ///     t = i * <ancho>           ; con índice constante: sin estas dos
    ///     t = t + 4                 ; líneas, el offset ya viene sumado
    ///     x = a[t]
    /// ```
    fn gen_list_index(&mut self, node: &ParseNode, base: &ParseNode, index: &ParseNode) -> Operand {
        let slot = match self.type_of(base) {
            Some(Type::Array(elem)) => self.list_slot(elem),
            _ => None,
        };
        let Some(slot) = slot else {
            return self.unsupported(node, "un índice sobre algo que no es una lista");
        };
        let list = self.gen_expr(base);
        let list = self.as_value(list);
        let i = self.gen_expr(index);
        self.emit(Instr::CheckBounds { array: list.clone(), index: i.clone() });
        let offset = match i {
            Operand::Int(n) => Operand::Int(LIST_HEADER as i64 + n * slot as i64),
            i => {
                self.release(&i);
                let offset = self.new_temp();
                self.emit(Instr::Binary { dst: offset.clone(), op: BinOp::Mul, left: i, right: Operand::Int(slot as i64) });
                self.emit(Instr::Binary {
                    dst: offset.clone(),
                    op: BinOp::Add,
                    left: offset.clone(),
                    right: Operand::Int(LIST_HEADER as i64),
                });
                offset
            }
        };
        self.release(&list);
        self.release(&offset);
        let dst = self.new_temp();
        self.emit(Instr::Load { dst: dst.clone(), base: list, index: offset });
        dst
    }

    /// Asignar a un elemento (`a[i] = x`) no tiene traducción porque la
    /// gramática no tiene esa producción; no hay nada que reconocer.
    pub(crate) fn try_gen_list_assign(&mut self, _node: &ParseNode) -> bool {
        false
    }
}
