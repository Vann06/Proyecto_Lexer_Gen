//! Traducción de sentencias — Fase 1 (integrante A).
//!
//! Punto de entrada: `try_gen_statement`. Devuelve `true` si tradujo `node`
//! (y entonces el recorrido genérico no baja a sus hijos). Cada forma se
//! reconoce por lo que declara el `.yalp`, nunca por el nombre de la
//! producción:
//!
//! | Sentencia | Directiva |
//! |---|---|
//! | Declaración con o sin valor inicial | `%declare … variable` + `%init_of` |
//! | Asignación a una variable | `%assign` |
//! | `print` | `%print` |
//! | `if`, `while`, `do_while`, `for`, `foreach`, `switch`, `try` | `%flow` |
//! | `break`, `continue` | `%break`, `%continue` |
//!
//! Las formas de la Fase 2 (`return`, asignación a un campo o a un índice)
//! se delegan a `func.rs`, `objects.rs` y `lists.rs`.

use crate::semantico::analyzer::{find_child_index, find_identifier_child};
use crate::semantico::bindings::SymbolRef;
use crate::semantico::collections;
use crate::semantico::symbols::SymbolKind;
use crate::semantico::types::Type;
use crate::sintactico::gramatica::flow::{FlowKind, FlowRole};
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::{BinOp, Instr, Operand};
use super::{Generator, JumpTargets, LIST_HEADER};

impl<'a> Generator<'a> {
    pub(crate) fn try_gen_statement(&mut self, node: &ParseNode) -> bool {
        if self.try_gen_return(node) || self.try_gen_object_assign(node) || self.try_gen_list_assign(node) {
            return true;
        }
        if let Some(value) = self.ispec.print_value(node) {
            let v = self.gen_expr(value);
            self.emit(Instr::Print(v.clone()));
            self.release(&v);
            return true;
        }
        if self.try_gen_declaration(node) || self.try_gen_assignment(node) {
            return true;
        }
        if let Some(shape) = self.ispec.flow_of(node) {
            return match shape.kind() {
                FlowKind::If => self.gen_if(node),
                FlowKind::While => self.gen_while(node),
                FlowKind::DoWhile => self.gen_do_while(node),
                FlowKind::For => self.gen_for(node),
                FlowKind::Foreach => self.gen_foreach(node),
                FlowKind::Switch => self.gen_switch(node),
                FlowKind::Try => self.gen_try(node),
                // `case`/`default`/`catch` los traduce su construcción.
                FlowKind::Case | FlowKind::Default | FlowKind::Catch => false,
            };
        }
        if self.sspec.flow.breaks.contains(&node.symbol) {
            self.gen_jump(node, false);
            return true;
        }
        if self.sspec.flow.continues.contains(&node.symbol) {
            self.gen_jump(node, true);
            return true;
        }
        false
    }

    // ─────────────────────── declaraciones y asignaciones ───────────────────────

    /// `let x: T = e;` / `const K = e;` → `x = <e>`. Sin valor inicial, la
    /// variable se inicializa con el de su tipo (`0`, `0.0`, `false`, `null`)
    /// para que su valor quede definido.
    fn try_gen_declaration(&mut self, node: &ParseNode) -> bool {
        let Some(rule) = self
            .sspec
            .declarations
            .iter()
            .find(|r| r.production == node.symbol && r.kind == SymbolKind::Variable)
        else {
            return false;
        };
        let Some((_, name_leaf)) = find_identifier_child(node, &self.sspec.identifier_token, rule.name_child) else {
            return false;
        };
        let Some(r) = self.analysis.bindings.get(name_leaf).cloned() else {
            self.unsupported(node, "una declaración sin símbolo");
            return true;
        };
        let init = rule.init_child.as_ref().and_then(|loc| find_child_index(node, loc)).map(|i| &node.children[i]);
        match init {
            Some(value) => self.assign_to(&r, value),
            None => {
                let default = default_value(self.analysis.resolve(&r).and_then(|s| s.ty.as_ref()));
                match self.var_operand(&r) {
                    Some(dst) => {
                        self.emit(Instr::Copy { dst: dst.clone(), src: default });
                        self.release(&dst);
                    }
                    None => {
                        self.unsupported(node, "una declaración sin lugar en memoria");
                    }
                }
            }
        }
        true
    }

    /// `x = e;` (`%assign`, cuando el destino es un identificador).
    fn try_gen_assignment(&mut self, node: &ParseNode) -> bool {
        let Some(rule) = &self.sspec.assign else { return false };
        if node.symbol != rule.production {
            return false;
        }
        let target = node.children.get(rule.target_index);
        let value = node.children.get(rule.value_index);
        let (Some(target), Some(value)) = (target, value) else { return false };
        if !target.children.is_empty() || target.symbol != self.sspec.identifier_token {
            return false;
        }
        let Some(r) = self.analysis.bindings.get(target).cloned() else {
            self.unsupported(node, "una asignación sin símbolo");
            return true;
        };
        self.assign_to(&r, value);
        true
    }

    /// Guarda el valor de `value` en la variable `r`, con su conversión si
    /// hace falta. El valor se calcula ANTES que la dirección del destino:
    /// así, si el destino necesita temporales (una variable de otra función),
    /// no quedan ocupados mientras se calcula el valor.
    pub(crate) fn assign_to(&mut self, r: &SymbolRef, value: &ParseNode) {
        let v = self.gen_widened(value);
        let Some(dst) = self.var_operand(r) else {
            self.unsupported(value, "un destino sin lugar en memoria");
            return;
        };
        if self.retarget_last(&v, &dst) {
            self.release(&v);
            return;
        }
        self.emit(Instr::Copy { dst: dst.clone(), src: v.clone() });
        self.release(&v);
        self.release(&dst);
    }

    // ─────────────────────────────── control de flujo ───────────────────────────────

    /// El hijo de `node` que cumple `role`, según su `%flow`.
    fn part<'n>(&self, node: &'n ParseNode, role: FlowRole) -> Option<&'n ParseNode> {
        self.ispec.flow_of(node)?.child(node, role)
    }

    /// ```text
    ///     ifFalse c goto Lelse          ifFalse c goto Lfin
    ///     <then>                        <then>
    ///     goto Lfin                 Lfin:
    /// Lelse:
    ///     <else>
    /// Lfin:
    /// ```
    fn gen_if(&mut self, node: &ParseNode) -> bool {
        let (Some(cond), Some(then)) = (self.part(node, FlowRole::Cond), self.part(node, FlowRole::Then)) else {
            self.unsupported(node, "un `if` incompleto");
            return true;
        };
        let end = self.new_label();
        match self.part(node, FlowRole::Else) {
            Some(otherwise) => {
                let else_label = self.new_label();
                self.gen_cond(cond, None, Some(&else_label));
                self.gen_stmt(then);
                self.emit(Instr::Goto(end.clone()));
                self.emit(Instr::Label(else_label));
                self.gen_stmt(otherwise);
            }
            None => {
                self.gen_cond(cond, None, Some(&end));
                self.gen_stmt(then);
            }
        }
        self.emit(Instr::Label(end));
        true
    }

    /// ```text
    /// Lcond:
    ///     ifFalse c goto Lfin
    ///     <cuerpo>              ; continue → Lcond, break → Lfin
    ///     goto Lcond
    /// Lfin:
    /// ```
    fn gen_while(&mut self, node: &ParseNode) -> bool {
        let (Some(cond), Some(body)) = (self.part(node, FlowRole::Cond), self.part(node, FlowRole::Body)) else {
            self.unsupported(node, "un `while` incompleto");
            return true;
        };
        let start = self.new_label();
        let end = self.new_label();
        self.emit(Instr::Label(start.clone()));
        self.gen_cond(cond, None, Some(&end));
        self.gen_loop_body(body, &end, &start);
        self.emit(Instr::Goto(start));
        self.emit(Instr::Label(end));
        true
    }

    /// ```text
    /// Lcuerpo:
    ///     <cuerpo>              ; continue → Lcond, break → Lfin
    /// Lcond:
    ///     if c goto Lcuerpo
    /// Lfin:
    /// ```
    fn gen_do_while(&mut self, node: &ParseNode) -> bool {
        let (Some(body), Some(cond)) = (self.part(node, FlowRole::Body), self.part(node, FlowRole::Cond)) else {
            self.unsupported(node, "un `do-while` incompleto");
            return true;
        };
        let start = self.new_label();
        let check = self.new_label();
        let end = self.new_label();
        self.emit(Instr::Label(start.clone()));
        self.gen_loop_body(body, &end, &check);
        self.emit(Instr::Label(check));
        self.gen_cond(cond, Some(&start), None);
        self.emit(Instr::Label(end));
        true
    }

    /// ```text
    ///     <init>
    /// Lcond:
    ///     ifFalse c goto Lfin
    ///     <cuerpo>              ; continue → Lpaso, break → Lfin
    /// Lpaso:
    ///     <update>
    ///     goto Lcond
    /// Lfin:
    /// ```
    fn gen_for(&mut self, node: &ParseNode) -> bool {
        let (Some(cond), Some(body)) = (self.part(node, FlowRole::Cond), self.part(node, FlowRole::Body)) else {
            self.unsupported(node, "un `for` incompleto");
            return true;
        };
        if let Some(init) = self.part(node, FlowRole::Init) {
            self.gen_stmt(init);
        }
        let start = self.new_label();
        let step = self.new_label();
        let end = self.new_label();
        self.emit(Instr::Label(start.clone()));
        self.gen_cond(cond, None, Some(&end));
        self.gen_loop_body(body, &end, &step);
        self.emit(Instr::Label(step));
        if let Some(update) = self.part(node, FlowRole::Update) {
            self.gen_stmt(update);
        }
        self.emit(Instr::Goto(start));
        self.emit(Instr::Label(end));
        true
    }

    /// Recorre la lista por índice. Una lista en el heap es
    /// `[longitud][e0][e1]…` (ver `LIST_HEADER`):
    ///
    /// ```text
    ///     tL = <lista>
    ///     tN = tL[0]                ; longitud
    ///     tI = 0
    /// Lcond:
    ///     if tI >= tN goto Lfin
    ///     tK = tI * <ancho>
    ///     tK = tK + 4
    ///     item = tL[tK]
    ///     <cuerpo>                  ; continue → Lpaso, break → Lfin
    /// Lpaso:
    ///     tI = tI + 1
    ///     goto Lcond
    /// Lfin:
    /// ```
    fn gen_foreach(&mut self, node: &ParseNode) -> bool {
        let (Some(var), Some(iter), Some(body)) =
            (self.part(node, FlowRole::Var), self.part(node, FlowRole::Iter), self.part(node, FlowRole::Body))
        else {
            self.unsupported(node, "un `foreach` incompleto");
            return true;
        };
        let slot = self
            .type_of(iter)
            .and_then(collections::element_type)
            .and_then(|elem| self.list_slot(&elem));
        let (Some(slot), Some(r)) = (slot, self.analysis.bindings.get(var).cloned()) else {
            self.unsupported(node, "un `foreach` sobre algo que no es una lista");
            return true;
        };

        // La lista se evalúa una sola vez, y su dirección queda fija en un
        // temporal aunque el cuerpo reasigne la variable.
        let value = self.gen_expr(iter);
        let list = match value {
            Operand::Temp(_) => value,
            other => {
                self.release(&other);
                let t = self.new_temp();
                self.emit(Instr::Copy { dst: t.clone(), src: other });
                t
            }
        };
        let len = self.new_temp();
        self.emit(Instr::Load { dst: len.clone(), base: list.clone(), index: Operand::Int(0) });
        let index = self.new_temp();
        self.emit(Instr::Copy { dst: index.clone(), src: Operand::Int(0) });

        let start = self.new_label();
        let step = self.new_label();
        let end = self.new_label();
        self.emit(Instr::Label(start.clone()));
        self.emit(Instr::IfRel { left: index.clone(), op: BinOp::Ge, right: len.clone(), label: end.clone() });
        let offset = self.new_temp();
        self.emit(Instr::Binary { dst: offset.clone(), op: BinOp::Mul, left: index.clone(), right: Operand::Int(slot as i64) });
        self.emit(Instr::Binary { dst: offset.clone(), op: BinOp::Add, left: offset.clone(), right: Operand::Int(LIST_HEADER as i64) });
        match self.var_operand(&r) {
            Some(item) => {
                self.emit(Instr::Load { dst: item.clone(), base: list.clone(), index: offset.clone() });
                self.release(&item);
            }
            None => {
                self.unsupported(var, "la variable de un `foreach` sin lugar en memoria");
            }
        }
        self.release(&offset);
        self.gen_loop_body(body, &end, &step);
        self.emit(Instr::Label(step));
        self.emit(Instr::Binary { dst: index.clone(), op: BinOp::Add, left: index.clone(), right: Operand::Int(1) });
        self.emit(Instr::Goto(start));
        self.emit(Instr::Label(end));
        self.release(&index);
        self.release(&len);
        self.release(&list);
        true
    }

    /// El discriminante se evalúa una vez; cada `case` se compara en orden y
    /// salta a su cuerpo. Los cuerpos van seguidos, así que sin `break` se
    /// cae al siguiente (como en TypeScript):
    ///
    /// ```text
    ///     t = <d>
    ///     if t == v1 goto Lcaso1
    ///     if t == v2 goto Lcaso2
    ///     goto Ldefault             ; o Lfin si no hay default
    /// Lcaso1:
    ///     <cuerpo 1>                ; break → Lfin
    /// Lcaso2:
    ///     <cuerpo 2>
    /// Ldefault:
    ///     <cuerpo default>
    /// Lfin:
    /// ```
    fn gen_switch(&mut self, node: &ParseNode) -> bool {
        let (Some(disc), Some(cases)) = (self.part(node, FlowRole::Disc), self.part(node, FlowRole::Cases)) else {
            // Un `switch` sin casos: solo se evalúa el discriminante.
            if let Some(disc) = self.part(node, FlowRole::Disc) {
                let v = self.gen_expr(disc);
                self.release(&v);
            }
            return true;
        };
        let value = self.gen_expr(disc);
        let subject = match value {
            Operand::Temp(_) => value,
            other => {
                self.release(&other);
                let t = self.new_temp();
                self.emit(Instr::Copy { dst: t.clone(), src: other });
                t
            }
        };

        let mut members = Vec::new();
        self.collect_cases(cases, &mut members);
        let end = self.new_label();
        let labels: Vec<String> = members.iter().map(|_| self.new_label()).collect();

        let mut default_label = None;
        for (member, label) in members.iter().zip(&labels) {
            match self.ispec.flow_of(member).map(|s| s.kind()) {
                Some(FlowKind::Case) => {
                    let Some(case_value) = self.part(member, FlowRole::Value) else { continue };
                    let v = self.gen_widened(case_value);
                    self.emit(Instr::IfRel { left: subject.clone(), op: BinOp::Eq, right: v.clone(), label: label.clone() });
                    self.release(&v);
                }
                _ => default_label = Some(label.clone()),
            }
        }
        self.emit(Instr::Goto(default_label.unwrap_or_else(|| end.clone())));
        self.release(&subject);

        self.jumps.push(JumpTargets { break_label: end.clone(), continue_label: None, try_depth: self.try_depth });
        for (member, label) in members.iter().zip(labels) {
            self.emit(Instr::Label(label));
            if let Some(body) = self.part(member, FlowRole::Body) {
                self.gen_stmt(body);
            }
        }
        self.jumps.pop();
        self.emit(Instr::Label(end));
        true
    }

    /// Los `case`/`default` de un `switch`, en orden, sin bajar a sus cuerpos.
    fn collect_cases<'n>(&self, node: &'n ParseNode, out: &mut Vec<&'n ParseNode>) {
        match self.ispec.flow_of(node).map(|s| s.kind()) {
            Some(FlowKind::Case) | Some(FlowKind::Default) => out.push(node),
            _ => {
                for child in &node.children {
                    self.collect_cases(child, out);
                }
            }
        }
    }

    /// ```text
    ///     try_begin Lcatch
    ///     <cuerpo>
    ///     try_end
    ///     goto Lfin
    /// Lcatch:
    ///     err = exception
    ///     <cuerpo del catch>
    /// Lfin:
    /// ```
    fn gen_try(&mut self, node: &ParseNode) -> bool {
        let (Some(body), Some(handler)) = (self.part(node, FlowRole::Body), self.part(node, FlowRole::Handler)) else {
            self.unsupported(node, "un `try` incompleto");
            return true;
        };
        let catch_label = self.new_label();
        let end = self.new_label();
        self.emit(Instr::TryBegin(catch_label.clone()));
        self.try_depth += 1;
        self.gen_stmt(body);
        self.try_depth -= 1;
        self.emit(Instr::TryEnd);
        self.emit(Instr::Goto(end.clone()));
        self.emit(Instr::Label(catch_label));

        let var = self.part(handler, FlowRole::Var);
        let leaf = var.and_then(|v| match v.children.is_empty() {
            true => Some(v),
            false => find_identifier_child(v, &self.sspec.identifier_token, None).map(|(_, leaf)| leaf),
        });
        match leaf.and_then(|l| self.analysis.bindings.get(l).cloned()).and_then(|r| self.var_operand(&r)) {
            Some(dst) => {
                self.emit(Instr::GetException(dst.clone()));
                self.release(&dst);
            }
            None => {
                self.unsupported(handler, "la variable de un `catch`");
            }
        }
        if let Some(handler_body) = self.part(handler, FlowRole::Body) {
            self.gen_stmt(handler_body);
        }
        self.emit(Instr::Label(end));
        true
    }

    /// El cuerpo de un bucle, con sus destinos de `break` y `continue`.
    fn gen_loop_body(&mut self, body: &ParseNode, break_label: &str, continue_label: &str) {
        self.jumps.push(JumpTargets {
            break_label: break_label.to_string(),
            continue_label: Some(continue_label.to_string()),
            try_depth: self.try_depth,
        });
        self.gen_stmt(body);
        self.jumps.pop();
    }

    /// `break` / `continue`: salta a la etiqueta de la construcción más
    /// interna que lo admite. Si el salto sale de uno o más `try`, primero
    /// cierra sus manejadores.
    fn gen_jump(&mut self, node: &ParseNode, is_continue: bool) {
        let target = if is_continue {
            self.jumps.iter().rev().find_map(|j| j.continue_label.clone().map(|l| (l, j.try_depth)))
        } else {
            self.jumps.last().map(|j| (j.break_label.clone(), j.try_depth))
        };
        let Some((label, depth)) = target else {
            self.unsupported(node, if is_continue { "un `continue` fuera de un bucle" } else { "un `break` fuera de un bucle" });
            return;
        };
        for _ in depth..self.try_depth {
            self.emit(Instr::TryEnd);
        }
        self.emit(Instr::Goto(label));
    }
}

/// El valor inicial de una variable declarada sin valor.
fn default_value(ty: Option<&Type>) -> Operand {
    match ty {
        Some(Type::Int) => Operand::Int(0),
        Some(Type::Float) => Operand::Float(0.0),
        Some(Type::Bool) => Operand::Bool(false),
        _ => Operand::Null,
    }
}
