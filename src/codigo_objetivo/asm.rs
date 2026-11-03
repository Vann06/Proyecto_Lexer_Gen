//! La representación del ensamblador MIPS: registros, líneas y el programa
//! completo. Quien traduce no escribe texto a mano: arma `AsmLine` y este
//! módulo las imprime.

use std::fmt;

use serde_json::{json, Value};

/// Un registro de MIPS. Los usos de cada uno están en `PLAN_MIPS.md` §4.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reg {
    Zero,
    V0,
    V1,
    /// `$a0`–`$a3`.
    A(u8),
    /// `$t0`–`$t9`.
    T(u8),
    /// `$s0`–`$s7`.
    S(u8),
    Sp,
    Fp,
    Ra,
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reg::Zero => write!(f, "$zero"),
            Reg::V0 => write!(f, "$v0"),
            Reg::V1 => write!(f, "$v1"),
            Reg::A(n) => write!(f, "$a{n}"),
            Reg::T(n) => write!(f, "$t{n}"),
            Reg::S(n) => write!(f, "$s{n}"),
            Reg::Sp => write!(f, "$sp"),
            Reg::Fp => write!(f, "$fp"),
            Reg::Ra => write!(f, "$ra"),
        }
    }
}

/// Una línea del `.asm`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsmLine {
    /// `nombre:` (sin los dos puntos).
    Label(String),
    Instr { op: &'static str, args: Vec<String> },
    /// `.word …`, `.asciiz …` (con el punto, sin indentar).
    Directive(String),
    /// Texto del comentario, sin el `#`.
    Comment(String),
}

impl fmt::Display for AsmLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AsmLine::Label(l) => write!(f, "{l}:"),
            AsmLine::Instr { op, args } if args.is_empty() => write!(f, "    {op}"),
            AsmLine::Instr { op, args } => write!(f, "    {op} {}", args.join(", ")),
            AsmLine::Directive(d) => write!(f, "    {d}"),
            AsmLine::Comment(c) => write!(f, "    # {c}"),
        }
    }
}

/// Lo que la asignación de registros cuenta de una función (para la API y el
/// IDE). `StackAlloc` lo deja en ceros; la Fase 4 lo llena.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FunctionStats {
    /// Etiqueta de la función; la pone `generate`, no el asignador.
    pub funcion: String,
    pub registros_usados: usize,
    pub derrames: usize,
    pub s_guardados: usize,
}

/// El programa MIPS: sección `.data`, sección `.text` y estadísticas.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MipsProgram {
    pub data: Vec<AsmLine>,
    pub text: Vec<AsmLine>,
    pub stats: Vec<FunctionStats>,
}

impl fmt::Display for MipsProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, ".data")?;
        for l in &self.data {
            writeln!(f, "{l}")?;
        }
        writeln!(f, "\n.text\n.globl main")?;
        for l in &self.text {
            writeln!(f, "{l}")?;
        }
        Ok(())
    }
}

impl MipsProgram {
    /// `{asm, stats: [{funcion, registros_usados, derrames, s_guardados}]}`.
    pub fn to_json(&self) -> Value {
        json!({
            "asm": self.to_string(),
            "stats": self.stats.iter().map(|s| json!({
                "funcion": s.funcion,
                "registros_usados": s.registros_usados,
                "derrames": s.derrames,
                "s_guardados": s.s_guardados,
            })).collect::<Vec<_>>(),
        })
    }
}
