//! La sección `.data`: el área estática `G`, las vtables y los textos.

use crate::intermedio::tac::TacProgram;

use super::asm::AsmLine;
use super::names::{func_label, vtable_label, StringTable};

/// La sección `.data` completa. `extra` son líneas que agregan otras partes
/// (los datos `rt_*` de las rutinas de apoyo) al final.
pub fn data_section(program: &TacProgram, strings: &StringTable, extra: Vec<AsmLine>) -> Vec<AsmLine> {
    let mut out = vec![
        AsmLine::Directive(".align 2".into()),
        AsmLine::Label("G".into()),
        AsmLine::Directive(format!(".space {}", program.static_size.max(4))),
    ];
    for item in &program.data {
        out.push(AsmLine::Directive(".align 2".into()));
        out.push(AsmLine::Label(vtable_label(&item.name)));
        for entry in &item.entries {
            out.push(AsmLine::Directive(format!(".word {}", func_label(entry))));
        }
    }
    for (label, directiva) in strings.entries() {
        out.push(AsmLine::Label(label));
        out.push(AsmLine::Directive(directiva));
    }
    out.extend(extra);
    out
}
