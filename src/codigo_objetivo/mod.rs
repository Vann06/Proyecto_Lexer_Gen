//! Generación de código objetivo: del TAC (`intermedio`) a ensamblador MIPS
//! para MARS. El plan, el reparto y las decisiones están en `PLAN_MIPS.md`.
//!
//! Nombre elegido para no chocar con dos módulos que también se llaman
//! "codegen" pero son otra cosa:
//!   - `lexico::codegen::rust_codegen` — emite el LEXER standalone en Rust,
//!     no código objetivo del programa fuente.
//!   - `api::codegen` — el handler HTTP que expone lo anterior.
//!
//! ## Cómo se arma
//!
//! `generate` recorre `main` y cada función. Por instrucción deja el TAC como
//! comentario y llama al traductor (`instr::translate`); lo que ningún
//! traductor cubre da `M900`, y un `float` da `M001`. Los errores se
//! acumulan: si hay alguno, no se devuelve `.asm`.

pub mod asm;
pub mod calls;
pub mod data;
pub mod instr;
pub mod names;
pub mod regs;
pub mod runtime;

use std::fmt;
use std::rc::Rc;

use crate::intermedio::gen::CodegenError;
use crate::intermedio::tac::{BinOp, Instr, Operand, PrintKind, TacFunction, TacProgram, UnOp};

use asm::{AsmLine, FunctionStats, MipsProgram, Reg};
use names::StringTable;
use regs::{RegAlloc, StackAlloc};

/// Qué asignación de registros usar (para compararlas, `PLAN_MIPS.md` §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allocator {
    Pila,
    GetReg,
}

/// Lo que comparten los tres traductores: las líneas que se van emitiendo y
/// el asignador de registros. `out` y `alloc` son campos públicos para poder
/// prestarlos a la vez (`fe.alloc.use_operand(&mut fe.out, op)`).
pub struct FunctionEmitter {
    pub out: Vec<AsmLine>,
    pub alloc: Box<dyn RegAlloc>,
    pub strings: Rc<StringTable>,
}

impl FunctionEmitter {
    pub fn emit(&mut self, op: &'static str, args: &[&dyn fmt::Display]) {
        regs::instr(&mut self.out, op, args);
    }

    pub fn label(&mut self, name: &str) {
        self.out.push(AsmLine::Label(name.to_string()));
    }

    pub fn comment(&mut self, text: impl Into<String>) {
        self.out.push(AsmLine::Comment(text.into()));
    }

    pub fn use_op(&mut self, op: &Operand) -> Reg {
        self.alloc.use_operand(&mut self.out, op)
    }

    pub fn def_op(&mut self, dst: &Operand) -> Reg {
        self.alloc.def_operand(&mut self.out, dst)
    }

    pub fn finish_def(&mut self, dst: &Operand, reg: Reg) {
        self.alloc.finish_def(&mut self.out, dst, reg);
    }
}

/// El `.asm` del programa. Usa `Allocator::GetReg` (hoy igual a `Pila`).
pub fn generate(program: &TacProgram) -> Result<MipsProgram, Vec<CodegenError>> {
    generate_with(program, Allocator::GetReg)
}

pub fn generate_with(program: &TacProgram, allocator: Allocator) -> Result<MipsProgram, Vec<CodegenError>> {
    match generate_partial(program, allocator) {
        (mips, errors) if errors.is_empty() => Ok(mips),
        (_, errors) => Err(errors),
    }
}

/// Como `generate_with`, pero devuelve siempre el `.asm` junto con los errores
/// (las instrucciones sin traducir quedan como comentario `M900`). Sirve para
/// inspeccionar y probar mientras falten traducciones; no es para entregar.
pub fn generate_partial(program: &TacProgram, allocator: Allocator) -> (MipsProgram, Vec<CodegenError>) {
    let strings = Rc::new(StringTable::from_program(program));
    let mut mips = MipsProgram { data: data::data_section(program, &strings, runtime::data_section()), ..Default::default() };
    let mut errors = Vec::new();
    for f in program.main.iter().chain(&program.functions) {
        // ponytail: GetReg cae a Pila hasta la Fase 4 (liveness.rs + GetRegAlloc).
        let alloc: Box<dyn RegAlloc> = match allocator {
            Allocator::Pila | Allocator::GetReg => Box::new(StackAlloc::new(strings.clone())),
        };
        let mut fe = FunctionEmitter { out: Vec::new(), alloc, strings: strings.clone() };
        let stats = emit_function(&mut fe, f, &mut errors);
        mips.text.extend(fe.out);
        mips.stats.push(FunctionStats { funcion: names::label_of(f), ..stats });
    }
    mips.text.extend(runtime::text_section(program));
    (mips, errors)
}

fn emit_function(fe: &mut FunctionEmitter, f: &TacFunction, errors: &mut Vec<CodegenError>) -> FunctionStats {
    fe.alloc.begin_function(f);
    let mut i = 0;
    while i < f.body.len() {
        // Una llamada consume su grupo completo. Así los parámetros se
        // escriben después de reservar el marco del llamado y `before_call`
        // ocurre exactamente una vez, en el índice de la llamada.
        if matches!(f.body[i], Instr::Param(_) | Instr::Link(_) | Instr::Call { .. } | Instr::CallV { .. }) {
            let end = (i..f.body.len()).find(|&j| matches!(f.body[j], Instr::Call { .. } | Instr::CallV { .. }));
            if let Some(end) = end {
                let simple = f.body[i..end].iter().all(|x| matches!(x, Instr::Param(_) | Instr::Link(_)));
                let translated = if simple {
                    calls::translate_call_group(fe, &f.body[i..=end], end)
                } else {
                    calls::translate_virtual_group(fe, &f.body[i..=end], i)
                };
                if translated {
                    i = end + 1;
                    continue;
                }
            }
        }

        let instr = &f.body[i];
        fe.alloc.begin_instr(i);
        if !matches!(instr, Instr::Label(_)) {
            fe.comment(instr.to_string());
        }
        if usa_float(instr) {
            errors.push(CodegenError::new("M001", format!("operación de punto flotante no soportada en MIPS: `{instr}` (en `{}`)", f.name), 0, 0));
        } else if !traducir(fe, instr, &names::ret_label(f)) {
            fe.comment(format!("M900: sin traducir"));
            errors.push(CodegenError::new("M900", format!("instrucción TAC sin traducir a MIPS: `{instr}` (en `{}`)", f.name), 0, 0));
        }
        fe.alloc.end_instr(&mut fe.out);
        i += 1;
    }
    let stats = fe.alloc.stats();
    calls::wrap_function(fe, f);
    stats
}

/// Único punto donde se enchufan los traductores: B agrega aquí `calls` y
/// `runtime` (`instr::translate(..) || calls::translate(..) || …`).
fn traducir(fe: &mut FunctionEmitter, instr: &Instr, return_label: &str) -> bool {
    instr::translate(fe, instr) || calls::translate(fe, instr, return_label) || runtime::translate(fe, instr)
}

/// `float` no existe en la gramática (`PLAN_MIPS.md` §4.8): cualquier
/// instrucción de punto flotante da `M001`.
fn usa_float(instr: &Instr) -> bool {
    let op_float = match instr {
        Instr::Binary { op, .. } => matches!(op, BinOp::AddF | BinOp::SubF | BinOp::MulF | BinOp::DivF),
        Instr::Unary { op, .. } => matches!(op, UnOp::NegF | UnOp::IntToFloat),
        Instr::Print { kind, .. } => *kind == PrintKind::Float,
        _ => false,
    };
    op_float || instr.operands().into_iter().any(|o| matches!(o, Operand::Float(_)))
}
