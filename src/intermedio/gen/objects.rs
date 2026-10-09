//! Clases y objetos — Fase 2 (integrante B).
//!
//! - Layout de un objeto: `[puntero a vtable][campos…]` (offsets de
//!   `storage::allocate_classes`). Un struct no lleva vtable.
//! - `gen_class`: los métodos (cada uno una función, ver `func.rs`), el
//!   inicializador de campos `<Clase>.$init` y la vtable como dato.
//! - `new C(args)`: `alloc`, vtable, `$init` y constructor.
//! - Campos: `obj.campo` es la dirección `t[offset]`. Métodos: despacho
//!   virtual (`t = obj[0]`, `t = t[ranura × 4]`, `callv`).
//! - Literal de struct `P { x: 1 }`: `alloc` y un `store` por campo.

use crate::semantico::analyzer::find_identifier_child;
use crate::semantico::classes::{find_member_access, find_struct_literal, flatten_arg_list, flatten_field_inits};
use crate::semantico::scopes::ScopeKind;
use crate::semantico::symbols::{Symbol, SymbolKind};
use crate::semantico::types::Type;
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::{Base, DataItem, Instr, Operand, TacFunction};
use super::Generator;

impl<'a> Generator<'a> {
    // ─────────────────────────────── herencia ───────────────────────────────

    /// La clase y sus ancestros, de la más derivada a la raíz.
    fn lineage(&self, class: &str) -> Vec<&'a Symbol> {
        let analysis = self.analysis;
        let mut out: Vec<&'a Symbol> = Vec::new();
        let mut next = Some(class.to_string());
        while let Some(name) = next {
            let Some(sym) = analysis.table.lookup_global(&name) else { break };
            if out.iter().any(|s| s.name == sym.name) {
                break;
            }
            next = sym.parent.clone();
            out.push(sym);
        }
        out
    }

    /// El miembro `name` de `class`, subiendo por la herencia, y la clase que
    /// lo define.
    pub(crate) fn find_member(&self, class: &str, name: &str) -> Option<(&'a Symbol, &'a Symbol)> {
        self.lineage(class)
            .into_iter()
            .find_map(|c| c.members.as_ref()?.iter().find(|m| m.name == name).map(|m| (m, c)))
    }

    /// `true` si la clase declara campos propios (y entonces tiene `$init`).
    fn has_init(class: &Symbol) -> bool {
        class.members.iter().flatten().any(|m| m.kind == SymbolKind::Variable)
    }

    /// La etiqueta del `$init` más cercano a `class`, ella incluida.
    fn nearest_init(&self, class: &str) -> Option<String> {
        self.lineage(class).into_iter().find(|c| Self::has_init(c)).map(|c| format!("{}.$init", c.name))
    }

    /// El nombre de la clase de `node`, según su tipo estático.
    pub(crate) fn class_of(&self, node: &ParseNode) -> Option<String> {
        match self.type_of(node)? {
            Type::Named(name) => Some(name.clone()),
            _ => None,
        }
    }

    /// Deja `value` en un temporal (para usarlo como base de una dirección).
    fn into_temp(&mut self, value: Operand) -> Operand {
        if matches!(value, Operand::Temp(_)) {
            return value;
        }
        self.release(&value);
        let t = self.new_temp();
        self.emit(Instr::Copy { dst: t.clone(), src: value });
        t
    }

    /// Como `into_temp`, pero solo si `value` es una dirección de memoria:
    /// una variable o un literal ya se pueden usar como base de `x[i]`.
    pub(crate) fn as_value(&mut self, value: Operand) -> Operand {
        match value {
            Operand::Mem { .. } => self.into_temp(value),
            other => other,
        }
    }

    // ───────────────────────────────── clases ─────────────────────────────────

    /// Una declaración de clase o struct. Una clase emite sus métodos, su
    /// `$init` y su vtable; un struct no emite nada (no tiene métodos ni
    /// vtable, y sus campos se escriben donde se usa el literal).
    pub(crate) fn gen_class(&mut self, node: &ParseNode) {
        if self.scope_kind_of(node) == Some(ScopeKind::Struct) {
            return;
        }
        let name = self
            .sspec
            .declarations
            .iter()
            .find(|r| r.production == node.symbol)
            .and_then(|r| find_identifier_child(node, &self.sspec.identifier_token, r.name_child))
            .and_then(|(_, leaf)| leaf.lexeme.clone());
        let analysis = self.analysis;
        let Some((name, class)) = name.and_then(|n| analysis.table.lookup_global(&n).map(|c| (n, c))) else {
            self.unsupported(node, "una clase que no está en el ámbito global");
            return;
        };

        let (mut methods, mut fields) = (Vec::new(), Vec::new());
        self.collect_members(node, &mut methods, &mut fields);

        let outer = self.class_ctx.replace(name.clone());
        if Self::has_init(class) {
            self.gen_init(&name, class, &fields);
        }
        for method in methods {
            self.gen_function(method);
        }
        self.class_ctx = outer;

        // Una entrada por método, en el orden de ranuras de la vtable; un
        // método sobrescrito conserva la ranura del padre y apunta a la
        // definición más derivada.
        let slots = analysis.layout.classes.vtable(&name).unwrap_or(&[]);
        let entries: Vec<String> =
            slots.iter().filter_map(|m| self.find_member(&name, m)?.0.label.clone()).collect();
        if entries.len() != slots.len() {
            self.unsupported(node, "una vtable con métodos sin etiqueta");
            return;
        }
        self.program.data.push(DataItem { name: format!("vt_{name}"), entries });
    }

    /// Separa el cuerpo de una clase en métodos y declaraciones de campos,
    /// sin bajar a ninguno de los dos.
    fn collect_members<'n>(&self, node: &'n ParseNode, methods: &mut Vec<&'n ParseNode>, fields: &mut Vec<&'n ParseNode>) {
        if self.scope_kind_of(node) == Some(ScopeKind::Function) {
            return methods.push(node);
        }
        if self.sspec.declarations.iter().any(|r| r.production == node.symbol && r.kind == SymbolKind::Variable) {
            return fields.push(node);
        }
        for child in &node.children {
            self.collect_members(child, methods, fields);
        }
    }

    /// `<Clase>.$init`: inicializa los campos de la clase (primero los del
    /// padre, si los tiene). Recibe `this` como cualquier método. Cada campo
    /// se traduce con el código de una declaración: dentro del cuerpo de la
    /// clase su acceso es `Field`, así que se escribe a través de `this`.
    fn gen_init(&mut self, name: &str, class: &Symbol, fields: &[&ParseNode]) {
        let frame = (-self.target.local_base) as usize;
        let parent_init = class.parent.as_deref().and_then(|p| self.nearest_init(p));
        let this = self.this_operand();
        self.in_function(TacFunction::new(format!("{name}.$init"), 1, frame), Some(0), |g| {
            if let Some(parent_init) = parent_init {
                g.emit(Instr::Param(this));
                g.emit(Instr::Call { dst: None, func: parent_init, nargs: 1 });
            }
            for field in fields {
                g.gen_stmt(field);
            }
        });
    }

    // ─────────────────────────────── expresiones ───────────────────────────────

    pub(crate) fn try_gen_object_expr(&mut self, node: &ParseNode) -> Option<Operand> {
        let sspec = self.sspec;
        if let Some(rule) = &sspec.instantiation {
            if node.symbol == rule.production && node.children.first().map(|c| c.symbol.as_str()) == Some(rule.new_token.as_str()) {
                let class = node.children.get(rule.class_name_index)?.lexeme.clone()?;
                let args = flatten_arg_list(node.children.get(rule.arg_list_index)?, sspec.args_list_symbol.as_deref()?);
                return Some(self.gen_new(node, &class, &args));
            }
        }
        if let Some((name, fields)) = find_struct_literal(node, sspec) {
            return Some(self.gen_struct_literal(node, &name, fields));
        }
        if let Some((_, member_idx)) = find_member_access(node, sspec) {
            let base = node.children.first()?;
            let field = node.children.get(member_idx)?.lexeme.clone()?;
            let Some(slot) = self.field_slot(base, &field) else {
                return Some(self.unsupported(node, "el acceso a algo que no es un campo"));
            };
            let object = self.gen_expr(base);
            return Some(self.field_operand(object, slot, &field));
        }
        None
    }

    /// El offset del campo `field` del objeto que produce `base`.
    fn field_slot(&self, base: &ParseNode, field: &str) -> Option<isize> {
        let (member, _) = self.find_member(&self.class_of(base)?, field)?;
        (member.kind == SymbolKind::Variable).then(|| member.storage.as_ref().map(|s| s.offset))?
    }

    /// La dirección `t[offset]` del campo de `object`, con el nombre del
    /// campo como comentario.
    fn field_operand(&mut self, object: Operand, offset: isize, field: &str) -> Operand {
        let Operand::Temp(t) = self.into_temp(object) else { unreachable!() };
        Operand::Mem { base: Base::Temp(t), offset, name: Some(field.to_string()) }
    }

    /// ```text
    ///     t = alloc <tamaño>
    ///     t[0] = &vt_C
    ///     param t
    ///     call C.$init, 1           ; si la clase (o un ancestro) tiene campos
    ///     param t
    ///     param <arg>…
    ///     call C.constructor, n+1   ; si la clase (o un ancestro) tiene uno
    /// ```
    /// El valor de la expresión es `t`.
    fn gen_new(&mut self, node: &ParseNode, class: &str, args: &[&ParseNode]) -> Operand {
        let Some(size) = self.analysis.layout.classes.instance_size(class) else {
            return self.unsupported(node, "`new` de una clase sin tamaño");
        };
        let obj = self.new_temp();
        self.emit(Instr::Alloc { dst: obj.clone(), size: Operand::Int(size as i64) });
        self.emit(Instr::Store { base: obj.clone(), index: Operand::Int(0), src: Operand::Addr(format!("vt_{class}")) });
        if let Some(init) = self.nearest_init(class) {
            self.emit(Instr::Param(obj.clone()));
            self.emit(Instr::Call { dst: None, func: init, nargs: 1 });
        }
        let constructor = self.sspec.constructor_name.as_deref().and_then(|c| self.find_member(class, c));
        if let Some((ctor, _)) = constructor.filter(|(m, _)| m.kind == SymbolKind::Function) {
            let Some(label) = ctor.label.clone() else {
                return self.unsupported(node, "un constructor sin etiqueta");
            };
            let values = self.gen_arguments(args);
            self.emit(Instr::Param(obj.clone()));
            self.emit_params(&values);
            self.emit(Instr::Call { dst: None, func: label, nargs: values.len() + 1 });
        }
        obj
    }

    /// ```text
    ///     t = alloc <tamaño>
    ///     t[<offset>] = <valor>     ; uno por campo
    /// ```
    fn gen_struct_literal(&mut self, node: &ParseNode, name: &str, fields: &ParseNode) -> Operand {
        let Some(size) = self.analysis.layout.classes.instance_size(name) else {
            return self.unsupported(node, "un literal de struct sin tamaño");
        };
        let obj = self.new_temp();
        self.emit(Instr::Alloc { dst: obj.clone(), size: Operand::Int(size as i64) });
        for (field, value) in flatten_field_inits(fields, self.sspec) {
            let offset = field
                .lexeme
                .as_deref()
                .and_then(|f| self.find_member(name, f))
                .and_then(|(m, _)| m.storage.as_ref().map(|s| s.offset));
            let Some(offset) = offset else {
                self.unsupported(field, "un campo de struct sin offset");
                continue;
            };
            let v = self.gen_widened(value);
            self.emit(Instr::Store { base: obj.clone(), index: Operand::Int(offset as i64), src: v.clone() });
            self.release(&v);
        }
        obj
    }

    /// Llamada a `method` sobre `this` por la vtable de la clase estática
    /// `class`: la ranura es la posición del método en esa vtable, así que
    /// sirve para cualquier subclase que lo sobrescriba.
    ///
    /// ```text
    ///     param <this>
    ///     param <arg>…
    ///     t = <this>[0]             ; vtable
    ///     t = t[ranura × 4]
    ///     x = callv t, n+1          ; n+1: cuenta a this
    /// ```
    pub(crate) fn gen_method_call(&mut self, node: &ParseNode, class: &str, method: &str, this: Operand, args: &[&ParseNode]) -> Operand {
        let slot = self.analysis.layout.classes.vtable(class).and_then(|v| v.iter().position(|m| m == method));
        let returns_value = self
            .find_member(class, method)
            .and_then(|(m, _)| m.signature.as_ref())
            .is_some_and(|s| s.returns != Type::Void);
        let Some(slot) = slot else {
            return self.unsupported(node, "un método que no está en la vtable de su clase");
        };

        let values = self.gen_arguments(args);
        self.emit(Instr::Param(this.clone()));
        self.emit_params(&values);

        self.release(&this);
        let target = self.new_temp();
        self.emit(Instr::Load { dst: target.clone(), base: this, index: Operand::Int(0) });
        let offset = (slot * self.target.pointer) as i64;
        self.emit(Instr::Load { dst: target.clone(), base: target.clone(), index: Operand::Int(offset) });
        self.release(&target);
        let dst = returns_value.then(|| self.new_temp());
        self.emit(Instr::CallV { dst: dst.clone(), target, nargs: values.len() + 1 });
        dst.unwrap_or(Operand::Null)
    }

    // ─────────────────────────────── asignación ───────────────────────────────

    /// `obj.campo = valor;` (la alternativa de `%assign` con acceso a
    /// miembro). El valor se calcula antes que el objeto, así los temporales
    /// del objeto no están ocupados mientras se evalúa el valor.
    pub(crate) fn try_gen_object_assign(&mut self, node: &ParseNode) -> bool {
        let sspec = self.sspec;
        let Some(rule) = &sspec.assign else { return false };
        if node.symbol != rule.production {
            return false;
        }
        let Some((_, member_idx)) = find_member_access(node, sspec) else { return false };
        let (Some(base), Some(field), Some(value)) =
            (node.children.first(), node.children[member_idx].lexeme.as_deref(), node.children.last())
        else {
            return false;
        };
        let Some(offset) = self.field_slot(base, field) else {
            self.unsupported(node, "la asignación a algo que no es un campo");
            return true;
        };
        let v = self.gen_widened(value);
        let object = self.gen_expr(base);
        let dst = self.field_operand(object, offset, field);
        self.emit(Instr::Copy { dst: dst.clone(), src: v.clone() });
        self.release(&v);
        self.release(&dst);
        true
    }
}
