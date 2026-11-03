//! Los nombres de las etiquetas del `.asm` (`PLAN_MIPS.md` §4.5). Todo el
//! mapeo de caracteres de los nombres del programa a etiquetas de MARS vive
//! aquí: si MARS no aceptara el `.` en una etiqueta, solo se cambia `mapear`.

use std::collections::HashMap;

use crate::intermedio::tac::{Operand, TacFunction, TacProgram};

/// La etiqueta del código de nivel de programa.
pub const PROGRAM_LABEL: &str = "main";

fn mapear(nombre: &str) -> String {
    nombre.to_string()
}

/// La etiqueta de una función del programa por su nombre en el TAC
/// (`Animal.hablar` → `_Animal.hablar`). Es también el destino de un `call`.
pub fn func_label(nombre: &str) -> String {
    format!("_{}", mapear(nombre))
}

/// La etiqueta de una `TacFunction`: `main` para el nivel de programa (nivel
/// 0), `func_label` para el resto (un `main` del usuario es `_main`).
pub fn label_of(f: &TacFunction) -> String {
    if f.level == 0 { PROGRAM_LABEL.to_string() } else { func_label(&f.name) }
}

/// La etiqueta del epílogo: `_f.0ret`. Empieza con dígito después del punto,
/// igual que `0init`, así que no choca con una función anidada.
pub fn ret_label(f: &TacFunction) -> String {
    format!("{}.0ret", label_of(f))
}

/// La etiqueta de una vtable (el TAC ya la llama `vt_C`).
pub fn vtable_label(nombre: &str) -> String {
    mapear(nombre)
}

/// Los textos del programa: `S0`, `S1`… sin repetir.
#[derive(Debug, Clone, Default)]
pub struct StringTable {
    labels: HashMap<String, String>,
    orden: Vec<String>,
}

impl StringTable {
    /// Todos los textos que aparecen como operando, en orden de aparición
    /// (`main` y luego las funciones).
    pub fn from_program(program: &TacProgram) -> Self {
        let mut t = StringTable::default();
        for f in program.main.iter().chain(&program.functions) {
            for instr in &f.body {
                for op in instr.operands() {
                    if let Operand::Str(s) = op {
                        t.intern(s);
                    }
                }
            }
        }
        t
    }

    /// La etiqueta del texto; la crea si es nuevo.
    pub fn intern(&mut self, s: &str) -> String {
        if let Some(l) = self.labels.get(s) {
            return l.clone();
        }
        let label = format!("S{}", self.orden.len());
        self.labels.insert(s.to_string(), label.clone());
        self.orden.push(s.to_string());
        label
    }

    /// La etiqueta de un texto ya registrado.
    pub fn label(&self, s: &str) -> &str {
        self.labels.get(s).map(String::as_str).unwrap_or_else(|| panic!("texto sin registrar: {s:?}"))
    }

    /// `(etiqueta, directiva)` de cada texto, en orden.
    pub fn entries(&self) -> Vec<(String, String)> {
        self.orden.iter().map(|s| (self.labels[s].clone(), directiva(s))).collect()
    }
}

/// `.asciiz "…"` si el texto es ASCII imprimible (con `"`, `\` y saltos de
/// línea escapados); si no, `.byte` con sus bytes UTF-8 y un `0` al final.
fn directiva(s: &str) -> String {
    if s.bytes().all(|b| b == b'\n' || (0x20..0x7f).contains(&b)) {
        let mut out = String::new();
        for c in s.chars() {
            match c {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                c => out.push(c),
            }
        }
        format!(".asciiz \"{out}\"")
    } else {
        let bytes: Vec<String> = s.bytes().chain(std::iter::once(0)).map(|b| b.to_string()).collect();
        format!(".byte {}", bytes.join(", "))
    }
}
