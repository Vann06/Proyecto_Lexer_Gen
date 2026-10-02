//! El generador de código intermedio: recorre el árbol anotado y emite TAC.
//!
//! Es una traducción dirigida por la sintaxis (§6.4 del libro): cada forma
//! del árbol tiene su regla, y las reglas de las expresiones devuelven la
//! dirección (`Operand`) donde quedó su valor —el `E.addr` del libro—
//! mientras van emitiendo instrucciones.
//!
//! Igual que `semantico::analyzer`, el generador NO nombra producciones de
//! ninguna gramática. Reconoce cada forma con lo que ya declaró el `.yalp`:
//! los reconocedores del análisis (`classes::find_arithmetic`,
//! `operators::find_comparison`…), los `%flow` y `%print` de
//! `IntermediateSpec`, y los mapas laterales que dejó el análisis (tipos,
//! conversiones, enlaces uso→declaración, registros de activación).
//!
//! ## Organización (una parte por integrante — ver `PLAN_TAC.md`)
//!
//! | Archivo | Qué traduce | Fase |
//! |---|---|---|
//! | `mod.rs` | Compuertas, recorrido genérico, utilidades | 0 |
//! | `expr.rs` | Expresiones | 1 |
//! | `stmt.rs` | Declaraciones, asignaciones, control de flujo | 1 |
//! | `func.rs` | Funciones, llamadas, `return`, enlaces de acceso | 2 |
//! | `objects.rs` | Clases: `new`, campos, métodos, `this`, vtables | 2 |
//! | `lists.rs` | Listas: literales e índices | 2 |
//!
//! Cada archivo agrega métodos al mismo `Generator` con su propio bloque
//! `impl`; el recorrido de acá llama a sus puntos de entrada (`try_gen_*`).

mod expr;
mod func;
mod lists;
mod objects;
mod stmt;

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::semantico::analyzer::AnalysisResult;
use crate::semantico::bindings::SymbolRef;
use crate::semantico::errors::Severity;
use crate::semantico::scopes::ScopeKind;
use crate::semantico::bindings::Access;
use crate::semantico::spec::SemanticSpec;
use crate::semantico::storage::{width_of, LayoutReport, TargetLayout, Width};
use crate::semantico::types::Type;
use crate::sintactico::runtime::parse_tree::ParseNode;

use super::spec::IntermediateSpec;
use super::tac::{Base, Instr, Operand, TacFunction, TacProgram};
use super::temps::TempPool;

/// Bytes de la cabecera de una lista en el heap: la primera palabra guarda
/// su longitud, y los elementos empiezan después.
pub(crate) const LIST_HEADER: isize = 4;

/// Un problema de la generación, con la misma forma que los diagnósticos de
/// las otras fases para que el IDE los muestre igual. Códigos `C###`:
///
/// - `C001`: el programa tiene errores de fases anteriores; no se genera TAC.
/// - `C002`: algún registro de activación quedó incompleto (un símbolo sin
///   tipo no tiene tamaño); no se generan direcciones inventadas.
/// - `C900`: una construcción que el generador todavía no traduce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodegenError {
    pub code: String,
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl CodegenError {
    pub fn new(code: &str, message: impl Into<String>, line: usize, col: usize) -> Self {
        CodegenError { code: code.to_string(), message: message.into(), line, col }
    }

    /// Forma `{level, code, msg, loc, line, col}`, la de `problems`.
    pub fn to_problem(&self, source_name: &str) -> Value {
        json!({
            "level": "err",
            "code": self.code,
            "msg": self.message,
            "loc": format!("{}:{}:{}", source_name, self.line, self.col),
            "line": self.line,
            "col": self.col,
        })
    }
}

/// Traduce el programa `tree` a TAC.
///
/// Solo genera si las fases anteriores no dejaron errores (las advertencias
/// no bloquean) y si todos los registros de activación quedaron completos.
/// Si algo no se puede traducir, devuelve TODOS los problemas encontrados, no
/// solo el primero, igual que el análisis semántico.
pub fn generate(
    tree: &ParseNode,
    analysis: &AnalysisResult,
    sspec: &SemanticSpec,
    ispec: &IntermediateSpec,
) -> Result<TacProgram, Vec<CodegenError>> {
    let semantic_errors = analysis.errors.iter().filter(|d| d.severity == Severity::Error).count();
    if semantic_errors > 0 {
        return Err(vec![CodegenError::new(
            "C001",
            format!("no se genera código intermedio: el programa tiene {semantic_errors} error(es)"),
            0,
            0,
        )]);
    }
    if !analysis.layout.is_complete() {
        return Err(vec![CodegenError::new(
            "C002",
            "no se genera código intermedio: hay símbolos sin tipo, y sin tipo no se sabe cuánto ocupan en su registro de activación",
            0,
            0,
        )]);
    }

    let mut generator = Generator::new(analysis, sspec, ispec);
    generator.gen_stmt(tree);
    generator.finish()
}

/// Completa el informe de almacenamiento del análisis con lo que solo se
/// sabe después de generar: cuántas ranuras de temporal necesitó cada
/// función (y `main`). Es la vuelta de la generación a la tabla de símbolos
/// que pide el enunciado ("la tabla de símbolos interactúa con cada fase"):
/// el marco final de una función es el que calculó el análisis más su área
/// de temporales.
///
/// Cada función se encuentra por su etiqueta (`Symbol::label`), que es el
/// nombre de su `TacFunction`.
pub fn extend_layout(layout: &mut LayoutReport, program: &TacProgram) {
    for func in &program.functions {
        if let Some(owner) = layout.functions.values_mut().find(|o| o.name == func.name) {
            owner.temp_slots = Some(func.max_temps);
        }
    }
    layout.main_temp_slots = program.main.as_ref().map(|m| m.max_temps);
}

/// La función que se está traduciendo: sus instrucciones y sus temporales.
pub(crate) struct FunctionBuilder {
    pub(crate) func: TacFunction,
    pub(crate) temps: TempPool,
}

impl FunctionBuilder {
    pub(crate) fn new(func: TacFunction) -> Self {
        FunctionBuilder { func, temps: TempPool::new() }
    }

    /// Cierra la función: guarda en ella cuántos temporales necesitó.
    ///
    /// Las ranuras del marco salen del TAC ya terminado, no del contador del
    /// `TempPool`: la asignación directa (`retarget_last`) puede eliminar un
    /// temporal que se llegó a pedir, y ese no necesita lugar. Como el
    /// repartidor siempre entrega el número libre más bajo, que aparezca
    /// `tk` implica que hubo `k + 1` vivos a la vez: las ranuras son el
    /// número más alto que aparece, más uno.
    pub(crate) fn finish(mut self) -> TacFunction {
        self.func.max_temps = self
            .func
            .body
            .iter()
            .flat_map(|i| i.operands())
            .filter_map(Operand::temp)
            .max()
            .map_or(0, |t| t + 1);
        self.func.temps_requested = self.temps.requested();
        self.func
    }
}

/// Las etiquetas de salto de una construcción que admite `break` (un bucle o
/// un `switch`) y, si es un bucle, `continue`.
pub(crate) struct JumpTargets {
    pub(crate) break_label: String,
    /// `None` en un `switch`: un `continue` ahí sigue al bucle de afuera.
    pub(crate) continue_label: Option<String>,
    /// Cuántos `try` estaban abiertos al entrar: un salto que sale de un
    /// `try` tiene que cerrar sus manejadores (`try_end`) antes de saltar.
    pub(crate) try_depth: usize,
}

/// El estado del recorrido. Cada archivo de `gen/` le agrega métodos.
// `suspended` lo usa recién la Fase 2 (funciones anidadas): quitar el
// `allow` cuando esté.
#[allow(dead_code)]
pub(crate) struct Generator<'a> {
    pub(crate) analysis: &'a AnalysisResult,
    pub(crate) sspec: &'a SemanticSpec,
    pub(crate) ispec: &'a IntermediateSpec,
    pub(crate) program: TacProgram,
    /// La función en curso. El código de nivel de programa es `main`; al
    /// entrar a una función se apila el builder de afuera (ver `func.rs`).
    pub(crate) current: FunctionBuilder,
    /// Funciones cuya traducción quedó a medias porque se entró a una
    /// anidada; se retoman al terminarla.
    pub(crate) suspended: Vec<FunctionBuilder>,
    next_label: usize,
    /// Construcciones abiertas que admiten `break`/`continue`, la más interna
    /// al final.
    pub(crate) jumps: Vec<JumpTargets>,
    /// Cuántos `try` hay abiertos en la función en curso.
    pub(crate) try_depth: usize,
    /// La máquina destino (anchos, offsets del marco): la misma que usó
    /// `semantico::storage`.
    pub(crate) target: TargetLayout,
    /// Nombre con que se escribe cada variable en el TAC, por su
    /// declaración `(scope_id, decl_index)`. Ver `var_name`.
    var_names: HashMap<(usize, usize), String>,
    /// Cuántas variables distintas ya usaron cada nombre del fuente.
    name_uses: HashMap<String, usize>,
    pub(crate) errors: Vec<CodegenError>,
}

impl<'a> Generator<'a> {
    fn new(analysis: &'a AnalysisResult, sspec: &'a SemanticSpec, ispec: &'a IntermediateSpec) -> Self {
        let program = TacProgram {
            static_size: analysis.layout.frames.get(&0).copied().unwrap_or(0),
            ..TacProgram::default()
        };
        Generator {
            analysis,
            sspec,
            ispec,
            program,
            current: FunctionBuilder::new(TacFunction::new("main", 0, 0)),
            suspended: Vec::new(),
            next_label: 0,
            jumps: Vec::new(),
            try_depth: 0,
            target: TargetLayout::default(),
            var_names: HashMap::new(),
            name_uses: HashMap::new(),
            errors: Vec::new(),
        }
    }

    fn finish(mut self) -> Result<TacProgram, Vec<CodegenError>> {
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        let main = std::mem::replace(&mut self.current, FunctionBuilder::new(TacFunction::new("", 0, 0)));
        self.program.main = Some(main.finish());
        Ok(self.program)
    }

    // ─────────────────────────── utilidades comunes ───────────────────────────

    /// Agrega una instrucción a la función en curso.
    pub(crate) fn emit(&mut self, instr: Instr) {
        self.current.func.body.push(instr);
    }

    /// Una etiqueta nueva (`L0`, `L1`, …), única en todo el programa.
    pub(crate) fn new_label(&mut self) -> String {
        let label = format!("L{}", self.next_label);
        self.next_label += 1;
        label
    }

    /// El nombre con que se escribe en el TAC la variable declarada en `r`
    /// (convención C, ver `tac.rs`). La primera variable con un nombre lo usa
    /// tal cual; otra variable DISTINTA con el mismo nombre (una local que
    /// tapa a una global, dos funciones con un parámetro `n`…) recibe un
    /// sufijo: `x.1`, `x.2`. La misma declaración siempre da el mismo nombre.
    pub(crate) fn var_name(&mut self, r: &SymbolRef) -> String {
        if let Some(name) = self.var_names.get(&(r.scope_id, r.decl_index)) {
            return name.clone();
        }
        let uses = self.name_uses.entry(r.name.clone()).or_insert(0);
        let name = if *uses == 0 { r.name.clone() } else { format!("{}.{}", r.name, uses) };
        *uses += 1;
        self.var_names.insert((r.scope_id, r.decl_index), name.clone());
        name
    }

    /// Un temporal nuevo de la función en curso.
    pub(crate) fn new_temp(&mut self) -> Operand {
        self.current.temps.new_operand()
    }

    /// Marca `operand` como consumido: si es un temporal, vuelve a la reserva.
    /// Se llama DESPUÉS de emitir la instrucción que lo lee.
    pub(crate) fn release(&mut self, operand: &Operand) {
        self.current.temps.release_operand(operand);
    }

    /// Registra que `node` es una construcción que todavía no se traduce.
    /// Devuelve un operando de relleno para que el recorrido siga y se
    /// reporten todos los casos, no solo el primero.
    pub(crate) fn unsupported(&mut self, node: &ParseNode, what: &str) -> Operand {
        self.errors.push(CodegenError::new(
            "C900",
            format!("construcción todavía no soportada por el generador: {what} (`{}`)", node.symbol),
            node.line,
            node.col,
        ));
        Operand::Null
    }

    /// El tipo que el análisis le dio a `node`, si es una expresión tipada.
    pub(crate) fn type_of(&self, node: &ParseNode) -> Option<&'a Type> {
        self.analysis.types.get(node)
    }

    /// Dónde vive la variable que nombra `r`, ya en forma de operando
    /// (convención C, ver `tac.rs`):
    ///
    /// - `Static` / `Local`: por nombre (`x`, `total`).
    /// - `NonLocal { hops }`: sigue `hops` enlaces de acceso desde `fp[12]` y
    ///   devuelve `tN[offset]`, con el nombre como comentario.
    /// - `Field`: un campo usado sin `this.` dentro de un método: carga
    ///   `this` y devuelve `tN[offset]`.
    ///
    /// `None` si `r` no es una variable con lugar en memoria (una función,
    /// una clase).
    pub(crate) fn var_operand(&mut self, r: &SymbolRef) -> Option<Operand> {
        let offset = self.analysis.resolve(r)?.storage.as_ref()?.offset;
        let name = self.var_name(r);
        Some(match r.access {
            Access::Static => Operand::global(name, offset),
            Access::Local => Operand::local(name, offset),
            Access::NonLocal { hops } => {
                let frame = self.follow_access_links(hops);
                Operand::Mem { base: frame, offset, name: Some(name) }
            }
            Access::Field => {
                let this = self.new_temp();
                self.emit(Instr::Copy { dst: this.clone(), src: self.this_operand() });
                let Operand::Temp(t) = this else { unreachable!() };
                Operand::Mem { base: Base::Temp(t), offset, name: Some(r.name.clone()) }
            }
        })
    }

    /// `this` dentro de un método: su primer parámetro oculto.
    pub(crate) fn this_operand(&self) -> Operand {
        Operand::local("this", self.target.param_base)
    }

    /// Sigue `hops` (≥ 1) enlaces de acceso desde el marco actual y devuelve
    /// el temporal que queda apuntando al marco alcanzado:
    /// `t = fp[12]`, `t = t[12]`, …
    pub(crate) fn follow_access_links(&mut self, hops: usize) -> Base {
        let link = self.target.access_link;
        let frame = self.new_temp();
        let Operand::Temp(t) = frame else { unreachable!() };
        self.emit(Instr::Copy { dst: frame.clone(), src: Operand::mem(Base::Fp, link) });
        for _ in 1..hops {
            self.emit(Instr::Copy { dst: frame.clone(), src: Operand::mem(Base::Temp(t), link) });
        }
        Base::Temp(t)
    }

    /// Bytes que ocupa cada elemento de una lista de `elem` (redondeado a la
    /// palabra). Una lista en el heap es `[longitud][e0][e1]…`; ver
    /// `LIST_HEADER`. Lo comparten `stmt.rs` (`foreach`) y `lists.rs`.
    pub(crate) fn list_slot(&self, elem: &Type) -> Option<usize> {
        match width_of(elem, &self.target) {
            Width::Known(w) => {
                let word = self.target.word;
                Some(w.max(1).div_ceil(word) * word)
            }
            Width::Unresolved => None,
        }
    }

    /// Asignación directa (como escribe el libro: `x = a + b`): si la última
    /// instrucción emitida dejó su resultado en el temporal `value`, se le
    /// cambia el destino por `dst` en vez de agregar `dst = value`. Solo para
    /// un destino con nombre (`Operand::Var`), que es la forma de tres
    /// direcciones `x = y op z`. Devuelve `true` si lo hizo; el llamador
    /// libera `value`.
    pub(crate) fn retarget_last(&mut self, value: &Operand, dst: &Operand) -> bool {
        if !matches!(value, Operand::Temp(_)) || !matches!(dst, Operand::Var { .. }) {
            return false;
        }
        let Some(last) = self.current.func.body.last_mut() else { return false };
        let slot = match last {
            Instr::Copy { dst, .. }
            | Instr::Binary { dst, .. }
            | Instr::Unary { dst, .. }
            | Instr::Load { dst, .. }
            | Instr::Alloc { dst, .. }
            | Instr::AddrOf { dst, .. } => dst,
            Instr::Call { dst: Some(dst), .. } | Instr::CallV { dst: Some(dst), .. } => dst,
            _ => return false,
        };
        if slot != value {
            return false;
        }
        *slot = dst.clone();
        true
    }

    /// El tipo de ámbito que abre `node`, si su producción tiene `%scope`.
    pub(crate) fn scope_kind_of(&self, node: &ParseNode) -> Option<ScopeKind> {
        self.sspec.scopes.iter().find(|r| r.production == node.symbol).map(|r| r.kind)
    }

    // ─────────────────────────── recorrido genérico ───────────────────────────

    /// Traduce una sentencia (o cualquier nodo que no sea expresión).
    ///
    /// Orden de decisión:
    /// 1. Una función o una clase/struct: tienen su propio código (`func.rs`,
    ///    `objects.rs`), no se traducen en línea.
    /// 2. Una sentencia que reconoce `stmt.rs` (declaración, asignación,
    ///    control de flujo, `print`, `break`/`continue`, `return`…).
    /// 3. Un nodo tipado es una EXPRESIÓN usada como sentencia: se evalúa (por
    ///    sus efectos, p.ej. una llamada) y su resultado se descarta.
    /// 4. Cualquier otro nodo (secuencias, bloques, envoltorios como
    ///    `stmt: X SEMI`): se traducen sus hijos en orden. Las hojas sueltas
    ///    (`;`, `{`, `}`) no generan nada.
    pub(crate) fn gen_stmt(&mut self, node: &ParseNode) {
        match self.scope_kind_of(node) {
            Some(ScopeKind::Function) => return self.gen_function(node),
            Some(ScopeKind::Class) | Some(ScopeKind::Struct) => return self.gen_class(node),
            _ => {}
        }
        if self.try_gen_statement(node) {
            return;
        }
        if !node.children.is_empty() && self.analysis.types.get(node).is_some() {
            let value = self.gen_expr(node);
            self.release(&value);
            return;
        }
        for child in &node.children {
            if !child.children.is_empty() {
                self.gen_stmt(child);
            }
        }
    }

    /// Traduce una expresión y devuelve dónde quedó su valor.
    ///
    /// Un nodo de un solo hijo es un eslabón de la cadena de precedencia
    /// (`expr → or_expr → … → primary → atom`): su valor es el del hijo. El
    /// resto lo decide `expr.rs` (Fase 1) y, para llamadas, objetos y listas,
    /// `func.rs`/`objects.rs`/`lists.rs` (Fase 2).
    pub(crate) fn gen_expr(&mut self, node: &ParseNode) -> Operand {
        if node.children.len() == 1 && !node.children[0].children.is_empty() {
            return self.gen_expr(&node.children[0]);
        }
        if let Some(value) = self.try_gen_expr(node) {
            return value;
        }
        self.unsupported(node, "expresión")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_error_de_generacion_tiene_la_forma_de_los_problemas() {
        let e = CodegenError::new("C900", "algo", 3, 7);
        let p = e.to_problem("prog.cps");
        assert_eq!(p["level"], "err");
        assert_eq!(p["code"], "C900");
        assert_eq!(p["loc"], "prog.cps:3:7");
        assert_eq!(p["line"], 3);
    }
}
