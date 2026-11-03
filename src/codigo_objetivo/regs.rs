//! La asignación de registros. `RegAlloc` es la interfaz que usan los
//! traductores (`instr.rs`, `calls.rs`, `runtime.rs`); `StackAlloc` es la
//! primera implementación: cada temporal vive siempre en su ranura del marco.

use std::rc::Rc;

use crate::intermedio::tac::{Base, Operand, TacFunction, TEMP_SLOT};

use super::asm::{AsmLine, FunctionStats, Reg};
use super::names::{vtable_label, StringTable};

/// Auxiliares que nunca guardan un valor de una instrucción a otra
/// (`PLAN_MIPS.md` §4.2): `$t8` para direcciones calculadas, `$t9` para la
/// base de un acceso `tk[off]`.
pub const AUX_DIR: Reg = Reg::T(8);
pub const AUX_BASE: Reg = Reg::T(9);

pub trait RegAlloc {
    fn begin_function(&mut self, f: &TacFunction);
    fn begin_instr(&mut self, index: usize);
    /// Un registro que contiene el valor de `op` (emite el lw/li/la necesario).
    fn use_operand(&mut self, out: &mut Vec<AsmLine>, op: &Operand) -> Reg;
    /// El registro donde calcular el valor de `dst`.
    fn def_operand(&mut self, out: &mut Vec<AsmLine>, dst: &Operand) -> Reg;
    /// Después de calcular: si `dst` es variable o memoria, el sw.
    fn finish_def(&mut self, out: &mut Vec<AsmLine>, dst: &Operand, reg: Reg);
    fn end_instr(&mut self, out: &mut Vec<AsmLine>);
    /// En cada etiqueta y después de cada salto.
    fn block_boundary(&mut self, out: &mut Vec<AsmLine>);
    /// Antes de un call: guarda los $t vivos después de la llamada.
    fn before_call(&mut self, out: &mut Vec<AsmLine>);
    /// Después de un call: los valores cacheados de variables ya no sirven.
    fn after_call(&mut self);
    /// Los $s que usó la función (para el prólogo y el epílogo).
    fn saved_registers(&self) -> Vec<Reg>;
    fn stats(&self) -> FunctionStats;
}

/// `lw`/`sw`… con el registro y la dirección ya formateada.
pub fn instr(out: &mut Vec<AsmLine>, op: &'static str, args: &[&dyn std::fmt::Display]) {
    out.push(AsmLine::Instr { op, args: args.iter().map(|a| a.to_string()).collect() });
}

/// Carga una constante (`Int`, `Bool`, `Null`, `Str`, `Addr`) en `reg`.
/// Devuelve `false` si `op` no es una constante.
pub fn load_const(out: &mut Vec<AsmLine>, strings: &StringTable, op: &Operand, reg: Reg) -> bool {
    match op {
        Operand::Int(n) => instr(out, "li", &[&reg, n]),
        Operand::Bool(b) => instr(out, "li", &[&reg, &(*b as i32)]),
        Operand::Null => instr(out, "li", &[&reg, &0]),
        Operand::Str(s) => instr(out, "la", &[&reg, &strings.label(s)]),
        Operand::Addr(name) => instr(out, "la", &[&reg, &vtable_label(name)]),
        Operand::Float(_) => unreachable!("un Float se rechaza con M001 antes de traducir"),
        _ => return false,
    }
    true
}

/// Asignación por pila: cada operando se carga en `$t0`/`$t1`/`$t2` (rotando)
/// justo antes de usarlo y cada resultado se guarda en seguida en su ranura o
/// su variable. No usa `$s`.
pub struct StackAlloc {
    strings: Rc<StringTable>,
    frame_size: usize,
    siguiente: u8,
}

impl StackAlloc {
    pub fn new(strings: Rc<StringTable>) -> Self {
        StackAlloc { strings, frame_size: 0, siguiente: 0 }
    }

    fn rotar(&mut self) -> Reg {
        let r = Reg::T(self.siguiente);
        self.siguiente = (self.siguiente + 1) % 3;
        r
    }

    /// `fp-(frame_size + 8·(k+1))`: la ranura del temporal `tk`.
    fn slot(&self, k: usize) -> String {
        format!("-{}($fp)", self.frame_size + TEMP_SLOT * (k + 1))
    }

    /// La dirección de memoria de una `Var` o `Mem`, lista para `lw`/`sw`.
    /// Para `tk[off]` carga `tk` en `$t9` primero.
    fn direccion(&self, out: &mut Vec<AsmLine>, op: &Operand) -> String {
        let (base, offset) = match op {
            Operand::Var { base, offset, .. } | Operand::Mem { base, offset, .. } => (base, *offset),
            _ => unreachable!("no es una dirección: {op}"),
        };
        match base {
            Base::Fp => format!("{offset}($fp)"),
            Base::Global if offset == 0 => "G".to_string(),
            Base::Global => format!("G+{offset}"),
            Base::Temp(k) => {
                instr(out, "lw", &[&AUX_BASE, &self.slot(*k)]);
                format!("{offset}({AUX_BASE})")
            }
        }
    }
}

impl RegAlloc for StackAlloc {
    fn begin_function(&mut self, f: &TacFunction) {
        self.frame_size = f.frame_size;
    }

    fn begin_instr(&mut self, _index: usize) {
        self.siguiente = 0;
    }

    fn use_operand(&mut self, out: &mut Vec<AsmLine>, op: &Operand) -> Reg {
        let r = self.rotar();
        match op {
            Operand::Temp(k) => instr(out, "lw", &[&r, &self.slot(*k)]),
            Operand::Var { .. } | Operand::Mem { .. } => {
                let dir = self.direccion(out, op);
                instr(out, "lw", &[&r, &dir]);
            }
            _ => {
                load_const(out, &self.strings, op, r);
            }
        }
        r
    }

    fn def_operand(&mut self, _out: &mut Vec<AsmLine>, _dst: &Operand) -> Reg {
        self.rotar()
    }

    fn finish_def(&mut self, out: &mut Vec<AsmLine>, dst: &Operand, reg: Reg) {
        match dst {
            Operand::Temp(k) => instr(out, "sw", &[&reg, &self.slot(*k)]),
            Operand::Var { .. } | Operand::Mem { .. } => {
                let dir = self.direccion(out, dst);
                instr(out, "sw", &[&reg, &dir]);
            }
            _ => unreachable!("no se puede escribir en {dst}"),
        }
    }

    fn end_instr(&mut self, _out: &mut Vec<AsmLine>) {}
    fn block_boundary(&mut self, _out: &mut Vec<AsmLine>) {}
    fn before_call(&mut self, _out: &mut Vec<AsmLine>) {}
    fn after_call(&mut self) {}

    fn saved_registers(&self) -> Vec<Reg> {
        Vec::new()
    }

    fn stats(&self) -> FunctionStats {
        FunctionStats::default()
    }
}
