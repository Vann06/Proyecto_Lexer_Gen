//! Declaraciones duplicadas y variables/parámetros nunca leídos.
//! No conoce tokens ni producciones: recibe símbolos y ámbitos ya resueltos.

use super::errors::{Diagnostic, ErrorKind, Severity};
use super::scopes::ScopeKind;
use super::symbols::{SemanticError, Symbol, SymbolKind};

/// Solo se consulta el ámbito actual: ocultar un nombre exterior es válido.
/// Conserva el diagnóstico S001 existente y la posición de la primera declaración.
pub fn validate_declaration(
    existing: Option<&Symbol>,
    name: &str,
    line: usize,
    col: usize,
) -> Result<(), SemanticError> {
    if let Some(first) = existing {
        return Err(SemanticError::Redeclared {
            name: name.to_string(),
            line,
            col,
            first_line: first.line,
            first_col: first.col,
        });
    }
    Ok(())
}

/// Se llama al cerrar un ámbito (o al terminar el global), nunca al declarar:
/// una lectura posterior o desde una closure todavía puede usar el símbolo.
/// Los campos de clases/structs quedan fuera: pueden usarse desde otro objeto
/// después de cerrar el ámbito de su tipo.
pub fn unused_diagnostics<'a>(
    kind: ScopeKind,
    symbols: impl Iterator<Item = &'a Symbol>,
) -> Vec<Diagnostic> {
    if matches!(kind, ScopeKind::Class | ScopeKind::Struct) {
        return Vec::new();
    }
    let mut diagnostics: Vec<_> = symbols
        .filter(|symbol| !symbol.used)
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Variable | SymbolKind::Parameter))
        .map(|symbol| Diagnostic {
            kind: ErrorKind::Ambito,
            code: "W001".to_string(),
            message: format!(
                "{} '{}' declarad{} pero nunca leíd{}",
                if symbol.kind == SymbolKind::Parameter {
                    "parámetro"
                } else {
                    "variable"
                },
                symbol.name,
                if symbol.kind == SymbolKind::Parameter {
                    "o"
                } else {
                    "a"
                },
                if symbol.kind == SymbolKind::Parameter {
                    "o"
                } else {
                    "a"
                },
            ),
            line: symbol.line,
            col: symbol.col,
            severity: Severity::Warning,
        })
        .collect();
    diagnostics.sort_by(|a, b| (a.line, a.col, &a.message).cmp(&(b.line, b.col, &b.message)));
    diagnostics
}
