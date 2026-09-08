// Entornos anidados (Fase 15): la pila de scopes (global, función, clase,
// bloque) que sostiene la tabla de símbolos. Este módulo es solo la
// MECÁNICA de apilar/desapilar y guardar — sin política de qué está
// permitido (redeclarar, buscar de adentro hacia afuera, etc.); eso vive
// en `super::symbols::SymbolTable`, que es quien realmente conoce las
// reglas semánticas y usa este stack como estructura de soporte.
use serde_json::{json, Value};
use std::collections::HashMap;

use super::symbols::Symbol;

/// Los cuatro tipos de entorno que pide la tabla de símbolos. Deliberadamente
/// un enum cerrado (no un `String` libre como los símbolos de la gramática
/// en `ParseNode`) porque estos cuatro son estándar de cualquier lenguaje
/// imperativo/orientado a objetos, y tenerlos tipados permite reglas futuras
/// específicas por tipo de entorno (p. ej. "`return` solo es válido dentro
/// de un `Function`") sin andar comparando strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Function,
    Class,
    /// Entorno de un tipo registro (struct). Separado de `Class` porque un
    /// struct no tiene metodos, ni `this`, ni herencia: distinguirlos deja
    /// que las reglas que solo aplican a clases (declarar `this` al abrir un
    /// metodo) no se disparen aca por accidente.
    Struct,
    Block,
}

/// Un solo nivel de anidamiento: su tipo, una etiqueta opcional (p. ej. el
/// nombre de la función o clase, útil para `dump()`) y los símbolos
/// declarados directamente en él (no los de scopes exteriores).
#[derive(Debug, Clone)]
pub struct Scope {
    kind: ScopeKind,
    label: Option<String>,
    /// Posición del nodo que abrió este scope (p. ej. la `{` de un `bloque`,
    /// o el nombre de un `func_decl`/`class_decl`) — sin esto, dos `Block`
    /// anónimos sin declaraciones propias son indistinguibles en la salida:
    /// no hay forma de saber a qué `if`/`while`/cuerpo de función corresponde
    /// cada uno.
    open_line: usize,
    open_col: usize,
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new(kind: ScopeKind, label: Option<String>, open_line: usize, open_col: usize) -> Self {
        Scope { kind, label, open_line, open_col, symbols: HashMap::new() }
    }

    pub fn kind(&self) -> ScopeKind {
        self.kind
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn position(&self) -> (usize, usize) {
        (self.open_line, self.open_col)
    }

    /// Busca `name` SOLO en este scope (no en los exteriores) — de ahí "own".
    pub fn get_own(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }

    /// Igual que `get_own`, mutable — para `SymbolTable::lookup_mut`.
    pub fn get_own_mut(&mut self, name: &str) -> Option<&mut Symbol> {
        self.symbols.get_mut(name)
    }

    pub fn contains_own(&self, name: &str) -> bool {
        self.symbols.contains_key(name)
    }

    /// Símbolos declarados en este scope, sin orden garantizado (respaldado
    /// por un HashMap) — para `dump()` el llamador los ordena si quiere.
    pub fn symbols(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    // Privado a propósito: insertar sin chequear redeclaración es una
    // operación insegura semánticamente — solo `SymbolTable::declare` (que sí
    // chequea) debe poder hacerlo.
    fn insert(&mut self, symbol: Symbol) {
        self.symbols.insert(symbol.name.clone(), symbol);
    }
}

/// Se intentó desapilar el scope global — no hay a dónde volver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopGlobalScope;

/// La pila de entornos activa. `scopes[0]` es siempre el scope Global — se
/// crea junto con el stack y nunca se puede desapilar (ver `exit`).
pub struct ScopeStack {
    scopes: Vec<Scope>,
}

impl ScopeStack {
    pub fn new() -> Self {
        ScopeStack { scopes: vec![Scope::new(ScopeKind::Global, None, 0, 0)] }
    }

    pub fn enter(&mut self, kind: ScopeKind, label: Option<String>, line: usize, col: usize) {
        self.scopes.push(Scope::new(kind, label, line, col));
    }

    /// Desapila el scope actual y lo devuelve (por si el llamador quiere
    /// inspeccionar qué quedó declarado ahí antes de descartarlo). `Err` si
    /// el único scope restante es el Global — el stack queda intacto en ese
    /// caso, no se corrompe el invariante "scopes[0] siempre existe".
    pub fn exit(&mut self) -> Result<Scope, PopGlobalScope> {
        if self.scopes.len() <= 1 {
            return Err(PopGlobalScope);
        }
        Ok(self.scopes.pop().expect("longitud verificada arriba"))
    }

    pub fn current(&self) -> &Scope {
        self.scopes.last().expect("scopes[0] (Global) nunca se desapila")
    }

    pub fn current_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("scopes[0] (Global) nunca se desapila")
    }

    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// De adentro hacia afuera — el orden que necesita `lookup` para que el
    /// símbolo más cercano (el que hace shadowing) gane.
    pub fn iter_innermost_first(&self) -> impl Iterator<Item = &Scope> {
        self.scopes.iter().rev()
    }

    /// Igual que `iter_innermost_first`, pero además da el índice ABSOLUTO de
    /// cada scope en la pila (0 = Global, creciente hacia adentro) — lo que
    /// necesita la resolución de nombres libres para closures: comparar en
    /// qué profundidad se declaró un símbolo contra la profundidad de la
    /// función actual, y así decidir si es una captura o un local normal.
    pub fn iter_innermost_first_with_index(&self) -> impl Iterator<Item = (usize, &Scope)> {
        self.scopes.iter().enumerate().rev()
    }

    /// Igual que `iter_innermost_first`, mutable — para `lookup_mut`.
    pub fn iter_innermost_first_mut(&mut self) -> impl Iterator<Item = &mut Scope> {
        self.scopes.iter_mut().rev()
    }

    /// De afuera hacia adentro — el orden natural para mostrar en `dump()`.
    pub fn iter_outermost_first(&self) -> impl Iterator<Item = &Scope> {
        self.scopes.iter()
    }

    pub(super) fn insert_in_current(&mut self, symbol: Symbol) {
        self.current_mut().insert(symbol);
    }
}

impl Default for ScopeStack {
    fn default() -> Self {
        Self::new()
    }
}

/// Foto de un ámbito en el momento en que se cerró.
///
/// Existe porque la tabla viva no conserva nada de un ámbito cerrado: lo que
/// se declaró dentro de una función o una clase sobrevive anidado en
/// `Symbol::members`, pero **lo declarado dentro de un ámbito ANÓNIMO (un
/// bloque) se descarta**. Sin estas fotos, un `let` dentro de un `if` no
/// aparece en ninguna salida del proyecto, que es justo lo que confunde al
/// mirar el resultado del análisis.
#[derive(Debug, Clone)]
pub struct ScopeSnapshot {
    /// Orden de CIERRE: 1 es el primer ámbito que se cerró, o sea el más
    /// interno de los que se cerraron primero.
    pub order: usize,
    pub kind: ScopeKind,
    pub label: Option<String>,
    /// Profundidad que ocupaba en la pila cuando se cerró (0 = Global).
    pub depth: usize,
    /// Posición de apertura del scope (ver `Scope::open_line`/`open_col`) —
    /// deja identificar CADA `Block` anónimo por su lugar en el código, en
    /// vez de que todos luzcan idénticos cuando no declaran nada propio.
    pub line: usize,
    pub col: usize,
    /// Los símbolos declarados DIRECTAMENTE en él, ordenados por nombre.
    pub symbols: Vec<Symbol>,
}

/// Acumula un `ScopeSnapshot` por cada ámbito que se cierra durante el
/// recorrido — mismo espíritu que `closures::ClosureCollector` y
/// `errors::ErrorCollector`: observa y guarda, sin participar de ninguna
/// decisión.
///
/// **No toca la tabla viva.** Ese era el motivo por el que esto no se había
/// hecho: aplanar los ámbitos anónimos hacia arriba reinsertándolos en la
/// tabla filtraría la visibilidad de esos nombres más allá de su bloque y
/// rompería el `lookup` con scoping correcto. Guardar copias aparte no corre
/// ese riesgo.
///
/// El ámbito Global NUNCA aparece acá: no se cierra nunca, así que se consulta
/// directamente en la tabla al terminar (`SymbolTable::dump`).
#[derive(Debug, Default)]
pub struct ScopeCollector {
    snapshots: Vec<ScopeSnapshot>,
}

impl ScopeCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra un ámbito recién cerrado. `depth` es la profundidad que
    /// ocupaba (la que tenía la pila ANTES de desapilarlo, menos uno).
    pub fn record(&mut self, scope: &Scope, depth: usize) {
        let mut symbols: Vec<Symbol> = scope.symbols().cloned().collect();
        symbols.sort_by(|a, b| a.name.cmp(&b.name));
        let (line, col) = scope.position();
        self.snapshots.push(ScopeSnapshot {
            order: self.snapshots.len() + 1,
            kind: scope.kind(),
            label: scope.label().map(str::to_string),
            depth,
            line,
            col,
            symbols,
        });
    }

    pub fn is_empty(&self) -> bool {
        self.snapshots.is_empty()
    }

    pub fn len(&self) -> usize {
        self.snapshots.len()
    }

    /// En orden de cierre: primero el que se cerró primero.
    pub fn snapshots(&self) -> &[ScopeSnapshot] {
        &self.snapshots
    }

    /// Volcado legible, con el MISMO formato de símbolo que
    /// `SymbolTable::dump()` — se reusan sus helpers en vez de inventar otro.
    /// Forma `[{order, kind, label, depth, symbols:[...]}]` -- la que consume
    /// `/api/pipeline` para la pestana de ambitos del IDE. Mismo criterio que
    /// `ClosureCollector::to_json`: datos estructurados y no el texto del
    /// volcado, para que el frontend pueda ordenar y filtrar sin parsear
    /// cadenas.
    pub fn to_json(&self) -> Vec<Value> {
        self.snapshots
            .iter()
            .map(|snap| {
                json!({
                    "order": snap.order,
                    "kind": format!("{:?}", snap.kind),
                    "label": snap.label,
                    "depth": snap.depth,
                    "line": snap.line,
                    "col": snap.col,
                    "symbols": snap.symbols.iter().map(|sym| json!({
                        "name": sym.name,
                        "kind": format!("{:?}", sym.kind),
                        "ty": sym.ty.as_ref().map(|t| t.to_string()),
                        "mutable": sym.mutable,
                        "initialized": sym.initialized,
                        "line": sym.line,
                        "col": sym.col,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect()
    }

    pub fn dump(&self) -> String {
        let mut out = String::new();
        for snap in &self.snapshots {
            out.push_str(&format!(
                "#{} {} (profundidad {}) @{}:{}
",
                snap.order,
                crate::semantico::symbols::scope_header_of(snap.kind, snap.label.as_deref()),
                snap.depth,
                snap.line,
                snap.col
            ));
            if snap.symbols.is_empty() {
                out.push_str("      (sin declaraciones propias)
");
            }
            for sym in &snap.symbols {
                out.push_str(&format!(
                    "      {}: {} @{}:{}
",
                    sym.name,
                    crate::semantico::symbols::describe(sym),
                    sym.line,
                    sym.col
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_stack_starts_at_global_with_depth_one() {
        let stack = ScopeStack::new();
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current().kind(), ScopeKind::Global);
    }

    #[test]
    fn enter_and_exit_restore_previous_depth_and_kind() {
        let mut stack = ScopeStack::new();
        stack.enter(ScopeKind::Function, Some("foo".to_string()), 1, 1);
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current().kind(), ScopeKind::Function);
        assert_eq!(stack.current().label(), Some("foo"));

        stack.exit().expect("hay un scope Function para desapilar");
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current().kind(), ScopeKind::Global);
    }

    #[test]
    fn collector_records_one_snapshot_per_closed_scope_in_closing_order() {
        let mut stack = ScopeStack::new();
        let mut collector = ScopeCollector::new();

        // Global > Function("f") > Block
        stack.enter(ScopeKind::Function, Some("f".to_string()), 1, 1);
        stack.enter(ScopeKind::Block, None, 2, 3);

        // Se cierran de adentro hacia afuera, que es como los cierra el walker.
        let block = stack.exit().expect("hay Block");
        collector.record(&block, stack.depth());
        let func = stack.exit().expect("hay Function");
        collector.record(&func, stack.depth());

        assert_eq!(collector.len(), 2);
        let snaps = collector.snapshots();

        // El primero en cerrarse es el más interno.
        assert_eq!(snaps[0].order, 1);
        assert_eq!(snaps[0].kind, ScopeKind::Block);
        assert_eq!(snaps[0].label, None);
        assert_eq!(snaps[0].depth, 2, "el Block ocupaba la profundidad 2");
        assert_eq!((snaps[0].line, snaps[0].col), (2, 3), "posición de apertura del Block");

        assert_eq!(snaps[1].order, 2);
        assert_eq!(snaps[1].kind, ScopeKind::Function);
        assert_eq!(snaps[1].label, Some("f".to_string()));
        assert_eq!(snaps[1].depth, 1, "la Function ocupaba la profundidad 1");
        assert_eq!((snaps[1].line, snaps[1].col), (1, 1), "posición de apertura de la Function");

        // El Global no se cierra nunca, así que no puede aparecer.
        assert!(
            !snaps.iter().any(|s| s.kind == ScopeKind::Global),
            "el Global nunca se desapila: se consulta en la tabla, no acá"
        );
        assert_eq!(stack.depth(), 1, "queda solo el Global");
    }

    #[test]
    fn exit_on_global_only_stack_errors_and_leaves_stack_intact() {
        let mut stack = ScopeStack::new();
        assert_eq!(stack.exit().unwrap_err(), PopGlobalScope);
        // El error no debe haber corrompido el invariante.
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current().kind(), ScopeKind::Global);
    }
}
