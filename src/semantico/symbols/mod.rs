// Tabla de símbolos (Fase 15): declare/lookup sobre la pila de entornos de
// `super::scopes`. Este módulo es la POLÍTICA semántica — qué está
// permitido (redeclarar en un scope distinto sí, en el mismo no; buscar de
// adentro hacia afuera) — construida sobre la mecánica de apilar/desapilar
// que provee `ScopeStack`.
use thiserror::Error;

use super::scopes::{PopGlobalScope, Scope, ScopeKind, ScopeStack};
use super::types::{resolve_assignment, Coercion};

pub use super::types::Type;

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

/// El tipo de dato de un símbolo. Primitivos comunes a casi cualquier
/// lenguaje imperativo + `Named` como escape hatch para tipos definidos por
/// el usuario (una clase, un alias) — misma idea que `SymbolKind::Other`.
/// `Unknown` es el estado antes de que una futura fase de inferencia lo
/// resuelva; no es "sin tipo", es "todavía no lo sabemos".
/// Firma de una función: tipos de los parámetros, en orden, y tipo de
/// retorno — lo que hace falta para chequear una llamada (aridad y tipos)
/// contra su declaración.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub params: Vec<Type>,
    pub returns: Type,
}

/// Dónde vive un símbolo en tiempo de ejecución: offset relativo al marco
/// de activación (puede ser negativo según la convención) y su tamaño en
/// bytes. El libro del dragón (cap. 7, "Run-Time Environments") trata esto
/// como parte central de la tabla de símbolos — lo llena una futura fase de
/// asignación de almacenamiento, no el walker semántico actual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageInfo {
    pub offset: isize,
    pub size_bytes: usize,
}

/// Un símbolo declarado: su nombre, qué clase de símbolo es, dónde se
/// declaró (para mensajes de error y para que un futuro `Redeclared` pueda
/// señalar la declaración original), y el resto de los atributos que pide
/// el libro del dragón — todos opcionales/con default porque `declare()`
/// solo conoce nombre+kind+posición en el momento en que se declara; lo
/// demás se llena después vía `SymbolTable::lookup_mut` (tipo inferido,
/// firma completa una vez procesados los parámetros, etc.).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
    pub col: usize,
    pub ty: Option<Type>,
    pub mutable: bool,
    pub initialized: bool,
    pub used: bool,
    pub signature: Option<Signature>,
    pub storage: Option<StorageInfo>,
    /// Los símbolos declarados DIRECTAMENTE en el scope que este símbolo
    /// abrió (campos/métodos de una clase, parámetros de una función) — se
    /// llenan solos cuando `analyzer::walk` cierra ese scope, no hace falta
    /// volver a recorrer el árbol para consultar "qué tiene Foo". No se
    /// aplana transitivamente: un local declarado dentro de un bloque
    /// anidado dentro del cuerpo de una función NO aparece acá, solo lo que
    /// cuelga directo del scope de la función misma (ver el comentario en
    /// `analyzer::walk` sobre por qué).
    pub members: Option<Vec<Symbol>>,
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

    #[error("la constante '{name}' debe inicializarse al declararse")]
    ConstRequiresInitializer {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("no se puede asignar a la constante '{name}'")]
    AssignmentToConst {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("asignación incompatible para '{name}': se esperaba {expected}, se encontró {found}")]
    AssignmentTypeMismatch {
        name: String,
        expected: Type,
        found: Type,
        line: usize,
        col: usize,
    },
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

    /// Cierra el scope actual y lo devuelve (para que quien llama pueda
    /// adjuntar sus símbolos a algún otro, p.ej. `analyzer::walk` los pega
    /// como `members` del símbolo que abrió este scope). Nunca se puede
    /// cerrar el Global — ver `ScopeStack::exit`.
    pub fn exit_scope(&mut self) -> Result<Scope, SemanticError> {
        Ok(self.stack.exit()?)
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
            ty: None,
            mutable: true,
            initialized: false,
            used: false,
            signature: None,
            storage: None,
            members: None,
        });
        Ok(())
    }

    /// Declara un símbolo con tipo y mutabilidad conocidos. Una constante
    /// siempre debe incluir inicializador, y el inicializador se valida con
    /// la misma tabla de coerciones usada por `assign`.
    pub fn declare_typed(
        &mut self,
        name: &str,
        kind: SymbolKind,
        ty: Type,
        mutable: bool,
        initializer_type: Option<Type>,
        line: usize,
        col: usize,
    ) -> Result<(), SemanticError> {
        if !mutable && initializer_type.is_none() {
            return Err(SemanticError::ConstRequiresInitializer {
                name: name.to_string(),
                line,
                col,
            });
        }
        if let Some(found) = &initializer_type {
            resolve_assignment(&ty, found).map_err(|_| SemanticError::AssignmentTypeMismatch {
                name: name.to_string(),
                expected: ty.clone(),
                found: found.clone(),
                line,
                col,
            })?;
        }

        self.declare(name, kind, line, col)?;
        let symbol = self
            .lookup_mut(name)
            .expect("el símbolo acaba de declararse");
        symbol.ty = Some(ty);
        symbol.mutable = mutable;
        symbol.initialized = initializer_type.is_some();
        Ok(())
    }

    /// Verifica y registra una asignación contra el tipo declarado.
    pub fn assign(
        &mut self,
        name: &str,
        value_type: &Type,
        line: usize,
        col: usize,
    ) -> Result<Coercion, SemanticError> {
        let symbol = self.lookup_or_err(name, line, col)?;
        if !symbol.mutable {
            return Err(SemanticError::AssignmentToConst {
                name: name.to_string(),
                line,
                col,
            });
        }
        let expected = symbol.ty.clone().unwrap_or(Type::Unknown);
        let coercion = resolve_assignment(&expected, value_type).map_err(|_| {
            SemanticError::AssignmentTypeMismatch {
                name: name.to_string(),
                expected: expected.clone(),
                found: value_type.clone(),
                line,
                col,
            }
        })?;
        self.lookup_mut(name)
            .expect("el símbolo ya fue encontrado")
            .initialized = true;
        Ok(coercion)
    }

    /// Busca `name` empezando por el scope actual y subiendo hacia el
    /// Global — el primero que lo tenga declarado gana (shadowing).
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        self.stack.iter_innermost_first().find_map(|scope| scope.get_own(name))
    }

    /// Igual que `lookup`, pero mutable — la forma de llenar los atributos
    /// que `declare()` no puede conocer todavía (tipo inferido, firma
    /// completa, `used`, `storage`) sin tener que reconstruir el símbolo.
    pub fn lookup_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.stack.iter_innermost_first_mut().find_map(|scope| scope.get_own_mut(name))
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
                out.push_str(&format!("{indent}    {}: {} @{}:{}\n", sym.name, describe(sym), sym.line, sym.col));
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

/// Descripción de un símbolo para `dump()`: el `kind` siempre, más
/// anotaciones solo cuando se apartan del default (tiene tipo, es const,
/// se marcó como usado) — así un símbolo recién declarado (el caso común en
/// los tests existentes) se ve igual que antes de agregar estos campos.
fn describe(sym: &Symbol) -> String {
    let mut parts = vec![format!("{:?}", sym.kind)];
    if let Some(ty) = &sym.ty {
        parts.push(format!("{ty:?}"));
    }
    if !sym.mutable {
        parts.push("const".to_string());
    }
    if sym.used {
        parts.push("usado".to_string());
    }
    parts.join(", ")
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

    #[test]
    fn lookup_mut_allows_filling_attributes_declared_after_the_fact() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        assert_eq!(t.lookup("x").unwrap().ty, None, "sin tipo hasta que se infiera");

        t.lookup_mut("x").unwrap().ty = Some(Type::Int);
        t.lookup_mut("x").unwrap().used = true;

        let sym = t.lookup("x").unwrap();
        assert_eq!(sym.ty, Some(Type::Int));
        assert!(sym.used);
    }

    #[test]
    fn lookup_mut_respects_innermost_first_like_lookup() {
        let mut t = SymbolTable::new();
        t.declare("x", SymbolKind::Variable, 1, 1).unwrap();
        t.enter_scope(ScopeKind::Function);
        t.declare("x", SymbolKind::Parameter, 2, 5).unwrap();

        // Debe mutar el "x" del Function (el más cercano), no el de Global.
        t.lookup_mut("x").unwrap().mutable = false;

        assert!(!t.lookup("x").unwrap().mutable);
        t.exit_scope().unwrap();
        assert!(t.lookup("x").unwrap().mutable, "el x de Global no debía tocarse");
    }

    #[test]
    fn lookup_mut_on_undeclared_name_returns_none() {
        let mut t = SymbolTable::new();
        assert!(t.lookup_mut("fantasma").is_none());
    }

    #[test]
    fn describe_only_annotates_attributes_that_differ_from_default() {
        // Un símbolo recién declarado (el caso común) no debe traer ruido
        // extra en dump() — solo lo agregado cuando de verdad se usó.
        assert_eq!(
            describe(&Symbol {
                name: "x".to_string(),
                kind: SymbolKind::Variable,
                line: 1,
                col: 1,
                ty: None,
                mutable: true,
                initialized: false,
                used: false,
                signature: None,
                storage: None,
                members: None,
            }),
            "Variable"
        );
        assert_eq!(
            describe(&Symbol {
                name: "x".to_string(),
                kind: SymbolKind::Variable,
                line: 1,
                col: 1,
                ty: Some(Type::Int),
                mutable: false,
                initialized: true,
                used: true,
                signature: None,
                storage: None,
                members: None,
            }),
            "Variable, Int, const, usado"
        );
    }
}
