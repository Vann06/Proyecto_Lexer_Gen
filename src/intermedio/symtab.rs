//! Vista extendida de la tabla de símbolos para la Fase 3.
//!
//! Este módulo no asigna almacenamiento. Solo reúne la información que ya
//! calcularon el análisis semántico, `LayoutReport`, `extend_layout` y el
//! programa TAC, y la presenta en formatos estables de texto y JSON.

use std::collections::HashSet;
use std::fmt::Write;

use serde::Serialize;
use serde_json::Value;

use crate::semantico::analyzer::AnalysisResult;
use crate::semantico::scopes::{ScopeKind, ScopeSnapshot};
use crate::semantico::storage::TEMP_SLOT_BYTES;
use crate::semantico::symbols::{Symbol, SymbolKind};

use super::tac::TacProgram;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtendedSymbolTable {
    pub symbols: Vec<SymbolEntry>,
    pub frames: Vec<FrameEntry>,
    pub classes: Vec<ClassEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolEntry {
    pub name: String,
    pub kind: String,
    pub ty: Option<String>,
    pub scope: String,
    pub area: Option<String>,
    pub offset: Option<isize>,
    pub size: Option<usize>,
    pub label: Option<String>,
    pub nesting_level: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameEntry {
    pub owner: String,
    pub level: usize,
    pub base_frame_size: usize,
    pub temp_slots: usize,
    pub total_frame_size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassEntry {
    pub name: String,
    pub kind: String,
    pub instance_size: usize,
    pub fields: Vec<ClassFieldEntry>,
    pub vtable_name: Option<String>,
    pub vtable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClassFieldEntry {
    pub name: String,
    pub ty: Option<String>,
    pub offset: isize,
    pub size: usize,
}

impl ExtendedSymbolTable {
    /// Construye la vista después de generar TAC y llamar a `extend_layout`.
    /// Los tamaños y offsets se copian; aquí no se ejecuta ningún asignador.
    pub fn build(analysis: &AnalysisResult, program: &TacProgram) -> Self {
        let mut symbols = Vec::new();
        let mut snapshots: Vec<&ScopeSnapshot> = analysis.scopes.snapshots().iter().collect();
        snapshots.sort_by_key(|scope| scope.order);

        for scope in &snapshots {
            let mut declared: Vec<&Symbol> = scope.symbols.iter().collect();
            declared.sort_by_key(|symbol| symbol.decl_index);
            let scope_name = scope_name(scope, &snapshots, analysis);
            for symbol in declared {
                symbols.push(symbol_entry(symbol, scope, &scope_name, program));
            }
        }

        let mut frames = Vec::new();
        if let Some(main) = &program.main {
            let temp_slots = analysis
                .layout
                .main_temp_slots
                .unwrap_or(main.max_temps);
            frames.push(FrameEntry {
                owner: main.name.clone(),
                level: main.level,
                base_frame_size: main.frame_size,
                temp_slots,
                total_frame_size: main.frame_size + temp_slots * TEMP_SLOT_BYTES,
            });
        }

        let mut function_frames: Vec<_> = analysis.layout.functions.iter().collect();
        function_frames.sort_by(|(left_id, left), (right_id, right)| {
            left.name
                .cmp(&right.name)
                .then_with(|| left_id.cmp(right_id))
        });
        for (scope_id, owner) in function_frames {
            let Some(&base_frame_size) = analysis.layout.frames.get(scope_id) else {
                continue;
            };
            let temp_slots = owner
                .temp_slots
                .or_else(|| {
                    program
                        .functions
                        .iter()
                        .find(|function| function.name == owner.name)
                        .map(|function| function.max_temps)
                })
                .unwrap_or(0);
            frames.push(FrameEntry {
                owner: owner.name.clone(),
                level: owner.level,
                base_frame_size,
                temp_slots,
                total_frame_size: base_frame_size + temp_slots * TEMP_SLOT_BYTES,
            });
        }

        let mut classes = Vec::new();
        for scope in snapshots {
            if !matches!(scope.kind, ScopeKind::Class | ScopeKind::Struct) {
                continue;
            }
            let Some(name) = scope.label.as_ref() else {
                continue;
            };
            let Some(instance_size) = analysis.layout.classes.instance_size(name) else {
                continue;
            };

            // La vista de una clase muestra el objeto completo. Los campos
            // heredados se copian desde los símbolos definitivos del padre;
            // sus offsets ya fueron asignados por storage y no se recalculan.
            let mut fields = Vec::new();
            collect_class_fields(name, analysis, &mut HashSet::new(), &mut fields);
            fields.sort_by_key(|field| field.offset);

            let vtable_name = if scope.kind == ScopeKind::Class {
                let expected = format!("vt_{name}");
                program
                    .data
                    .iter()
                    .find(|item| item.name == expected)
                    .map(|item| item.name.clone())
            } else {
                None
            };
            let vtable = vtable_name
                .as_ref()
                .and_then(|vtable| program.data.iter().find(|item| &item.name == vtable))
                .map(|item| item.entries.clone())
                // Si se consulta antes de materializar datos TAC, todavía se
                // puede mostrar la vtable semántica existente, sin rehacerla.
                .unwrap_or_else(|| {
                    analysis
                        .layout
                        .classes
                        .vtable(name)
                        .unwrap_or_default()
                        .to_vec()
                });

            classes.push(ClassEntry {
                name: name.clone(),
                kind: scope_kind_name(scope.kind).to_lowercase(),
                instance_size,
                fields,
                vtable_name,
                vtable,
            });
        }
        classes.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            symbols,
            frames,
            classes,
        }
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("ExtendedSymbolTable siempre es serializable")
    }

    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("ExtendedSymbolTable siempre es serializable")
    }

    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str("SÍMBOLOS\n");
        out.push_str("nombre | clase | tipo | ámbito | área | offset | size | label | nivel\n");
        for symbol in &self.symbols {
            let _ = writeln!(
                out,
                "{} | {} | {} | {} | {} | {} | {} | {} | {}",
                symbol.name,
                symbol.kind,
                optional(&symbol.ty),
                symbol.scope,
                optional(&symbol.area),
                signed(symbol.offset),
                number(symbol.size),
                optional(&symbol.label),
                number(symbol.nesting_level),
            );
        }

        out.push_str("\nFRAMES\n");
        out.push_str("owner | nivel | base | temporales | total\n");
        for frame in &self.frames {
            let _ = writeln!(
                out,
                "{} | {} | {} | {} | {}",
                frame.owner,
                frame.level,
                frame.base_frame_size,
                frame.temp_slots,
                frame.total_frame_size,
            );
        }

        out.push_str("\nCLASES\n");
        for class in &self.classes {
            let _ = writeln!(
                out,
                "{} {}: {} bytes; vtable {} [{}]",
                class.kind,
                class.name,
                class.instance_size,
                optional(&class.vtable_name),
                class.vtable.join(", "),
            );
            for field in &class.fields {
                let _ = writeln!(
                    out,
                    "  {}: {} obj{:+} size {}",
                    field.name,
                    optional(&field.ty),
                    field.offset,
                    field.size,
                );
            }
        }
        out
    }
}

fn collect_class_fields(
    class_name: &str,
    analysis: &AnalysisResult,
    visited: &mut HashSet<String>,
    fields: &mut Vec<ClassFieldEntry>,
) {
    if !visited.insert(class_name.to_string()) {
        return;
    }
    let Some(class) = analysis.table.lookup_global(class_name) else {
        return;
    };
    if let Some(parent) = &class.parent {
        collect_class_fields(parent, analysis, visited, fields);
    }
    for symbol in class
        .members
        .iter()
        .flatten()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Variable))
    {
        let Some(storage) = &symbol.storage else {
            continue;
        };
        fields.push(ClassFieldEntry {
            name: symbol.name.clone(),
            ty: symbol.ty.as_ref().map(ToString::to_string),
            offset: storage.offset,
            size: storage.size_bytes,
        });
    }
}

fn symbol_entry(
    symbol: &Symbol,
    scope: &ScopeSnapshot,
    scope_name: &str,
    program: &TacProgram,
) -> SymbolEntry {
    let storage = symbol.storage.as_ref();
    let class_vtable = if matches!(symbol.kind, SymbolKind::Class) {
        let expected = format!("vt_{}", symbol.name);
        program
            .data
            .iter()
            .find(|item| item.name == expected)
            .map(|item| item.name.clone())
    } else {
        None
    };
    SymbolEntry {
        name: symbol.name.clone(),
        kind: symbol_kind_name(&symbol.kind, scope.kind),
        ty: symbol.ty.as_ref().map(ToString::to_string),
        scope: scope_name.to_string(),
        area: storage
            .map(|storage| storage.area.base().to_string())
            .or_else(|| {
                matches!(symbol.kind, SymbolKind::Class | SymbolKind::Struct)
                    .then(|| "heap".to_string())
            }),
        offset: storage.map(|storage| storage.offset),
        size: storage
            .map(|storage| storage.size_bytes)
            .or(symbol.storage_size),
        label: symbol.label.clone().or(class_vtable),
        nesting_level: symbol.nesting_level,
    }
}

fn symbol_kind_name(kind: &SymbolKind, scope: ScopeKind) -> String {
    match kind {
        SymbolKind::Variable if matches!(scope, ScopeKind::Class | ScopeKind::Struct) => {
            "campo".into()
        }
        SymbolKind::Variable => "variable".into(),
        SymbolKind::Parameter => "parámetro".into(),
        SymbolKind::Function if scope == ScopeKind::Class => "método".into(),
        SymbolKind::Function => "función".into(),
        SymbolKind::Class => "clase".into(),
        SymbolKind::Struct => "struct".into(),
        SymbolKind::Other(name) => name.clone(),
    }
}

fn scope_name(
    scope: &ScopeSnapshot,
    snapshots: &[&ScopeSnapshot],
    analysis: &AnalysisResult,
) -> String {
    if scope.kind == ScopeKind::Global {
        return "Global".into();
    }
    if let Some(owner) = analysis.layout.functions.get(&scope.id) {
        return owner.name.clone();
    }
    if let Some(label) = &scope.label {
        return label.clone();
    }

    let mut parent_id = scope.parent_id;
    let mut lexical_owner = None;
    while let Some(id) = parent_id {
        if let Some(owner) = analysis.layout.functions.get(&id) {
            lexical_owner = Some(owner.name.clone());
            break;
        }
        let Some(parent) = snapshots.iter().find(|candidate| candidate.id == id) else {
            break;
        };
        if let Some(label) = &parent.label {
            lexical_owner = Some(label.clone());
            break;
        }
        parent_id = parent.parent_id;
    }

    let own = format!(
        "{}#{}@{}:{}",
        scope_kind_name(scope.kind),
        scope.id,
        scope.line,
        scope.col
    );
    lexical_owner.map_or(own.clone(), |owner| format!("{own} (en {owner})"))
}

fn scope_kind_name(kind: ScopeKind) -> String {
    match kind {
        ScopeKind::Global => "Global",
        ScopeKind::Function => "Function",
        ScopeKind::Class => "Class",
        ScopeKind::Struct => "Struct",
        ScopeKind::Block => "Block",
    }
    .into()
}

fn optional(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("—")
}

fn number(value: Option<usize>) -> String {
    value.map_or_else(|| "—".into(), |value| value.to_string())
}

fn signed(value: Option<isize>) -> String {
    value.map_or_else(|| "—".into(), |value| format!("{value:+}"))
}
