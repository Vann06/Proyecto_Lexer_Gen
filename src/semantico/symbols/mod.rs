// Tabla de símbolos (Fase 15): declare/lookup sobre la pila de entornos de
// `super::scopes`. Este módulo es la POLÍTICA semántica — qué está
// permitido (redeclarar en un scope distinto sí, en el mismo no; buscar de
// adentro hacia afuera) — construida sobre la mecánica de apilar/desapilar
// que provee `ScopeStack`.
use thiserror::Error;

use super::scopes::{PopGlobalScope, Scope, ScopeKind, ScopeStack};

/// Qué clase de cosa es un símbolo declarado. Cuatro variantes fijas para
/// los casos obvios (coinciden con lo que un `ScopeKind::Function`/`Class`
/// va a necesitar declarar) más un escape hatch `Other` para lo que una
/// gramática concreta necesite sin tener que tocar este enum — misma idea
/// que `ParseNode.symbol: String` es agnóstico al nombre exacto de cada
/// no-terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolKind {
    Variable,
    Parameter,
    Function,
    Class,
    Other(String),
}

/// Un símbolo declarado: su nombre, qué clase de símbolo es, y dónde se
/// declaró (para mensajes de error y para que un futuro `Redeclared` pueda
/// señalar la declaración original).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SemanticError {
    #[error(
        "'{name}' ya fue declarada en este ámbito (línea {first_line}:{first_col})"
    )]
    Redeclared {
        name: String,
        line: usize,
        col: usize,
        first_line: usize,
        first_col: usize,
    },

    #[error("variable '{name}' no declarada")]
    Undeclared { name: String, line: usize, col: usize },

    #[error("no se puede cerrar el ámbito global")]
    PopGlobalScope,
}

impl From<PopGlobalScope> for SemanticError {
    fn from(_: PopGlobalScope) -> Self {
        SemanticError::PopGlobalScope
    }
}

/// La tabla de símbolos: entornos anidados + declare/lookup con las reglas
/// semánticas de la rúbrica. Agnóstica a cualquier gramática concreta — no
/// sabe qué no-terminal de un `.yalp` dado representa una declaración; eso
/// lo decide quien recorra el `ParseNode` y llame a `declare`/`lookup`.
pub struct SymbolTable {
    stack: ScopeStack,
}

impl SymbolTable {
    pub fn new() -> Self {
        SymbolTable { stack: ScopeStack::new() }
    }

    pub fn enter_scope(&mut self, kind: ScopeKind) {
        self.stack.enter(kind, None);
    }

    pub fn enter_scope_named(&mut self, kind: ScopeKind, label: impl Into<String>) {
        self.stack.enter(kind, Some(label.into()));
    }

    /// Cierra el scope actual. Nunca se puede cerrar el Global — ver
    /// `ScopeStack::exit`.
    pub fn exit_scope(&mut self) -> Result<(), SemanticError> {
        self.stack.exit()?;
        Ok(())
    }

    /// Declara `name` en el scope ACTUAL. Rechaza la redeclaración dentro de
    /// ese mismo scope (con la posición de la declaración original en el
    /// error), pero no toca los scopes exteriores — declarar el mismo
    /// nombre en un scope anidado distinto es shadowing válido, no error.
    pub fn declare(
        &mut self,
        name: &str,
        kind: SymbolKind,
        line: usize,
        col: usize,
    ) -> Result<(), SemanticError> {
        if let Some(existing) = self.stack.current().get_own(name) {
            return Err(SemanticError::Redeclared {
                name: name.to_string(),
                line,
                col,
                first_line: existing.line,
                first_col: existing.col,
            });
        }
        self.stack.insert_in_current(Symbol {
            name: name.to_string(),
            kind,
            line,
            col,
        });
        Ok(())
    }

    /// Busca `name` empezando por el scope actual y subiendo hacia el
    /// Global — el primero que lo tenga declarado gana (shadowing).
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.stack.iter_innermost_first().find_map(|scope| scope.get_own(name))
    }

    /// Igual que `lookup`, pero devuelve el `SemanticError::Undeclared` que
    /// pide la rúbrica en vez de `None` cuando no se encuentra.
    pub fn lookup_or_err(&self, name: &str, line: usize, col: usize) -> Result<&Symbol, SemanticError> {
        self.lookup(name).ok_or_else(|| SemanticError::Undeclared {
            name: name.to_string(),
            line,
            col,
        })
    }

    pub fn depth(&self) -> usize {
        self.stack.depth()
    }

    pub fn current_scope_kind(&self) -> ScopeKind {
        self.stack.current().kind()
    }

    /// Vuelca el estado completo de la tabla, un bloque por entorno activo,
    /// de afuera (Global) hacia adentro (el scope actual) — mismo espíritu
    /// ASCII que `sintactico::runtime::parse_tree::print_ascii`.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for (depth, scope) in self.stack.iter_outermost_first().enumerate() {
            let indent = "  ".repeat(depth);
            out.push_str(&format!("{indent}[{depth}] {}", scope_header(scope)));
            out.push('\n');

            let mut symbols: Vec<&Symbol> = scope.symbols().collect();
            symbols.sort_by(|a, b| a.name.cmp(&b.name));
            for sym in symbols {
                out.push_str(&format!(
                    "{indent}    {}: {:?} @{}:{}\n",
                    sym.name, sym.kind, sym.line, sym.col
                ));
            }
        }
        out
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

fn scope_header(scope: &Scope) -> String {
    match scope.label() {
        Some(label) => format!("{:?}({label})", scope.kind()),
        None => format!("{:?}", scope.kind()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declare_then_lookup_finds_it() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        let found = t.lookup("x").expect("x fue declarada");
        assert_eq!(found.name, "x");
        assert_eq!(found.kind, SymbolKind::Variable);
        assert_eq!((found.line, found.col), (1, 1));
    }

    #[test]
    fn lookup_undeclared_variable_errors() {
        let t = SymbolTable::new();
        assert_eq!(t.lookup("y"), None);
        let err = t.lookup_or_err("y", 3, 7).unwrap_err();
        assert_eq!(
            err,
            SemanticError::Undeclared { name: "y".to_string(), line: 3, col: 7 }
        );
    }

    #[test]
    fn redeclare_in_same_scope_is_rejected() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        let err = t.declare("x", SymbolKind::Variable, 5, 2).unwrap_err();
        // El error debe apuntar a la declaración ORIGINAL, no a la nueva.
        assert_eq!(
            err,
            SemanticError::Redeclared {
                name: "x".to_string(),
                line: 5,
                col: 2,
                first_line: 1,
                first_col: 1,
            }
        );
    }

    #[test]
    fn redeclare_in_different_nested_scope_is_allowed_shadowing() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        t.enter_scope(ScopeKind::Function);
        // Mismo nombre, scope DISTINTO — no es redeclaración, es shadowing.
        t.declare("x", SymbolKind::Parameter, 2, 5).unwrap();
        assert_eq!(t.lookup("x").unwrap().kind, SymbolKind::Parameter);
    }

    #[test]
    fn lookup_from_nested_block_finds_outer_scope_symbol() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        t.enter_scope(ScopeKind::Function);
        t.enter_scope(ScopeKind::Block);
        // "x" no se redeclaró aquí adentro — el lookup debe atravesar
        // Block -> Function -> Global y encontrarla.
        let found = t.lookup("x").expect("x visible desde el bloque anidado");
        assert_eq!(found.line, 1);
    }

    #[test]
    fn shadowing_lookup_returns_innermost_first() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        t.enter_scope(ScopeKind::Function);
        t.declare("x", SymbolKind::Parameter, 2, 5).unwrap();
        t.enter_scope(ScopeKind::Block);
        // Ningún "x" declarado en Block — debe ganar el de Function (el más
        // cercano), no el de Global.
        let found = t.lookup("x").unwrap();
        assert_eq!(found.kind, SymbolKind::Parameter);
        assert_eq!((found.line, found.col), (2, 5));
    }

    #[test]
    fn exit_scope_removes_inner_declarations_from_lookup() {
        let mut t = SymbolTable::new();
        t.enter_scope(ScopeKind::Block);
        t.declare("y", SymbolKind::Variable, 3, 3).unwrap();
        assert!(t.lookup("y").is_some());

        t.exit_scope().unwrap();
        assert_eq!(t.lookup("y"), None, "y era local al bloque que ya se cerró");
    }

    #[test]
    fn cannot_pop_global_scope() {
        let mut t = SymbolTable::new();
        assert_eq!(t.exit_scope().unwrap_err(), SemanticError::PopGlobalScope);
        // El error no debe haber corrompido el estado — sigue en Global, profundidad 1.
        assert_eq!(t.depth(), 1);
        assert_eq!(t.current_scope_kind(), ScopeKind::Global);
    }

    #[test]
    fn dump_reflects_each_active_scope() {
        let mut t = SymbolTable::new();
        t.declare("g", SymbolKind::Variable, 1, 1).unwrap();
        t.enter_scope_named(ScopeKind::Function, "foo");
        t.declare("a", SymbolKind::Parameter, 2, 10).unwrap();
        t.enter_scope(ScopeKind::Block);
        t.declare("y", SymbolKind::Variable, 3, 5).unwrap();

        let dump = t.dump();
        assert!(dump.contains("[0] Global"));
        assert!(dump.contains("g: Variable @1:1"));
        assert!(dump.contains("[1] Function(foo)"));
        assert!(dump.contains("a: Parameter @2:10"));
        assert!(dump.contains("[2] Block"));
        assert!(dump.contains("y: Variable @3:5"));
    }
}
