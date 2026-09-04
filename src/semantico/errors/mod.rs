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

use crate::semantico::flow::FlowError;
use crate::semantico::functions::FunctionError;
use crate::semantico::operators::OperatorError;
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
            SemanticError::InvalidArithmetic { line, col, .. } => (ErrorKind::Tipos, "S015", *line, *col),
            SemanticError::StructFieldTypeMismatch { line, col, .. } => (ErrorKind::Tipos, "S022", *line, *col),
            SemanticError::MissingStructField { line, col, .. } => (ErrorKind::Tipos, "S023", *line, *col),
            SemanticError::DuplicateStructField { line, col, .. } => (ErrorKind::Tipos, "S024", *line, *col),
            SemanticError::HeterogeneousArrayElements { line, col, .. } => (ErrorKind::Listas, "S032", *line, *col),
            SemanticError::IndexNotInteger { line, col, .. } => (ErrorKind::Listas, "S033", *line, *col),
            SemanticError::NotIndexable { line, col, .. } => (ErrorKind::Listas, "S034", *line, *col),
            SemanticError::NotIterable { line, col, .. } => (ErrorKind::Listas, "S036", *line, *col),
            SemanticError::MapKeyTypeMismatch { line, col, .. } => (ErrorKind::Listas, "S037", *line, *col),
            SemanticError::TupleIndexOutOfRange { line, col, .. } => (ErrorKind::Listas, "S038", *line, *col),
            // Mismo código que `FunctionError::NotCallable`: es el mismo error
            // visto desde el otro enum, igual que S013/S014 más abajo.
            SemanticError::NotCallable { line, col, .. } => (ErrorKind::Funciones, "S020", *line, *col),
        };
        Diagnostic { kind, code: code.to_string(), message, line, col, severity: Severity::Error }
    }
}

/// Mapea cada variante de `FunctionError` a su categoría y código. `S016..S019`
/// son de retorno y `S020..S021` de invocabilidad; todas
/// `ErrorKind::Funciones`, igual que `S013`/`S014`.
///
/// `ArityMismatch`/`ArgumentTypeMismatch` reusan a propósito `S013`/`S014`: son
/// literalmente el mismo error que `SemanticError::CallArityMismatch`/
/// `CallArgTypeMismatch`, con el mismo mensaje, solo que producido desde el
/// lado de `functions` en vez del de `classes`. Un evaluador que vea el código
/// no debería tener que saber cuál de los dos módulos lo emitió.
///
/// `NotCallable`/`MissingSignature` todavía no las produce nadie (el walker
/// resuelve el llamado por su cuenta antes de validar), pero el `match` es
/// exhaustivo a propósito: así agregar una variante sin asignarle código es un
/// error de compilación y no un diagnóstico que se pierde en silencio.
impl From<&FunctionError> for Diagnostic {
    fn from(err: &FunctionError) -> Self {
        // Un error que en realidad venía de la tabla de símbolos conserva su
        // código original — no se le inventa uno nuevo por haber pasado por
        // `functions`.
        if let FunctionError::Symbol(inner) = err {
            return Diagnostic::from(inner);
        }

        let message = err.to_string();
        let (kind, code, line, col) = match err {
            FunctionError::Symbol(_) => unreachable!("tratado arriba"),
            FunctionError::ArityMismatch { line, col, .. } => (ErrorKind::Funciones, "S013", *line, *col),
            FunctionError::ArgumentTypeMismatch { line, col, .. } => (ErrorKind::Funciones, "S014", *line, *col),
            FunctionError::ReturnTypeMismatch { line, col, .. } => (ErrorKind::Funciones, "S016", *line, *col),
            FunctionError::MissingReturnValue { line, col, .. } => (ErrorKind::Funciones, "S017", *line, *col),
            FunctionError::UnexpectedReturnValue { line, col, .. } => (ErrorKind::Funciones, "S018", *line, *col),
            FunctionError::ReturnOutsideFunction { line, col } => (ErrorKind::Funciones, "S019", *line, *col),
            FunctionError::NotCallable { line, col, .. } => (ErrorKind::Funciones, "S020", *line, *col),
            FunctionError::MissingSignature { line, col, .. } => (ErrorKind::Funciones, "S021", *line, *col),
        };
        Diagnostic { kind, code: code.to_string(), message, line, col, severity: Severity::Error }
    }
}

impl From<&FlowError> for Diagnostic {
    fn from(err: &FlowError) -> Self {
        let (code, line, col) = match err {
            FlowError::ConditionNotBoolean { line, col, .. } => ("S025", *line, *col),
            FlowError::BreakOutsideLoop { line, col } => ("S026", *line, *col),
            FlowError::ContinueOutsideLoop { line, col } => ("S027", *line, *col),
            FlowError::CaseTypeMismatch { line, col, .. } => ("S035", *line, *col),
        };
        Diagnostic {
            kind: ErrorKind::ControlFlujo,
            code: code.to_string(),
            message: err.to_string(),
            line,
            col,
            severity: Severity::Error,
        }
    }
}

/// `S028..S031` son de expresiones binarias/unarias (`operators`). Van a
/// `ErrorKind::Tipos` y no a una categoría propia porque los cuatro son
/// fallos de compatibilidad de tipos: qué operandos admite un operador, y qué
/// puede aparecer como operando.
impl From<&OperatorError> for Diagnostic {
    fn from(err: &OperatorError) -> Self {
        let (code, line, col) = match err {
            OperatorError::LogicalOperandNotBoolean { line, col, .. } => ("S028", *line, *col),
            OperatorError::IncompatibleComparison { line, col, .. } => ("S029", *line, *col),
            OperatorError::UnaryOperandMismatch { line, col, .. } => ("S030", *line, *col),
            OperatorError::NonValueOperand { line, col, .. } => ("S031", *line, *col),
        };
        Diagnostic {
            kind: ErrorKind::Tipos,
            code: code.to_string(),
            message: err.to_string(),
            line,
            col,
            severity: Severity::Error,
        }
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

    /// Igual que `push_semantic`, para los errores que produce `functions`
    /// (aridad/tipos de una invocacion y validacion de `return`).
    pub fn push_function(&mut self, err: &FunctionError) {
        self.push(Diagnostic::from(err));
    }

    pub fn push_flow(&mut self, err: &FlowError) {
        self.push(Diagnostic::from(err));
    }

    /// Igual que los anteriores, para `operators` (expresiones binarias y
    /// unarias, y el sentido semántico de un operando).
    pub fn push_operator(&mut self, err: &OperatorError) {
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
