//! El lenguaje intermedio: código de tres direcciones (TAC) en forma de
//! cuádruplos (§6.2 del libro del dragón).
//!
//! Cada instrucción tiene a lo sumo tres direcciones: dos operandos y un
//! resultado. Este módulo define SOLO la representación —qué instrucciones
//! existen y cómo se escriben—; quién las produce es `gen` y quién las
//! ejecuta es el intérprete. La sintaxis textual que imprime `Display` es la
//! que documenta `CODIGO_INTERMEDIO.md`: si se cambia una, se cambia la otra.
//!
//! ## Cómo se escriben las variables (convención "C" del plan)
//!
//! - Una variable de la función actual o una global se escribe por su
//!   **nombre**, como en el capítulo 6 del libro: `total = 0`. Si dos
//!   variables distintas comparten nombre (una local que tapa a una global),
//!   la segunda lleva un sufijo: `x.1`. Su dirección (`fp[-12]`, `G[0]`) no
//!   se pierde: el operando la guarda y la muestra la tabla de símbolos
//!   extendida.
//! - Una variable que vive en el registro de activación de OTRA función (una
//!   función anidada que usa una variable de la que la encierra) se escribe
//!   por **dirección**, siguiendo los enlaces de acceso de forma explícita:
//!   `t0 = fp[8]` y luego `t1 = t0[-12]`, con el nombre de la variable como
//!   comentario al final de la línea (`; total`). Así se ve el registro de
//!   activación en funcionamiento.
//! - Los temporales (`t0`, `t1`, …) guardan resultados intermedios.
//!
//! Las direcciones usan una base y un desplazamiento en bytes:
//!
//! - `fp[-12]`: relativo al marco actual (`$fp`). `fp[8]` es el enlace de
//!   acceso.
//! - `G[8]`: el área estática.
//! - `t3[4]`: memoria apuntada por un temporal (un objeto, una lista, o el
//!   marco de otra función).

use std::fmt;

use serde_json::{json, Value};

/// La base de una dirección de memoria.
#[derive(Debug, Clone, PartialEq)]
pub enum Base {
    /// El marco de activación actual (`$fp`).
    Fp,
    /// El área de datos estáticos.
    Global,
    /// La dirección que guarda un temporal.
    Temp(usize),
}

/// Un operando de una instrucción.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Temporal `tN`. Lo reparte y recicla `temps::TempPool`.
    Temp(usize),
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Null,
    /// Una variable del programa de la función actual o global: se escribe
    /// por su `name` (ya único: `x`, `x.1`); `base` + `offset` es dónde vive.
    Var { name: String, base: Base, offset: isize },
    /// Una posición de memoria por dirección, con desplazamiento fijo en
    /// bytes. `name` es la variable que representa, si es una (una variable
    /// de otra función alcanzada por enlaces de acceso, o un campo): se
    /// imprime como comentario al final de la línea.
    Mem { base: Base, offset: isize, name: Option<String> },
    /// La dirección de un dato con nombre (p.ej. `&vt_Perro`, la vtable de
    /// una clase).
    Addr(String),
}

impl Operand {
    /// Variable local o parámetro de la función actual.
    pub fn local(name: impl Into<String>, offset: isize) -> Self {
        Operand::Var { name: name.into(), base: Base::Fp, offset }
    }

    /// Variable global (área estática).
    pub fn global(name: impl Into<String>, offset: isize) -> Self {
        Operand::Var { name: name.into(), base: Base::Global, offset }
    }

    /// Memoria por dirección, sin nombre de variable.
    pub fn mem(base: Base, offset: isize) -> Self {
        Operand::Mem { base, offset, name: None }
    }

    /// El temporal que este operando ocupa, si es uno — directo (`t3`) o como
    /// base de una dirección (`t3[4]`). Es lo que `TempPool` libera cuando el
    /// operando se consume.
    pub fn temp(&self) -> Option<usize> {
        match self {
            Operand::Temp(t) | Operand::Mem { base: Base::Temp(t), .. } => Some(*t),
            _ => None,
        }
    }

    /// El nombre de variable que va como comentario: solo para un acceso por
    /// dirección que representa una variable (`t0[-12]  ; total`).
    pub fn comment(&self) -> Option<&str> {
        match self {
            Operand::Mem { name: Some(n), .. } => Some(n),
            _ => None,
        }
    }
}

/// Operaciones binarias. Las aritméticas se distinguen por tipo: el
/// generador elige `AddF` cuando el nodo es `float` (y ya amplió el operando
/// entero con `int2float`), así ninguna instrucción mezcla representaciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    AddF,
    SubF,
    MulF,
    DivF,
    /// Concatenación de textos (`+` entre dos `string`).
    Concat,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
            BinOp::AddF => "+f",
            BinOp::SubF => "-f",
            BinOp::MulF => "*f",
            BinOp::DivF => "/f",
            BinOp::Concat => "concat",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
        }
    }

    /// `true` para las comparaciones: las únicas que pueden ir en un salto
    /// condicional `if x relop y goto L`.
    pub fn is_relational(self) -> bool {
        matches!(self, BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge)
    }
}

/// Operaciones unarias.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    NegF,
    Not,
    /// La ampliación implícita `integer → float` (`widen` del §6.5.2), que el
    /// libro escribe `(float) x`.
    IntToFloat,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Neg => "minus",
            UnOp::NegF => "minusf",
            UnOp::Not => "not",
            UnOp::IntToFloat => "(float)",
        }
    }
}

/// Una instrucción de tres direcciones.
#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    /// `x = y`
    Copy { dst: Operand, src: Operand },
    /// `x = y op z`
    Binary { dst: Operand, op: BinOp, left: Operand, right: Operand },
    /// `x = op y`
    Unary { dst: Operand, op: UnOp, src: Operand },
    /// `L:`
    Label(String),
    /// `goto L`
    Goto(String),
    /// `if x goto L`
    IfTrue { cond: Operand, label: String },
    /// `ifFalse x goto L`
    IfFalse { cond: Operand, label: String },
    /// `if x relop y goto L`
    IfRel { left: Operand, op: BinOp, right: Operand, label: String },
    /// `param x`: un argumento, en orden.
    Param(Operand),
    /// `link x`: el enlace de acceso que recibe la función que se va a llamar.
    Link(Operand),
    /// `x = call f, n` (o `call f, n` si no se usa el resultado).
    Call { dst: Option<Operand>, func: String, nargs: usize },
    /// `x = callv t, n`: llamada indirecta, a la dirección que guarda `t`
    /// (un método sacado de la vtable).
    CallV { dst: Option<Operand>, target: Operand, nargs: usize },
    /// `return x` / `return`
    Return(Option<Operand>),
    /// `x = y[i]`: lee en `y + i` (bytes).
    Load { dst: Operand, base: Operand, index: Operand },
    /// `x[i] = y`: escribe en `x + i` (bytes).
    Store { base: Operand, index: Operand, src: Operand },
    /// `x = alloc n`: reserva `n` bytes en el heap.
    Alloc { dst: Operand, size: Operand },
    /// `x = &nombre`: la dirección de un dato con nombre (una vtable).
    AddrOf { dst: Operand, name: String },
    /// `print x`
    Print(Operand),
    /// `check_bounds a, i`: si `i` está fuera de la lista `a`, error en
    /// tiempo de ejecución.
    CheckBounds { array: Operand, index: Operand },
    /// `check_zero x`: si `x` es cero, error en tiempo de ejecución
    /// (división o módulo entre cero).
    CheckZero(Operand),
    /// `try_begin L`: apila un manejador de errores que salta a `L`.
    TryBegin(String),
    /// `try_end`: desapila el manejador del `try` que termina.
    TryEnd,
    /// `x = exception`: el mensaje del error que activó el manejador.
    GetException(Operand),
}

/// Una función (o el código de nivel de programa) ya traducida.
#[derive(Debug, Clone, PartialEq)]
pub struct TacFunction {
    /// La etiqueta de la función: `f`, `Clase.metodo`, o `main` para el
    /// código de nivel de programa.
    pub name: String,
    /// Nivel de anidamiento estático (0 para `main`; ver
    /// `Symbol::nesting_level`).
    pub level: usize,
    /// Bytes del marco sin temporales (el `frame_size` de `storage`).
    pub frame_size: usize,
    /// Máximo de temporales vivos a la vez (ver `temps::TempPool`).
    pub max_temps: usize,
    /// Cuántos temporales se pidieron en total, antes de reciclar.
    pub temps_requested: usize,
    pub body: Vec<Instr>,
}

/// Bytes de cada ranura de temporal: lo bastante para un `float`.
pub const TEMP_SLOT: usize = 8;

impl TacFunction {
    pub fn new(name: impl Into<String>, level: usize, frame_size: usize) -> Self {
        TacFunction { name: name.into(), level, frame_size, max_temps: 0, temps_requested: 0, body: Vec::new() }
    }

    /// El marco completo: lo que calculó `storage` más el área de
    /// temporales, debajo de los locales.
    pub fn total_frame_size(&self) -> usize {
        self.frame_size + self.max_temps * TEMP_SLOT
    }
}

/// Un dato con nombre del programa: por ahora, la vtable de cada clase (la
/// lista ordenada de las etiquetas de sus métodos).
#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub name: String,
    pub entries: Vec<String>,
}

/// El programa traducido completo.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TacProgram {
    /// Tamaño del área estática (`G`).
    pub static_size: usize,
    pub data: Vec<DataItem>,
    /// El código de nivel de programa, como función `main`.
    pub main: Option<TacFunction>,
    /// Funciones y métodos, en orden de aparición.
    pub functions: Vec<TacFunction>,
}

// ───────────────────────────── sintaxis textual ─────────────────────────────

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Base::Fp => write!(f, "fp"),
            Base::Global => write!(f, "G"),
            Base::Temp(t) => write!(f, "t{t}"),
        }
    }
}

impl fmt::Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Temp(t) => write!(f, "t{t}"),
            Operand::Int(n) => write!(f, "{n}"),
            Operand::Float(x) => write!(f, "{x:?}"),
            Operand::Bool(b) => write!(f, "{b}"),
            Operand::Str(s) => write!(f, "{s:?}"),
            Operand::Null => write!(f, "null"),
            Operand::Var { name, .. } => write!(f, "{name}"),
            Operand::Mem { base, offset, .. } => write!(f, "{base}[{offset}]"),
            Operand::Addr(name) => write!(f, "&{name}"),
        }
    }
}

impl fmt::Display for Instr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instr::Copy { dst, src } => write!(f, "{dst} = {src}"),
            Instr::Binary { dst, op, left, right } => write!(f, "{dst} = {left} {} {right}", op.symbol()),
            Instr::Unary { dst, op, src } => write!(f, "{dst} = {} {src}", op.symbol()),
            Instr::Label(l) => write!(f, "{l}:"),
            Instr::Goto(l) => write!(f, "goto {l}"),
            Instr::IfTrue { cond, label } => write!(f, "if {cond} goto {label}"),
            Instr::IfFalse { cond, label } => write!(f, "ifFalse {cond} goto {label}"),
            Instr::IfRel { left, op, right, label } => write!(f, "if {left} {} {right} goto {label}", op.symbol()),
            Instr::Param(x) => write!(f, "param {x}"),
            Instr::Link(x) => write!(f, "link {x}"),
            Instr::Call { dst: Some(d), func, nargs } => write!(f, "{d} = call {func}, {nargs}"),
            Instr::Call { dst: None, func, nargs } => write!(f, "call {func}, {nargs}"),
            Instr::CallV { dst: Some(d), target, nargs } => write!(f, "{d} = callv {target}, {nargs}"),
            Instr::CallV { dst: None, target, nargs } => write!(f, "callv {target}, {nargs}"),
            Instr::Return(Some(x)) => write!(f, "return {x}"),
            Instr::Return(None) => write!(f, "return"),
            Instr::Load { dst, base, index } => write!(f, "{dst} = {base}[{index}]"),
            Instr::Store { base, index, src } => write!(f, "{base}[{index}] = {src}"),
            Instr::Alloc { dst, size } => write!(f, "{dst} = alloc {size}"),
            Instr::AddrOf { dst, name } => write!(f, "{dst} = &{name}"),
            Instr::Print(x) => write!(f, "print {x}"),
            Instr::CheckBounds { array, index } => write!(f, "check_bounds {array}, {index}"),
            Instr::CheckZero(x) => write!(f, "check_zero {x}"),
            Instr::TryBegin(l) => write!(f, "try_begin {l}"),
            Instr::TryEnd => write!(f, "try_end"),
            Instr::GetException(d) => write!(f, "{d} = exception"),
        }
    }
}

impl Instr {
    /// Los operandos de la instrucción, para quien necesite recorrerlos (el
    /// comentario de variables, el intérprete).
    pub fn operands(&self) -> Vec<&Operand> {
        match self {
            Instr::Copy { dst, src } | Instr::Unary { dst, src, .. } => vec![dst, src],
            Instr::Binary { dst, left, right, .. } => vec![dst, left, right],
            Instr::IfTrue { cond, .. } | Instr::IfFalse { cond, .. } => vec![cond],
            Instr::IfRel { left, right, .. } => vec![left, right],
            Instr::Param(x) | Instr::Link(x) | Instr::Print(x) | Instr::CheckZero(x) | Instr::GetException(x) => vec![x],
            Instr::Call { dst, .. } => dst.iter().collect(),
            Instr::CallV { dst, target, .. } => dst.iter().chain(std::iter::once(target)).collect(),
            Instr::Return(x) => x.iter().collect(),
            Instr::Load { dst, base, index } => vec![dst, base, index],
            Instr::Store { base, index, src } => vec![base, index, src],
            Instr::Alloc { dst, size } => vec![dst, size],
            Instr::AddrOf { dst, .. } => vec![dst],
            Instr::CheckBounds { array, index } => vec![array, index],
            Instr::Label(_) | Instr::Goto(_) | Instr::TryBegin(_) | Instr::TryEnd => vec![],
        }
    }

    /// El comentario de la línea: los nombres de las variables a las que se
    /// accede por dirección (`; total`), o `None` si no hay ninguna.
    pub fn comment(&self) -> Option<String> {
        let names: Vec<&str> = self.operands().into_iter().filter_map(Operand::comment).collect();
        (!names.is_empty()).then(|| names.join(", "))
    }
}

impl fmt::Display for TacFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "func {} nivel {} marco {}", self.name, self.level, self.total_frame_size())?;
        for instr in &self.body {
            // Las etiquetas van al margen; el resto, indentado, con el nombre
            // de las variables accedidas por dirección como comentario.
            let line = match instr {
                Instr::Label(_) => instr.to_string(),
                _ => format!("    {instr}"),
            };
            match instr.comment() {
                Some(c) => writeln!(f, "{line:<28}; {c}")?,
                None => writeln!(f, "{line}")?,
            }
        }
        writeln!(f, "endfunc")
    }
}

impl fmt::Display for TacProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; área estática: {} bytes", self.static_size)?;
        for item in &self.data {
            writeln!(f, "data {}: {}", item.name, item.entries.join(", "))?;
        }
        if !self.data.is_empty() {
            writeln!(f)?;
        }
        if let Some(main) = &self.main {
            write!(f, "{main}")?;
        }
        for func in &self.functions {
            writeln!(f)?;
            write!(f, "{func}")?;
        }
        Ok(())
    }
}

// ─────────────────────────────── forma JSON ───────────────────────────────

/// Forma de un cuádruplo: `{op, arg1, arg2, res, text}`. `text` es la misma
/// instrucción en la sintaxis textual, para mostrarla sin reconstruirla.
pub fn quad_json(instr: &Instr) -> Value {
    let s = |o: &Operand| o.to_string();
    let (op, arg1, arg2, res): (String, Option<String>, Option<String>, Option<String>) = match instr {
        Instr::Copy { dst, src } => ("=".into(), Some(s(src)), None, Some(s(dst))),
        Instr::Binary { dst, op, left, right } => (op.symbol().into(), Some(s(left)), Some(s(right)), Some(s(dst))),
        Instr::Unary { dst, op, src } => (op.symbol().into(), Some(s(src)), None, Some(s(dst))),
        Instr::Label(l) => ("label".into(), None, None, Some(l.clone())),
        Instr::Goto(l) => ("goto".into(), None, None, Some(l.clone())),
        Instr::IfTrue { cond, label } => ("if".into(), Some(s(cond)), None, Some(label.clone())),
        Instr::IfFalse { cond, label } => ("ifFalse".into(), Some(s(cond)), None, Some(label.clone())),
        Instr::IfRel { left, op, right, label } => (format!("if{}", op.symbol()), Some(s(left)), Some(s(right)), Some(label.clone())),
        Instr::Param(x) => ("param".into(), Some(s(x)), None, None),
        Instr::Link(x) => ("link".into(), Some(s(x)), None, None),
        Instr::Call { dst, func, nargs } => ("call".into(), Some(func.clone()), Some(nargs.to_string()), dst.as_ref().map(s)),
        Instr::CallV { dst, target, nargs } => ("callv".into(), Some(s(target)), Some(nargs.to_string()), dst.as_ref().map(s)),
        Instr::Return(x) => ("return".into(), x.as_ref().map(s), None, None),
        Instr::Load { dst, base, index } => ("=[]".into(), Some(s(base)), Some(s(index)), Some(s(dst))),
        Instr::Store { base, index, src } => ("[]=".into(), Some(s(src)), Some(s(index)), Some(s(base))),
        Instr::Alloc { dst, size } => ("alloc".into(), Some(s(size)), None, Some(s(dst))),
        Instr::AddrOf { dst, name } => ("&".into(), Some(name.clone()), None, Some(s(dst))),
        Instr::Print(x) => ("print".into(), Some(s(x)), None, None),
        Instr::CheckBounds { array, index } => ("check_bounds".into(), Some(s(array)), Some(s(index)), None),
        Instr::CheckZero(x) => ("check_zero".into(), Some(s(x)), None, None),
        Instr::TryBegin(l) => ("try_begin".into(), None, None, Some(l.clone())),
        Instr::TryEnd => ("try_end".into(), None, None, None),
        Instr::GetException(d) => ("exception".into(), None, None, Some(s(d))),
    };
    json!({ "op": op, "arg1": arg1, "arg2": arg2, "res": res, "text": instr.to_string(), "comment": instr.comment() })
}

impl TacFunction {
    pub fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "level": self.level,
            "frame_size": self.frame_size,
            "max_temps": self.max_temps,
            "temps_requested": self.temps_requested,
            "total_frame_size": self.total_frame_size(),
            "quads": self.body.iter().map(quad_json).collect::<Vec<_>>(),
        })
    }
}

impl TacProgram {
    pub fn to_json(&self) -> Value {
        json!({
            "static_size": self.static_size,
            "data": self.data.iter().map(|d| json!({ "name": d.name, "entries": d.entries })).collect::<Vec<_>>(),
            "main": self.main.as_ref().map(TacFunction::to_json),
            "functions": self.functions.iter().map(TacFunction::to_json).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_instruccion_se_escribe_con_la_sintaxis_documentada() {
        let t = Operand::Temp;
        let casos: Vec<(Instr, &str)> = vec![
            (Instr::Copy { dst: Operand::local("total", -12), src: Operand::Int(5) }, "total = 5"),
            (Instr::Binary { dst: t(0), op: BinOp::Add, left: Operand::global("x", 0), right: t(1) }, "t0 = x + t1"),
            (Instr::Binary { dst: t(0), op: BinOp::AddF, left: t(0), right: Operand::Float(2.5) }, "t0 = t0 +f 2.5"),
            (Instr::Unary { dst: t(2), op: UnOp::IntToFloat, src: Operand::local("paso", 12) }, "t2 = (float) paso"),
            (Instr::Unary { dst: t(2), op: UnOp::Neg, src: t(1) }, "t2 = minus t1"),
            (Instr::Copy { dst: Operand::local("x.1", -16), src: t(0) }, "x.1 = t0"),
            (Instr::Label("L0".into()), "L0:"),
            (Instr::IfRel { left: t(0), op: BinOp::Lt, right: Operand::Int(10), label: "L1".into() }, "if t0 < 10 goto L1"),
            (Instr::IfFalse { cond: t(0), label: "L2".into() }, "ifFalse t0 goto L2"),
            (Instr::Call { dst: Some(t(0)), func: "fact".into(), nargs: 1 }, "t0 = call fact, 1"),
            (Instr::Call { dst: None, func: "f".into(), nargs: 0 }, "call f, 0"),
            (Instr::CallV { dst: None, target: t(3), nargs: 2 }, "callv t3, 2"),
            (Instr::Load { dst: t(1), base: t(0), index: Operand::Int(4) }, "t1 = t0[4]"),
            (Instr::Store { base: t(0), index: t(1), src: Operand::Str("hola".into()) }, "t0[t1] = \"hola\""),
            (Instr::AddrOf { dst: t(0), name: "vt_Perro".into() }, "t0 = &vt_Perro"),
            (Instr::Copy { dst: t(0), src: Operand::mem(Base::Temp(1), -12) }, "t0 = t1[-12]"),
            (Instr::Copy { dst: t(0), src: Operand::mem(Base::Fp, 8) }, "t0 = fp[8]"),
            (Instr::Return(None), "return"),
        ];
        for (instr, esperado) in casos {
            assert_eq!(instr.to_string(), esperado);
        }
    }

    #[test]
    fn una_variable_de_otra_funcion_se_escribe_por_direccion_con_su_nombre_comentado() {
        // `total = total + paso` dentro de una función anidada (convención C).
        let total = Operand::Mem { base: Base::Temp(0), offset: -12, name: Some("total".into()) };
        let mut f = TacFunction::new("sumar", 2, 16);
        f.body = vec![
            Instr::Copy { dst: Operand::Temp(0), src: Operand::mem(Base::Fp, 8) },
            Instr::Copy { dst: Operand::Temp(1), src: total.clone() },
            Instr::Copy { dst: total, src: Operand::Temp(1) },
        ];
        let texto = f.to_string();
        let lineas: Vec<&str> = texto.lines().collect();
        assert_eq!(lineas[1].trim_end(), "    t0 = fp[8]", "el enlace de acceso no lleva comentario");
        assert!(lineas[2].starts_with("    t1 = t0[-12]") && lineas[2].ends_with("; total"), "{texto}");
        assert!(lineas[3].starts_with("    t0[-12] = t1") && lineas[3].ends_with("; total"), "{texto}");
    }

    #[test]
    fn el_marco_total_suma_las_ranuras_de_temporales() {
        let mut f = TacFunction::new("f", 1, 12);
        f.max_temps = 3;
        assert_eq!(f.total_frame_size(), 12 + 3 * TEMP_SLOT);
        assert!(f.to_string().starts_with("func f nivel 1 marco 36\n"));
    }

    #[test]
    fn el_temporal_de_un_operando_incluye_el_de_una_direccion() {
        assert_eq!(Operand::Temp(4).temp(), Some(4));
        assert_eq!(Operand::mem(Base::Temp(2), 8).temp(), Some(2));
        assert_eq!(Operand::local("x", -8).temp(), None);
    }

    #[test]
    fn el_cuadruplo_json_trae_sus_tres_direcciones_y_el_texto() {
        let instr = Instr::Binary { dst: Operand::Temp(0), op: BinOp::Mul, left: Operand::Temp(1), right: Operand::Int(4) };
        let q = quad_json(&instr);
        assert_eq!(q["op"], "*");
        assert_eq!(q["arg1"], "t1");
        assert_eq!(q["arg2"], "4");
        assert_eq!(q["res"], "t0");
        assert_eq!(q["text"], "t0 = t1 * 4");
    }
}
