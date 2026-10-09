//! Funciones y llamadas — Fase 2 (integrante B).
//!
//! - `gen_function`: una `TacFunction` propia por función o método, con la
//!   etiqueta de su símbolo (`contador.sumar`, `Animal.hablar`). Una función
//!   anidada se traduce en medio de la que la encierra: la de afuera se apila
//!   en `suspended` y se retoma al terminar.
//! - `try_gen_call` (`%call`): primero se evalúan TODOS los argumentos, luego
//!   se emiten juntos los `param`, después el `link` y el `call`. Así
//!   `f(g(1), 2)` no entrelaza los `param` de las dos llamadas.
//! - `try_gen_return` (`%return`).
//!
//! Una función cuyo cuerpo termina sin `return` no lleva uno explícito:
//! `endfunc` ya implica el retorno.

use crate::semantico::analyzer::{find_child_index, find_identifier_child};
use crate::semantico::bindings::Access;
use crate::semantico::classes::{find_member_access, flatten_arg_list};
use crate::semantico::symbols::{Symbol, SymbolKind};
use crate::semantico::types::Type;
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::{Base, Instr, Operand, TacFunction};
use super::{FunctionBuilder, Generator};

impl<'a> Generator<'a> {
    /// Traduce el cuerpo de una función como una `TacFunction` aparte.
    ///
    /// `hops` es el valor de `method_hops` dentro de ella (ver `Generator`).
    /// El lugar de la función en `program.functions` se reserva al entrar, así
    /// las funciones quedan en orden de aparición (la de afuera antes que las
    /// anidadas). `break` y `try` abiertos no cruzan el límite de la función.
    pub(crate) fn in_function(&mut self, func: TacFunction, hops: Option<usize>, body: impl FnOnce(&mut Self)) {
        let slot = self.program.functions.len();
        self.program.functions.push(func.clone());
        let outer = std::mem::replace(&mut self.current, FunctionBuilder::new(func));
        self.suspended.push(outer);
        let jumps = std::mem::take(&mut self.jumps);
        let try_depth = std::mem::take(&mut self.try_depth);
        let outer_hops = std::mem::replace(&mut self.method_hops, hops);

        body(self);

        self.method_hops = outer_hops;
        self.try_depth = try_depth;
        self.jumps = jumps;
        let outer = self.suspended.pop().expect("la función de afuera se apiló al entrar");
        self.program.functions[slot] = std::mem::replace(&mut self.current, outer).finish();
    }

    /// Una declaración de función o método.
    pub(crate) fn gen_function(&mut self, node: &ParseNode) {
        let Some(sym) = self.function_symbol(node) else {
            self.unsupported(node, "una función sin símbolo");
            return;
        };
        let (Some(label), Some(level), Some(frame)) = (sym.label.clone(), sym.nesting_level, sym.storage_size) else {
            self.unsupported(node, "una función sin etiqueta, nivel o marco");
            return;
        };
        let hops = match (self.class_ctx.is_some(), self.method_hops) {
            (_, Some(k)) => Some(k + 1),
            (true, None) => Some(0),
            (false, None) => None,
        };
        // El cuerpo es el último hijo (el bloque); los anteriores son el
        // nombre, los parámetros y el tipo de retorno.
        let Some(body) = node.children.last() else { return };
        self.in_function(TacFunction::new(label, level, frame), hops, |g| g.gen_stmt(body));
    }

    /// El símbolo de la función que declara `node` (`%declare … function`).
    fn function_symbol(&self, node: &ParseNode) -> Option<&'a Symbol> {
        let rule = self
            .sspec
            .declarations
            .iter()
            .find(|r| r.production == node.symbol && r.kind == SymbolKind::Function)?;
        let (_, leaf) = find_identifier_child(node, &self.sspec.identifier_token, rule.name_child)?;
        let analysis = self.analysis;
        analysis.resolve(analysis.bindings.get(leaf)?)
    }

    /// `true` si `node` es una llamada (`%call`).
    pub(crate) fn is_call(&self, node: &ParseNode) -> bool {
        self.sspec
            .call
            .as_ref()
            .is_some_and(|r| node.symbol == r.production && node.children.iter().any(|c| c.symbol == r.open_paren_token))
    }

    /// Una llamada usada como expresión (`%call`). Devuelve el operando con
    /// el resultado, o `Operand::Null` si la función no devuelve valor.
    pub(crate) fn try_gen_call(&mut self, node: &ParseNode) -> Option<Operand> {
        let sspec = self.sspec;
        let rule = sspec.call.as_ref()?;
        if node.symbol != rule.production || !node.children.iter().any(|c| c.symbol == rule.open_paren_token) {
            return None;
        }
        let callee = node.children.get(rule.callee_index)?;
        let args = flatten_arg_list(node.children.get(rule.arg_list_index)?, sspec.args_list_symbol.as_deref()?);

        // `obj.metodo(args)`: despacho virtual sobre la clase estática de `obj`.
        if let Some((_, member_idx)) = find_member_access(callee, sspec) {
            let base = callee.children.first()?;
            let method = callee.children.get(member_idx)?.lexeme.clone()?;
            let Some(class) = self.class_of(base) else {
                return Some(self.unsupported(node, "una llamada a un método de algo que no es un objeto"));
            };
            let object = self.gen_expr(base);
            let object = self.as_value(object);
            return Some(self.gen_method_call(node, &class, &method, object, &args));
        }

        // `f(args)`: bajar por la cadena de un solo hijo hasta el identificador.
        let mut leaf = callee;
        while leaf.children.len() == 1 {
            leaf = &leaf.children[0];
        }
        let Some(r) = self.analysis.bindings.get(leaf).cloned() else {
            return Some(self.unsupported(node, "una llamada a algo que no es una función con nombre"));
        };
        let Some(sym) = self.analysis.resolve(&r) else {
            return Some(self.unsupported(node, "una llamada a un nombre sin símbolo"));
        };
        if sym.kind != SymbolKind::Function {
            return Some(self.unsupported(node, "una llamada a algo que no es una función"));
        }

        // Un método llamado sin `this.` desde otro método (o desde una función
        // anidada en él): también es virtual, sobre el `this` actual.
        if r.access == Access::Field {
            let Some(class) = self.class_ctx.clone() else {
                return Some(self.unsupported(node, "un método llamado fuera de su clase"));
            };
            let this = match self.method_hops {
                Some(hops) if hops > 0 => self.load_this(),
                _ => self.this_operand(),
            };
            return Some(self.gen_method_call(node, &class, &r.name, this, &args));
        }

        let Some(label) = sym.label.clone() else {
            return Some(self.unsupported(node, "una función sin etiqueta"));
        };
        let returns_value = sym.signature.as_ref().is_some_and(|s| s.returns != Type::Void);

        let values = self.gen_arguments(&args);
        self.emit_params(&values);
        // Enlace de acceso del llamado (`nc - ng + 1` enlaces desde el propio
        // marco): una función declarada en la actual recibe el marco actual; la
        // de una función de afuera, el marco de esa función. Las globales no
        // lo necesitan.
        match r.access {
            Access::Local => self.emit(Instr::Link(frame_pointer())),
            Access::NonLocal { hops } => {
                let frame = self.follow_access_links(hops);
                let Base::Temp(t) = frame else { unreachable!() };
                self.emit(Instr::Link(Operand::Temp(t)));
                self.release(&Operand::Temp(t));
            }
            Access::Static | Access::Field => {}
        }
        let dst = returns_value.then(|| self.new_temp());
        self.emit(Instr::Call { dst: dst.clone(), func: label, nargs: values.len() });
        Some(dst.unwrap_or(Operand::Null))
    }

    /// Evalúa los argumentos de izquierda a derecha, con la ampliación
    /// `(float)` que marcó el análisis. Los operandos siguen vivos hasta su
    /// `param`.
    pub(crate) fn gen_arguments(&mut self, args: &[&ParseNode]) -> Vec<Operand> {
        args.iter().map(|a| self.gen_widened(a)).collect()
    }

    /// Un `param` por argumento, seguidos, liberando cada operando después.
    pub(crate) fn emit_params(&mut self, values: &[Operand]) {
        for v in values {
            self.emit(Instr::Param(v.clone()));
            self.release(v);
        }
    }

    /// Una sentencia `return` (`%return`). Con valor: se calcula (con la
    /// conversión al tipo de retorno), se cierran los `try` abiertos y se
    /// retorna.
    pub(crate) fn try_gen_return(&mut self, node: &ParseNode) -> bool {
        let Some(rule) = self.sspec.returns.iter().find(|r| r.production == node.symbol) else {
            return false;
        };
        let value = find_child_index(node, &rule.value_child).map(|i| self.gen_widened(&node.children[i]));
        for _ in 0..self.try_depth {
            self.emit(Instr::TryEnd);
        }
        if let Some(v) = &value {
            self.release(v);
        }
        self.emit(Instr::Return(value));
        true
    }
}

/// `fp`: la dirección del marco actual, como operando de `link`. Se escribe
/// `fp` (no `fp[0]`, que sería el enlace de control): el valor es el propio
/// `$fp`.
fn frame_pointer() -> Operand {
    Operand::Var { name: "fp".into(), base: Base::Fp, offset: 0 }
}
