//! El tipo de cada nodo de expresión, guardado aparte del árbol.
//!
//! El libro del dragón llama *árbol de análisis anotado* al árbol con los
//! valores de los atributos pegados en cada nodo. Este módulo es esa
//! anotación, con una diferencia: no vive DENTRO del nodo sino en un mapa
//! lateral, indexado por la identidad del nodo.
//!
//! Se hace así, y no con un campo en `ParseNode`, por tres razones:
//!
//! 1. `ParseNode` es de la capa SINTÁCTICA — la comparten LL(1), LR y los
//!    binarios de prueba. Un `Type` semántico adentro rompería la separación
//!    de capas que sostiene el resto del proyecto.
//! 2. Hay decenas de sitios que construyen un `ParseNode` con literal
//!    exhaustivo (en `sintactico`, en los módulos `#[cfg(test)]` y en los
//!    tests de integración); un campo nuevo los rompe a todos.
//! 3. Es lo que hace ANTLR con su `ParseTreeProperty<T>`, por el mismo
//!    motivo: el árbol que produce el parser no es el lugar de los atributos
//!    que calcula una fase posterior.
//!
//! El registro ocurre DURANTE el recorrido, no después. No es una decisión de
//! estilo: la tabla de símbolos viva descarta cada ámbito al cerrarlo (solo el
//! Global sobrevive — ver `scopes::ScopeSnapshot`), así que una segunda pasada
//! sobre el árbol ya no podría resolver ni una variable local. El tipo hay que
//! guardarlo en el momento en que el ámbito correcto todavía está en la pila.
//!
//! # Invariante de las claves
//!
//! La clave es la DIRECCIÓN del nodo (`&ParseNode as *const _ as usize`), y
//! por eso solo es válida mientras ese árbol siga vivo, en su lugar y sin
//! clonar. Un `ParseNode` clonado es otro nodo y no comparte anotaciones.
//! `api::pipeline` cumple la invariante: el árbol se construye una vez, se le
//! pasa por referencia a `analyze` y sigue vivo para generar el DOT anotado
//! justo después.
//!
//! La dirección se guarda como `usize` y no como `*const ParseNode` a
//! propósito: un puntero crudo no es `Send`/`Sync`, y `AnalysisResult` viaja
//! por los handlers async de `bin/api.rs`. Nunca se desreferencia — la
//! dirección se usa solo como clave de hash, así que no hay forma de leer
//! memoria liberada por acá.

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::sintactico::runtime::parse_tree::{NodeTypes, ParseNode};

use super::Type;

/// Identidad de un nodo dentro de un árbol vivo. Ver la invariante en el
/// doc-comment del módulo.
fn key(node: &ParseNode) -> usize {
    node as *const ParseNode as usize
}

/// El tipo inferido de cada nodo de expresión que el análisis llegó a tipar.
///
/// Mismo espíritu que `closures::ClosureCollector` y `scopes::ScopeCollector`:
/// observa y guarda, sin participar de ninguna decisión. Que un nodo no
/// aparezca acá no es un error — significa que nadie le pidió el tipo (una
/// palabra clave, un separador) o que no se pudo resolver.
#[derive(Debug, Default, Clone)]
pub struct TypeAnnotations {
    types: HashMap<usize, Type>,
}

impl TypeAnnotations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Anota `node` con `ty`. Si ya estaba anotado se sobreescribe: el mismo
    /// subárbol puede tiparse más de una vez (`resolve_expr_type` es un
    /// resolvedor bajo demanda y varios llamadores pasan por el mismo nodo),
    /// pero siempre con el mismo resultado, porque es una función pura de
    /// (nodo, tabla, spec).
    pub fn record(&mut self, node: &ParseNode, ty: &Type) {
        self.types.insert(key(node), ty.clone());
    }

    pub fn get(&self, node: &ParseNode) -> Option<&Type> {
        self.types.get(&key(node))
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Forma `[{id, symbol, lexeme, line, col, ty}]` — la que consume la
    /// pestaña de tipos del IDE.
    ///
    /// El `id` es `n{índice en preorden}`, EL MISMO identificador que
    /// `parse_tree::to_dot` le da a ese nodo en el grafo. Esa coincidencia es
    /// deliberada: deja correlacionar una fila de la tabla con su nodo del
    /// árbol dibujado. Si se cambia el esquema de ids de uno hay que cambiar
    /// el del otro.
    ///
    /// Recorre `root` en vez de volcar el mapa porque un `HashMap` no tiene
    /// orden y las direcciones no le dicen nada a nadie: recorriendo el árbol
    /// las filas salen en orden de lectura del programa.
    pub fn to_json(&self, root: &ParseNode) -> Vec<Value> {
        let mut out = Vec::new();
        let mut next_id = 0usize;
        self.collect_json(root, &mut out, &mut next_id);
        out
    }

    fn collect_json(&self, node: &ParseNode, out: &mut Vec<Value>, next_id: &mut usize) {
        let my_id = *next_id;
        *next_id += 1;

        if let Some(ty) = self.get(node) {
            out.push(json!({
                "id": format!("n{my_id}"),
                "symbol": node.symbol,
                "lexeme": node.lexeme,
                "line": node.line,
                "col": node.col,
                "ty": ty.to_string(),
            }));
        }

        for child in &node.children {
            self.collect_json(child, out, next_id);
        }
    }
}

/// Deja que `parse_tree::to_dot_annotated` dibuje el arbol anotado sin que la
/// capa sintactica tenga que conocer el enum `Type`: solo recibe el texto ya
/// formateado por el `Display` de `Type`.
impl NodeTypes for TypeAnnotations {
    fn label_for(&self, node: &ParseNode) -> Option<String> {
        self.get(node).map(|ty| ty.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sintactico::runtime::parse_tree::ParseToken;

    fn leaf(kind: &str, lexeme: &str) -> ParseNode {
        ParseNode::leaf(&ParseToken {
            kind: kind.to_string(),
            lexeme: lexeme.to_string(),
            line: 1,
            col: 1,
        })
    }

    #[test]
    fn un_arbol_sin_anotar_no_produce_filas() {
        let tree = ParseNode::internal("expr".to_string(), vec![leaf("NUM", "1")]);
        let anotaciones = TypeAnnotations::new();
        assert!(anotaciones.is_empty());
        assert!(anotaciones.to_json(&tree).is_empty());
    }

    #[test]
    fn anota_y_recupera_por_identidad_de_nodo() {
        let tree = ParseNode::internal("expr".to_string(), vec![leaf("NUM", "1")]);
        let mut anotaciones = TypeAnnotations::new();
        anotaciones.record(&tree, &Type::Int);

        assert_eq!(anotaciones.get(&tree), Some(&Type::Int));
        // El hijo es OTRO nodo: no hereda la anotación del padre.
        assert_eq!(anotaciones.get(&tree.children[0]), None);
    }

    #[test]
    fn el_id_del_json_sigue_el_preorden_del_dot() {
        // expr(NUM, expr2(NUM)) -> n0 = expr, n1 = NUM, n2 = expr2, n3 = NUM
        let tree = ParseNode::internal(
            "expr".to_string(),
            vec![
                leaf("NUM", "1"),
                ParseNode::internal("expr2".to_string(), vec![leaf("NUM", "2")]),
            ],
        );
        let mut anotaciones = TypeAnnotations::new();
        anotaciones.record(&tree.children[1], &Type::Float);
        anotaciones.record(&tree.children[0], &Type::Int);

        let filas = anotaciones.to_json(&tree);
        assert_eq!(filas.len(), 2);
        // En orden de recorrido, no en orden de inserción.
        assert_eq!(filas[0]["id"], "n1");
        assert_eq!(filas[0]["ty"], "integer");
        assert_eq!(filas[1]["id"], "n2");
        assert_eq!(filas[1]["ty"], "float");
    }

    #[test]
    fn sobreescribir_una_anotacion_no_duplica_la_fila() {
        let tree = ParseNode::internal("expr".to_string(), vec![leaf("NUM", "1")]);
        let mut anotaciones = TypeAnnotations::new();
        anotaciones.record(&tree, &Type::Int);
        anotaciones.record(&tree, &Type::Int);

        assert_eq!(anotaciones.len(), 1);
        assert_eq!(anotaciones.to_json(&tree).len(), 1);
    }
}
