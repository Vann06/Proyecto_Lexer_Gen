//! Traducción de expresiones — Fase 1 (integrante A).
//!
//! Dos formas de traducir una expresión, como en el capítulo 6 del libro:
//!
//! - **Como valor** (`try_gen_expr`, §6.4): emite las instrucciones que la
//!   calculan y devuelve la dirección donde quedó el resultado (el `E.addr`
//!   del libro). Los temporales que consume cada instrucción se liberan
//!   apenas se emite, así se reciclan (ver `temps.rs`).
//! - **Como condición** (`gen_cond`, §6.6): una expresión booleana en un
//!   `if`/`while`/`for` no se calcula, se traduce a saltos. `&&` y `||` hacen
//!   cortocircuito: el segundo operando ni se evalúa si el primero decide.
//!
//! Precedencia y asociatividad no se resuelven acá: ya vienen en la forma
//! del árbol (la gramática las codifica en su cadena `or_expr → and_expr →
//! … → unary`).
//!
//! Las formas de la Fase 2 (llamadas, objetos, listas) se delegan a
//! `func.rs`, `objects.rs` y `lists.rs`.

use crate::semantico::classes;
use crate::semantico::operators::{self, ComparisonOperator, LogicalOperator, UnaryOperator};
use crate::semantico::types::{ArithmeticOperator, Coercion, Type};
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::super::tac::{BinOp, Instr, Operand, UnOp};
use super::Generator;

impl<'a> Generator<'a> {
    pub(crate) fn try_gen_expr(&mut self, node: &ParseNode) -> Option<Operand> {
        // Las formas de la Fase 2 primero: una llamada `f(x)` o un acceso
        // `obj.campo` tienen hijos que el resto confundiría con otra cosa.
        if let Some(value) = self
            .try_gen_call(node)
            .or_else(|| self.try_gen_object_expr(node))
            .or_else(|| self.try_gen_list_expr(node))
        {
            return Some(value);
        }

        // Una hoja, o un nodo cuyo único hijo es una hoja (`atom: INT_LIT`).
        if node.children.is_empty() {
            return Some(self.gen_leaf(node));
        }
        if node.children.len() == 1 && node.children[0].children.is_empty() {
            return Some(self.gen_leaf(&node.children[0]));
        }

        // `( expr )`
        if let Some(inner) = self.group_inner(node) {
            return Some(self.gen_expr(inner));
        }

        if let Some((op, left, right)) = classes::find_arithmetic(node, self.sspec) {
            return Some(self.gen_arithmetic(node, op, left, right));
        }
        if let Some((op, left, right)) = operators::find_comparison(node, self.sspec) {
            let l = self.gen_widened(left);
            let r = self.gen_widened(right);
            self.release(&l);
            self.release(&r);
            let dst = self.new_temp();
            self.emit(Instr::Binary { dst: dst.clone(), op: relop(op), left: l, right: r });
            return Some(dst);
        }
        if operators::find_logical(node, self.sspec).is_some() {
            return Some(self.gen_bool_value(node));
        }
        if let Some((op, operand)) = operators::find_unary(node, self.sspec) {
            let value = self.gen_widened(operand);
            let unop = match op {
                UnaryOperator::Not => UnOp::Not,
                UnaryOperator::Negate if self.type_of(node) == Some(&Type::Float) => UnOp::NegF,
                UnaryOperator::Negate => UnOp::Neg,
            };
            self.release(&value);
            let dst = self.new_temp();
            self.emit(Instr::Unary { dst: dst.clone(), op: unop, src: value });
            return Some(dst);
        }
        None
    }

    /// El hijo que encierra una agrupación `( expr )` (`%group`).
    fn group_inner<'n>(&self, node: &'n ParseNode) -> Option<&'n ParseNode> {
        let rule = self.sspec.groups.iter().find(|g| g.production == node.symbol)?;
        if node.children.first()?.symbol != rule.open_token {
            return None;
        }
        node.children.get(rule.inner_index)
    }

    /// Una hoja: una variable (o `this`), o un literal.
    fn gen_leaf(&mut self, leaf: &ParseNode) -> Operand {
        if let Some(r) = self.analysis.bindings.get(leaf).cloned() {
            return match self.var_operand(&r) {
                Some(op) => op,
                None => self.unsupported(leaf, "un nombre que no es una variable"),
            };
        }
        let lexeme = leaf.lexeme.as_deref().unwrap_or("");
        match self.sspec.type_tokens.get(&leaf.symbol) {
            Some(Type::Int) => match lexeme.parse() {
                Ok(n) => Operand::Int(n),
                Err(_) => self.unsupported(leaf, "un literal entero fuera de rango"),
            },
            Some(Type::Float) => match lexeme.parse() {
                Ok(x) => Operand::Float(x),
                Err(_) => self.unsupported(leaf, "un literal flotante mal formado"),
            },
            Some(Type::Bool) => Operand::Bool(lexeme == "true"),
            Some(Type::Str) => Operand::Str(unquote(lexeme)),
            Some(Type::Null) => Operand::Null,
            _ => self.unsupported(leaf, "una hoja que no es variable ni literal"),
        }
    }

    /// Traduce `node` y le aplica la ampliación `(float)` si el análisis la
    /// marcó (`TypeAnnotations::coercion`).
    pub(crate) fn gen_widened(&mut self, node: &ParseNode) -> Operand {
        let value = self.gen_expr(node);
        self.widen(node, value)
    }

    /// Aplica a `value` (el valor ya calculado de `node`) la conversión que
    /// el análisis marcó en `node`, si hay una.
    pub(crate) fn widen(&mut self, node: &ParseNode, value: Operand) -> Operand {
        match self.analysis.types.coercion(node) {
            Coercion::Exact => value,
            Coercion::IntToFloat => {
                self.release(&value);
                let dst = self.new_temp();
                self.emit(Instr::Unary { dst: dst.clone(), op: UnOp::IntToFloat, src: value });
                dst
            }
        }
    }

    fn gen_arithmetic(&mut self, node: &ParseNode, op: ArithmeticOperator, left: &ParseNode, right: &ParseNode) -> Operand {
        let l = self.gen_widened(left);
        let r = self.gen_widened(right);
        let float = self.type_of(node) == Some(&Type::Float);
        let binop = match op {
            ArithmeticOperator::Add if self.type_of(node) == Some(&Type::Str) => BinOp::Concat,
            ArithmeticOperator::Add if float => BinOp::AddF,
            ArithmeticOperator::Subtract if float => BinOp::SubF,
            ArithmeticOperator::Multiply if float => BinOp::MulF,
            ArithmeticOperator::Divide if float => BinOp::DivF,
            ArithmeticOperator::Add => BinOp::Add,
            ArithmeticOperator::Subtract => BinOp::Sub,
            ArithmeticOperator::Multiply => BinOp::Mul,
            ArithmeticOperator::Divide => BinOp::Div,
            ArithmeticOperator::Modulo => BinOp::Mod,
        };
        // Dividir entre cero es un error en tiempo de ejecución (lo atrapa un
        // `try`, ver `stmt.rs`).
        if matches!(binop, BinOp::Div | BinOp::DivF | BinOp::Mod) {
            self.emit(Instr::CheckZero(r.clone()));
        }
        // Liberar ANTES de pedir el destino: así el resultado reusa el
        // temporal de un operando (`t0 = t0 + t1`), que ya no se vuelve a leer.
        self.release(&l);
        self.release(&r);
        let dst = self.new_temp();
        self.emit(Instr::Binary { dst: dst.clone(), op: binop, left: l, right: r });
        dst
    }

    /// Una expresión booleana como VALOR (`let b = x < 3 && y;`): se
    /// traduce como condición y cada salida guarda `true` o `false` en un
    /// mismo temporal.
    pub(crate) fn gen_bool_value(&mut self, node: &ParseNode) -> Operand {
        let on_false = self.new_label();
        let end = self.new_label();
        self.gen_cond(node, None, Some(&on_false));
        let dst = self.new_temp();
        self.emit(Instr::Copy { dst: dst.clone(), src: Operand::Bool(true) });
        self.emit(Instr::Goto(end.clone()));
        self.emit(Instr::Label(on_false));
        self.emit(Instr::Copy { dst: dst.clone(), src: Operand::Bool(false) });
        self.emit(Instr::Label(end));
        dst
    }

    /// Código de saltos para la condición `node` (§6.6 del libro, con la
    /// técnica de "caer de largo" del §6.6.5): salta a `on_true` si es
    /// verdadera y a `on_false` si es falsa. Una de las dos puede ser `None`,
    /// que significa "seguir con la instrucción siguiente" — así no se emiten
    /// `goto` innecesarios. Nunca las dos.
    pub(crate) fn gen_cond(&mut self, node: &ParseNode, on_true: Option<&str>, on_false: Option<&str>) {
        debug_assert!(on_true.is_some() || on_false.is_some());

        // Eslabones de la cadena de precedencia y agrupaciones: la condición
        // es la de adentro.
        if node.children.len() == 1 && !node.children[0].children.is_empty() {
            return self.gen_cond(&node.children[0], on_true, on_false);
        }
        if let Some(inner) = self.group_inner(node) {
            return self.gen_cond(inner, on_true, on_false);
        }

        if let Some((op, left, right)) = operators::find_logical(node, self.sspec) {
            match op {
                // B1 && B2: si B1 es falsa, ya se sabe la respuesta; si es
                // verdadera, decide B2.
                LogicalOperator::And => {
                    let left_false = on_false.map(str::to_string).unwrap_or_else(|| self.new_label());
                    self.gen_cond(left, None, Some(&left_false));
                    self.gen_cond(right, on_true, on_false);
                    if on_false.is_none() {
                        self.emit(Instr::Label(left_false));
                    }
                }
                // B1 || B2: si B1 es verdadera, ya se sabe; si no, decide B2.
                LogicalOperator::Or => {
                    let left_true = on_true.map(str::to_string).unwrap_or_else(|| self.new_label());
                    self.gen_cond(left, Some(&left_true), None);
                    self.gen_cond(right, on_true, on_false);
                    if on_true.is_none() {
                        self.emit(Instr::Label(left_true));
                    }
                }
            }
            return;
        }

        if let Some((UnaryOperator::Not, operand)) = operators::find_unary(node, self.sspec) {
            return self.gen_cond(operand, on_false, on_true);
        }

        if let Some((op, left, right)) = operators::find_comparison(node, self.sspec) {
            let l = self.gen_widened(left);
            let r = self.gen_widened(right);
            let op = relop(op);
            match (on_true, on_false) {
                (Some(t), Some(f)) => {
                    self.emit(Instr::IfRel { left: l.clone(), op, right: r.clone(), label: t.to_string() });
                    self.emit(Instr::Goto(f.to_string()));
                }
                (Some(t), None) => {
                    self.emit(Instr::IfRel { left: l.clone(), op, right: r.clone(), label: t.to_string() })
                }
                (None, Some(f)) => {
                    self.emit(Instr::IfRel { left: l.clone(), op: negate(op), right: r.clone(), label: f.to_string() })
                }
                (None, None) => {}
            }
            self.release(&l);
            self.release(&r);
            return;
        }

        // Un literal `true`/`false` decide sin evaluar nada.
        if let Some(Operand::Bool(b)) = self.bool_literal(node) {
            match (b, on_true, on_false) {
                (true, Some(t), _) => self.emit(Instr::Goto(t.to_string())),
                (false, _, Some(f)) => self.emit(Instr::Goto(f.to_string())),
                _ => {}
            }
            return;
        }

        // Cualquier otro booleano (una variable, una llamada…): se calcula y
        // se salta según su valor.
        let value = self.gen_expr(node);
        match (on_true, on_false) {
            (Some(t), Some(f)) => {
                self.emit(Instr::IfTrue { cond: value.clone(), label: t.to_string() });
                self.emit(Instr::Goto(f.to_string()));
            }
            (Some(t), None) => self.emit(Instr::IfTrue { cond: value.clone(), label: t.to_string() }),
            (None, Some(f)) => self.emit(Instr::IfFalse { cond: value.clone(), label: f.to_string() }),
            (None, None) => {}
        }
        self.release(&value);
    }

    /// `Some(Bool)` si `node` es (o envuelve) un literal booleano.
    fn bool_literal(&self, node: &ParseNode) -> Option<Operand> {
        let leaf = match node.children.as_slice() {
            [] => node,
            [only] if only.children.is_empty() => only,
            _ => return None,
        };
        (self.sspec.type_tokens.get(&leaf.symbol) == Some(&Type::Bool) && self.analysis.bindings.get(leaf).is_none())
            .then(|| Operand::Bool(leaf.lexeme.as_deref() == Some("true")))
    }
}

fn relop(op: ComparisonOperator) -> BinOp {
    match op {
        ComparisonOperator::Eq => BinOp::Eq,
        ComparisonOperator::Neq => BinOp::Ne,
        ComparisonOperator::Lt => BinOp::Lt,
        ComparisonOperator::Lte => BinOp::Le,
        ComparisonOperator::Gt => BinOp::Gt,
        ComparisonOperator::Gte => BinOp::Ge,
    }
}

/// La comparación contraria: `if a >= b goto Lfalse` en lugar de
/// `if a < b goto Lverdadero` seguido de `goto Lfalse`.
fn negate(op: BinOp) -> BinOp {
    match op {
        BinOp::Eq => BinOp::Ne,
        BinOp::Ne => BinOp::Eq,
        BinOp::Lt => BinOp::Ge,
        BinOp::Le => BinOp::Gt,
        BinOp::Gt => BinOp::Le,
        BinOp::Ge => BinOp::Lt,
        other => other,
    }
}

/// El texto de un literal de cadena sin sus comillas, con los escapes
/// comunes resueltos.
fn unquote(lexeme: &str) -> String {
    let inner = lexeme
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(lexeme);
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negar_una_comparacion_da_la_contraria() {
        assert_eq!(negate(BinOp::Lt), BinOp::Ge);
        assert_eq!(negate(BinOp::Ge), BinOp::Lt);
        assert_eq!(negate(BinOp::Eq), BinOp::Ne);
        assert_eq!(negate(BinOp::Le), BinOp::Gt);
    }

    #[test]
    fn un_literal_de_cadena_pierde_sus_comillas() {
        assert_eq!(unquote("\"hola\""), "hola");
        assert_eq!(unquote("\"a\\\"b\\n\""), "a\"b\n");
    }
}
