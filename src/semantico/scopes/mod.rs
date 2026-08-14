// Entornos anidados (Fase 15): la pila de scopes (global, función, clase,
// bloque) que sostiene la tabla de símbolos. Este módulo es solo la
// MECÁNICA de apilar/desapilar y guardar — sin política de qué está
// permitido (redeclarar, buscar de adentro hacia afuera, etc.); eso vive
// en `super::symbols::SymbolTable`, que es quien realmente conoce las
// reglas semánticas y usa este stack como estructura de soporte.
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
    Block,
}

/// Un solo nivel de anidamiento: su tipo, una etiqueta opcional (p. ej. el
/// nombre de la función o clase, útil para `dump()`) y los símbolos
/// declarados directamente en él (no los de scopes exteriores).
#[derive(Debug, Clone)]
pub struct Scope {
    kind: ScopeKind,
    label: Option<String>,
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new(kind: ScopeKind, label: Option<String>) -> Self {
        Scope { kind, label, symbols: HashMap::new() }
    }

    pub fn kind(&self) -> ScopeKind {
        self.kind
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Busca `name` SOLO en este scope (no en los exteriores) — de ahí "own".
    pub fn get_own(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
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
        ScopeStack { scopes: vec![Scope::new(ScopeKind::Global, None)] }
    }

    pub fn enter(&mut self, kind: ScopeKind, label: Option<String>) {
        self.scopes.push(Scope::new(kind, label));
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
        stack.enter(ScopeKind::Function, Some("foo".to_string()));
        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current().kind(), ScopeKind::Function);
        assert_eq!(stack.current().label(), Some("foo"));

        stack.exit().expect("hay un scope Function para desapilar");
        assert_eq!(stack.depth(), 1);
        assert_eq!(stack.current().kind(), ScopeKind::Global);
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
