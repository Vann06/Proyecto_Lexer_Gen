//! La asignación de registros. `RegAlloc` es la interfaz que usan los
//! traductores (`instr.rs`, `calls.rs`, `runtime.rs`); `StackAlloc` es la
//! primera implementación: cada temporal vive siempre en su ranura del marco.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::rc::Rc;

use crate::intermedio::tac::{Base, Operand, TacFunction, TEMP_SLOT};

use super::asm::{AsmLine, FunctionStats, Reg};
use super::liveness::Liveness;
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
    out.push(AsmLine::Instr {
        op,
        args: args.iter().map(|a| a.to_string()).collect(),
    });
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
        StackAlloc {
            strings,
            frame_size: 0,
            siguiente: 0,
        }
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
            Operand::Var { base, offset, .. } | Operand::Mem { base, offset, .. } => {
                (base, *offset)
            }
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegisterValue {
    Temp(usize),
    Var(String),
    Scratch,
}

/// Asignador local basado en descriptores de registros/direcciones y vida de
/// temporales (`PLAN_MIPS.md`, Fase 4).
pub struct GetRegAlloc {
    strings: Rc<StringTable>,
    frame_size: usize,
    current: usize,
    liveness: Liveness,
    registers: HashMap<Reg, RegisterValue>,
    temp_register: HashMap<usize, Reg>,
    temp_in_memory: HashMap<usize, bool>,
    var_register: HashMap<String, Reg>,
    pinned: HashSet<Reg>,
    used_registers: HashSet<Reg>,
    used_saved: BTreeSet<u8>,
    spills: usize,
}

impl GetRegAlloc {
    pub fn new(strings: Rc<StringTable>) -> Self {
        GetRegAlloc {
            strings,
            frame_size: 0,
            current: 0,
            liveness: Liveness::default(),
            registers: HashMap::new(),
            temp_register: HashMap::new(),
            temp_in_memory: HashMap::new(),
            var_register: HashMap::new(),
            pinned: HashSet::new(),
            used_registers: HashSet::new(),
            used_saved: BTreeSet::new(),
            spills: 0,
        }
    }

    fn slot(&self, temp: usize) -> String {
        format!("-{}($fp)", self.frame_size + TEMP_SLOT * (temp + 1))
    }

    fn var_key(op: &Operand) -> Option<String> {
        match op {
            Operand::Var {
                name,
                base: Base::Fp,
                offset,
            } => Some(format!("fp:{offset}:{name}")),
            Operand::Var {
                name,
                base: Base::Global,
                offset,
            } => Some(format!("g:{offset}:{name}")),
            _ => None,
        }
    }

    fn memory_address(&mut self, out: &mut Vec<AsmLine>, op: &Operand) -> String {
        let (base, offset) = match op {
            Operand::Var { base, offset, .. } | Operand::Mem { base, offset, .. } => {
                (base, *offset)
            }
            _ => unreachable!("no es una dirección: {op}"),
        };
        match base {
            Base::Fp => format!("{offset}($fp)"),
            Base::Global if offset == 0 => "G".to_string(),
            Base::Global => format!("G+{offset}"),
            Base::Temp(temp) => {
                let base = self.ensure_temp(out, *temp);
                self.pinned.insert(base);
                format!("{offset}({base})")
            }
        }
    }

    fn pool(saved: bool) -> Vec<Reg> {
        if saved {
            (0..8).map(Reg::S).collect()
        } else {
            (0..8).map(Reg::T).collect()
        }
    }

    fn allocate(&mut self, out: &mut Vec<AsmLine>, saved: bool) -> Reg {
        let pool = Self::pool(saved);
        if let Some(reg) = pool
            .iter()
            .copied()
            .find(|r| !self.registers.contains_key(r))
        {
            self.mark_used(reg);
            return reg;
        }

        // Una variable siempre tiene copia actual en memoria: es la víctima
        // más barata. Primero evita registros usados por la instrucción actual.
        if let Some(reg) = pool.iter().copied().find(|r| {
            !self.pinned.contains(r)
                && matches!(
                    self.registers.get(r),
                    Some(RegisterValue::Var(_) | RegisterValue::Scratch)
                )
        }) {
            self.forget_register(reg);
            self.mark_used(reg);
            return reg;
        }

        let choose_temp = |allow_pinned: bool, this: &Self| {
            pool.iter()
                .copied()
                .filter(|r| allow_pinned || !this.pinned.contains(r))
                .filter_map(|r| match this.registers.get(&r) {
                    Some(RegisterValue::Temp(temp)) => Some((
                        r,
                        *temp,
                        this.liveness
                            .next_use(*temp, this.current)
                            .unwrap_or(usize::MAX),
                    )),
                    _ => None,
                })
                .max_by_key(|(_, _, next)| *next)
        };
        if let Some((reg, temp, _)) = choose_temp(false, self).or_else(|| choose_temp(true, self)) {
            self.spill_for_pressure(out, temp, reg);
            self.mark_used(reg);
            return reg;
        }

        // Solo ocurre con muchos parámetros de una misma llamada: los
        // anteriores ya fueron copiados al marco del llamado.
        let reg = pool[0];
        self.forget_register(reg);
        self.mark_used(reg);
        reg
    }

    fn mark_used(&mut self, reg: Reg) {
        self.used_registers.insert(reg);
        if let Reg::S(n) = reg {
            self.used_saved.insert(n);
        }
    }

    fn forget_register(&mut self, reg: Reg) {
        match self.registers.remove(&reg) {
            Some(RegisterValue::Temp(temp)) => {
                self.temp_register.remove(&temp);
            }
            Some(RegisterValue::Var(key)) => {
                self.var_register.remove(&key);
            }
            _ => {}
        }
        self.pinned.remove(&reg);
    }

    fn spill_for_pressure(&mut self, out: &mut Vec<AsmLine>, temp: usize, reg: Reg) {
        let live = self
            .liveness
            .live_out
            .get(self.current)
            .is_some_and(|set| set.contains(&temp));
        if live && !self.temp_in_memory.get(&temp).copied().unwrap_or(false) {
            out.push(AsmLine::Comment(format!("derrame t{temp}")));
            instr(out, "sw", &[&reg, &self.slot(temp)]);
            self.temp_in_memory.insert(temp, true);
            self.spills += 1;
        }
        self.forget_register(reg);
    }

    fn bind_temp(&mut self, out: &mut Vec<AsmLine>, temp: usize, reg: Reg) {
        self.forget_register(reg);
        self.registers.insert(reg, RegisterValue::Temp(temp));
        self.temp_register.insert(temp, reg);
        self.mark_used(reg);
        out.push(AsmLine::Comment(format!("t{temp} -> {reg}")));
    }

    fn ensure_temp(&mut self, out: &mut Vec<AsmLine>, temp: usize) -> Reg {
        if let Some(&reg) = self.temp_register.get(&temp) {
            return reg;
        }
        let saved = self.liveness.live_across_calls.contains(&temp);
        let reg = self.allocate(out, saved);
        self.bind_temp(out, temp, reg);
        instr(out, "lw", &[&reg, &self.slot(temp)]);
        self.temp_in_memory.insert(temp, true);
        reg
    }

    fn bind_var(&mut self, key: String, reg: Reg) {
        self.forget_register(reg);
        self.registers.insert(reg, RegisterValue::Var(key.clone()));
        self.var_register.insert(key, reg);
        self.mark_used(reg);
    }

    fn scratch(&mut self, out: &mut Vec<AsmLine>) -> Reg {
        let reg = self.allocate(out, false);
        self.forget_register(reg);
        self.registers.insert(reg, RegisterValue::Scratch);
        self.mark_used(reg);
        reg
    }

    fn spill_live_and_clear(&mut self, out: &mut Vec<AsmLine>) {
        let live = self
            .liveness
            .live_out
            .get(self.current)
            .cloned()
            .unwrap_or_default();
        let temps: Vec<(usize, Reg)> = self.temp_register.iter().map(|(&t, &r)| (t, r)).collect();
        for (temp, reg) in temps {
            if live.contains(&temp) && !self.temp_in_memory.get(&temp).copied().unwrap_or(false) {
                instr(out, "sw", &[&reg, &self.slot(temp)]);
                self.temp_in_memory.insert(temp, true);
            }
        }
        self.registers.clear();
        self.temp_register.clear();
        self.var_register.clear();
        self.pinned.clear();
    }
}

impl RegAlloc for GetRegAlloc {
    fn begin_function(&mut self, f: &TacFunction) {
        self.frame_size = f.frame_size;
        self.current = 0;
        self.liveness = Liveness::analyze(f);
        self.registers.clear();
        self.temp_register.clear();
        self.temp_in_memory.clear();
        self.var_register.clear();
        self.pinned.clear();
        self.used_registers.clear();
        self.used_saved.clear();
        self.spills = 0;
    }

    fn begin_instr(&mut self, index: usize) {
        self.current = index;
        self.pinned.clear();
    }

    fn use_operand(&mut self, out: &mut Vec<AsmLine>, op: &Operand) -> Reg {
        let reg = match op {
            Operand::Temp(temp) => self.ensure_temp(out, *temp),
            Operand::Var { .. } if Self::var_key(op).is_some() => {
                let key = Self::var_key(op).unwrap();
                if let Some(&reg) = self.var_register.get(&key) {
                    reg
                } else {
                    let reg = self.allocate(out, false);
                    let address = self.memory_address(out, op);
                    instr(out, "lw", &[&reg, &address]);
                    self.bind_var(key, reg);
                    reg
                }
            }
            Operand::Var { .. } | Operand::Mem { .. } => {
                let address = self.memory_address(out, op);
                let reg = self.scratch(out);
                instr(out, "lw", &[&reg, &address]);
                reg
            }
            _ => {
                let reg = self.scratch(out);
                load_const(out, &self.strings, op, reg);
                reg
            }
        };
        self.pinned.insert(reg);
        reg
    }

    fn def_operand(&mut self, out: &mut Vec<AsmLine>, dst: &Operand) -> Reg {
        let reg = match dst {
            Operand::Temp(temp) => {
                if let Some(&reg) = self.temp_register.get(temp) {
                    reg
                } else {
                    let saved = self.liveness.live_across_calls.contains(temp);
                    let reg = self.allocate(out, saved);
                    self.bind_temp(out, *temp, reg);
                    reg
                }
            }
            Operand::Var { .. } if Self::var_key(dst).is_some() => {
                let key = Self::var_key(dst).unwrap();
                if let Some(&reg) = self.var_register.get(&key) {
                    reg
                } else {
                    let reg = self.allocate(out, false);
                    self.bind_var(key, reg);
                    reg
                }
            }
            Operand::Var { .. } | Operand::Mem { .. } => self.scratch(out),
            _ => unreachable!("no se puede definir {dst}"),
        };
        if let Operand::Temp(temp) = dst {
            self.temp_in_memory.insert(*temp, false);
        }
        self.pinned.insert(reg);
        reg
    }

    fn finish_def(&mut self, out: &mut Vec<AsmLine>, dst: &Operand, reg: Reg) {
        match dst {
            Operand::Temp(_) => {}
            Operand::Var { .. } | Operand::Mem { .. } => {
                let address = self.memory_address(out, dst);
                instr(out, "sw", &[&reg, &address]);
            }
            _ => unreachable!("no se puede escribir en {dst}"),
        }
    }

    fn end_instr(&mut self, _out: &mut Vec<AsmLine>) {
        let live = self
            .liveness
            .live_out
            .get(self.current)
            .cloned()
            .unwrap_or_default();
        let dead: Vec<(usize, Reg)> = self
            .temp_register
            .iter()
            .filter(|(temp, _)| !live.contains(temp))
            .map(|(&temp, &reg)| (temp, reg))
            .collect();
        for (_, reg) in dead {
            self.forget_register(reg);
        }
        let scratch: Vec<Reg> = self
            .registers
            .iter()
            .filter_map(|(&reg, value)| matches!(value, RegisterValue::Scratch).then_some(reg))
            .collect();
        for reg in scratch {
            self.forget_register(reg);
        }
        self.pinned.clear();
    }

    fn block_boundary(&mut self, out: &mut Vec<AsmLine>) {
        self.spill_live_and_clear(out);
    }

    fn before_call(&mut self, out: &mut Vec<AsmLine>) {
        let temps: Vec<(usize, Reg)> = self.temp_register.iter().map(|(&t, &r)| (t, r)).collect();
        for (temp, reg) in temps {
            if matches!(reg, Reg::T(_)) && !self.temp_in_memory.get(&temp).copied().unwrap_or(false)
            {
                // También cubre los argumentos que `param*` consumirá
                // después de `before_call`: si hay más de ocho, cualquiera
                // puede ser expulsado mientras se llena el marco del llamado.
                out.push(AsmLine::Comment(format!("guarda t{temp} antes de call")));
                instr(out, "sw", &[&reg, &self.slot(temp)]);
                self.temp_in_memory.insert(temp, true);
            }
        }
    }

    fn after_call(&mut self) {
        let volatile: Vec<Reg> = self
            .registers
            .keys()
            .copied()
            .filter(|r| matches!(r, Reg::T(_)))
            .collect();
        for reg in volatile {
            self.forget_register(reg);
        }
        self.var_register.clear();
    }

    fn saved_registers(&self) -> Vec<Reg> {
        self.used_saved.iter().copied().map(Reg::S).collect()
    }

    fn stats(&self) -> FunctionStats {
        FunctionStats {
            registros_usados: self.used_registers.len(),
            derrames: self.spills,
            s_guardados: self.used_saved.len(),
            ..Default::default()
        }
    }
}
