// Colector de diagnósticos (Fase 15): une los errores de dominio de
// `symbols::SemanticError` bajo una forma de REPORTE única — tipo, línea,
// columna y severidad — que es lo que pide la rúbrica ("Reporte de errores
// semánticos encontrados, con su tipo y ubicación") y lo que ya sabe leer
// `ProblemsList` en el IDE (`{level, code, msg, loc, line, col}`).
//
// `SemanticError` (symbols/mod.rs) sigue siendo el tipo de dominio — lleva los
// campos estructurados que la tabla de símbolos necesita para construir el
// mensaje. `Diagnostic` es la forma de reporte derivada de él; `ErrorCollector`
// los acumula igual que `AnalysisResult.errors` ya hacía con el `Vec` plano.
use serde_json::{json, Value};

use crate::semantico::symbols::SemanticError;

/// Categoría del error, según las familias de reglas que pide la rúbrica del
/// Proyecto 2 (tipos, ámbito, funciones, control de flujo, clases, listas).
/// Solo `Ambito` y `Tipos` tienen productores hoy (las 6 variantes de
/// `SemanticError`); el resto existe para que las próximas fases de reglas
/// semánticas (funciones, control de flujo, clases, listas) tengan dónde
/// clasificar sus propios diagnósticos sin inventar una taxonomía nueva.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Tipos,
    Ambito,
    Funciones,
    ControlFlujo,
    Clases,
    Listas,
    General,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    fn level(&self) -> &'static str {
        match self {
            Severity::Error => "err",
            Severity::Warning => "warn",
            Severity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub kind: ErrorKind,
    pub code: String,
    pub message: String,
    pub line: usize,
    pub col: usize,
    pub severity: Severity,
}

impl Diagnostic {
    /// Forma `{level, code, msg, loc, line, col}` — la que ya lee `ProblemsList`
    /// en `frontend/IDE/app.jsx` sin ningún cambio. `source_name` es el archivo
    /// real (p. ej. `ejemplo.cps`), no un `"input.txt"` fijo — así el gutter del
    /// editor (que compara `loc` contra el nombre del archivo abierto) encuentra
    /// la línea sin importar cómo se llame el archivo cargado.
    pub fn to_problem(&self, source_name: &str) -> Value {
        json!({
            "level": self.severity.level(),
            "code": self.code,
            "msg": self.message,
            "loc": format!("{}:{}:{}", source_name, self.line, self.col),
            "line": self.line,
            "col": self.col,
        })
    }
}

/// Mapea cada variante de `SemanticError` a su categoría y código. `S001..S006`
/// son de ámbito/tipos (Fase 15); `S007..S012` son de clases/objetos
/// (`ErrorKind::Clases`); `S013..S014` son de invocación —a un método o a una
/// función libre— (`ErrorKind::Funciones`). El mapeo es por `match` explícito,
/// no por posición del enum, así que agregar variantes al final nunca reordena
/// un código ya asignado.
impl From<&SemanticError> for Diagnostic {
    fn from(err: &SemanticError) -> Self {
        let message = err.to_string();
        let (kind, code, line, col) = match err {
            SemanticError::Redeclared { line, col, .. } => (ErrorKind::Ambito, "S001", *line, *col),
            SemanticError::Undeclared { line, col, .. } => (ErrorKind::Ambito, "S002", *line, *col),
            SemanticError::PopGlobalScope => (ErrorKind::Ambito, "S003", 0, 0),
            SemanticError::ConstRequiresInitializer { line, col, .. } => {
                (ErrorKind::Tipos, "S004", *line, *col)
            }
            SemanticError::AssignmentToConst { line, col, .. } => (ErrorKind::Tipos, "S005", *line, *col),
            SemanticError::AssignmentTypeMismatch { line, col, .. } => {
                (ErrorKind::Tipos, "S006", *line, *col)
            }
            SemanticError::UnknownClass { line, col, .. } => (ErrorKind::Clases, "S007", *line, *col),
            SemanticError::UnknownParentClass { line, col, .. } => (ErrorKind::Clases, "S008", *line, *col),
            SemanticError::ThisOutsideClass { line, col } => (ErrorKind::Clases, "S009", *line, *col),
            SemanticError::UnknownMember { line, col, .. } => (ErrorKind::Clases, "S010", *line, *col),
            SemanticError::ConstructorArityMismatch { line, col, .. } => (ErrorKind::Clases, "S011", *line, *col),
            SemanticError::ConstructorArgTypeMismatch { line, col, .. } => (ErrorKind::Clases, "S012", *line, *col),
            SemanticError::CallArityMismatch { line, col, .. } => (ErrorKind::Funciones, "S013", *line, *col),
            SemanticError::CallArgTypeMismatch { line, col, .. } => (ErrorKind::Funciones, "S014", *line, *col),
        };
        Diagnostic { kind, code: code.to_string(), message, line, col, severity: Severity::Error }
    }
}

/// Acumula diagnósticos durante un recorrido — lo que `Visitor::enter`/`exit`
/// va llenando en vez de detenerse en el primer error (mismo espíritu que el
/// modo pánico del parser LR: reportar todo lo que se pueda en una pasada).
#[derive(Debug, Default)]
pub struct ErrorCollector {
    diags: Vec<Diagnostic>,
}

impl ErrorCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diag: Diagnostic) {
        self.diags.push(diag);
    }

    /// Atajo para el caso común: convertir y acumular un `SemanticError` tal
    /// como lo devuelve `SymbolTable`.
    pub fn push_semantic(&mut self, err: &SemanticError) {
        self.push(Diagnostic::from(err));
    }

    pub fn is_empty(&self) -> bool {
        self.diags.is_empty()
    }

    pub fn len(&self) -> usize {
        self.diags.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diags.iter()
    }

    /// Serializa todos los diagnósticos a la forma que consume `problems` en
    /// la respuesta HTTP y `ProblemsList` en el IDE.
    pub fn to_problems(&self, source_name: &str) -> Vec<Value> {
        self.diags.iter().map(|d| d.to_problem(source_name)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantico::types::Type;

    #[test]
    fn undeclared_maps_to_ambito_s002() {
        let err = SemanticError::Undeclared { name: "z".into(), line: 3, col: 9 };
        let diag = Diagnostic::from(&err);
        assert_eq!(diag.kind, ErrorKind::Ambito);
        assert_eq!(diag.code, "S002");
        assert_eq!((diag.line, diag.col), (3, 9));
        assert!(diag.message.contains('z'));
    }

    #[test]
    fn redeclared_maps_to_ambito_s001() {
        let err = SemanticError::Redeclared {
            name: "x".into(),
            line: 5,
            col: 2,
            first_line: 1,
            first_col: 1,
        };
        let diag = Diagnostic::from(&err);
        assert_eq!(diag.kind, ErrorKind::Ambito);
        assert_eq!(diag.code, "S001");
        assert_eq!((diag.line, diag.col), (5, 2));
    }

    #[test]
    fn assignment_type_mismatch_maps_to_tipos_s006() {
        let err = SemanticError::AssignmentTypeMismatch {
            name: "x".into(),
            expected: Type::Int,
            found: Type::Str,
            line: 4,
            col: 1,
        };
        let diag = Diagnostic::from(&err);
        assert_eq!(diag.kind, ErrorKind::Tipos);
        assert_eq!(diag.code, "S006");
    }

    #[test]
    fn to_problems_uses_real_source_name_not_a_hardcoded_one() {
        let mut collector = ErrorCollector::new();
        collector.push_semantic(&SemanticError::Undeclared { name: "z".into(), line: 3, col: 9 });

        let problems = collector.to_problems("ejemplo.cps");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0]["level"], "err");
        assert_eq!(problems[0]["code"], "S002");
        assert_eq!(problems[0]["loc"], "ejemplo.cps:3:9");
        assert_eq!(problems[0]["line"], 3);
        assert_eq!(problems[0]["col"], 9);
    }

    #[test]
    fn collector_accumulates_every_error_without_stopping_at_the_first() {
        let mut collector = ErrorCollector::new();
        collector.push_semantic(&SemanticError::Undeclared { name: "a".into(), line: 1, col: 1 });
        collector.push_semantic(&SemanticError::Undeclared { name: "b".into(), line: 2, col: 1 });
        assert_eq!(collector.len(), 2);
        assert!(!collector.is_empty());
    }
}
