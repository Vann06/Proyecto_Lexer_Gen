//! Qué declaración nombra cada identificador del árbol.
//!
//! Durante el recorrido, `SymbolTable::lookup` resuelve cada nombre con el
//! scoping correcto (la `x` de un bloque interno oculta a la de afuera), pero
//! esa respuesta se perdía: al terminar, la pila de ámbitos ya no existe y
//! una segunda pasada no podría repetirla. La fase de código intermedio la
//! necesita en CADA hoja — para emitir `t1 = x + 1` tiene que saber a qué
//! `x` (y por tanto a qué offset de qué marco) se refiere.
//!
//! Mismo diseño que `types::annotations`: un mapa lateral indexado por la
//! identidad del nodo, con la misma clave y la misma invariante (el árbol
//! tiene que seguir vivo, en su lugar y sin clonar). Se registra en el
//! momento en que el ámbito correcto todavía está en la pila.
//!
//! Lo que se guarda no es una copia del `Symbol` —su `storage` todavía no
//! está calculado cuando se usa, se llena al cerrar el ámbito— sino una
//! referencia estable `(scope_id, decl_index)`: `AnalysisResult::resolve`
//! la convierte en el `Symbol` definitivo una vez terminado el análisis.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::sintactico::runtime::parse_tree::ParseNode;

use super::symbols::Symbol;
use super::types::annotations::key;

/// Cómo llega el código de una función al símbolo que nombra — la
/// clasificación del §7.3 del libro para el enlace de acceso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Área estática: declarado en el Global o en un bloque suelto fuera de
    /// toda función. Dirección fija, sin marco.
    Static,
    /// En el marco de la misma función que lo usa: `offset($fp)`.
    Local,
    /// En el marco de una función que encierra a la que lo usa: seguir
    /// `hops` enlaces de acceso desde `$fp` y luego `offset(...)`.
    NonLocal { hops: usize },
    /// Campo de la clase/struct que encierra al método: se llega por `this`,
    /// no por enlaces de acceso.
    Field,
}

/// A qué declaración apunta un identificador: el ámbito donde vive
/// (`scopes::Scope::id`, 0 = Global) y su posición de declaración dentro de
/// él (`Symbol::decl_index`). El par es único; `name` va solo para depurar.
/// `access` dice cómo alcanzarlo DESDE ESTE USO: la misma variable es
/// `Local` en su función y `NonLocal` en una anidada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolRef {
    pub name: String,
    pub scope_id: usize,
    pub decl_index: usize,
    pub access: Access,
}

impl SymbolRef {
    /// `None` si el símbolo nunca se insertó en un ámbito (no pasa con los que
    /// devuelve la tabla, pero `scope_id` es opcional en `Symbol`).
    pub fn of(symbol: &Symbol, access: Access) -> Option<Self> {
        Some(SymbolRef { name: symbol.name.clone(), scope_id: symbol.scope_id?, decl_index: symbol.decl_index, access })
    }
}

/// Uso/definición → declaración, por nodo hoja. Cubre las tres formas en que
/// un identificador aparece en el árbol: un uso como valor, el destino de una
/// asignación y el nombre en su propia declaración. Un identificador sin
/// entrada acá es uno que no se pudo resolver (ya hay un diagnóstico) o que
/// no nombra una variable (el nombre de un miembro tras `.`, de una clase en
/// `new`...).
#[derive(Debug, Default, Clone)]
pub struct Bindings {
    refs: HashMap<usize, SymbolRef>,
}

impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&mut self, node: &ParseNode, symbol: &Symbol, access: Access) {
        if let Some(r) = SymbolRef::of(symbol, access) {
            self.refs.insert(key(node), r);
        }
    }

    pub fn get(&self, node: &ParseNode) -> Option<&SymbolRef> {
        self.refs.get(&key(node))
    }

    pub fn len(&self) -> usize {
        self.refs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }

    /// Forma `[{id, lexeme, line, col, scope_id, decl_index, access, hops}]`
    /// (`access`: `static`/`local`/`nonlocal`/`field`; `hops` solo en
    /// `nonlocal`), con el mismo
    /// `id` en preorden que `parse_tree::to_dot` y `TypeAnnotations::to_json`.
    pub fn to_json(&self, root: &ParseNode) -> Vec<Value> {
        let mut out = Vec::new();
        let mut next_id = 0usize;
        self.collect_json(root, &mut out, &mut next_id);
        out
    }

    fn collect_json(&self, node: &ParseNode, out: &mut Vec<Value>, next_id: &mut usize) {
        let my_id = *next_id;
        *next_id += 1;
        if let Some(r) = self.get(node) {
            let (access, hops) = match r.access {
                Access::Static => ("static", None),
                Access::Local => ("local", None),
                Access::NonLocal { hops } => ("nonlocal", Some(hops)),
                Access::Field => ("field", None),
            };
            out.push(json!({
                "id": format!("n{my_id}"),
                "lexeme": node.lexeme,
                "line": node.line,
                "col": node.col,
                "scope_id": r.scope_id,
                "decl_index": r.decl_index,
                "access": access,
                "hops": hops,
            }));
        }
        for child in &node.children {
            self.collect_json(child, out, next_id);
        }
    }
}
