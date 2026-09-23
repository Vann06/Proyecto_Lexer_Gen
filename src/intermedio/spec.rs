//! Qué necesita saber la fase de código intermedio sobre una gramática
//! concreta, y que el análisis semántico no le da: la FORMA de cada
//! construcción de control de flujo (directivas `%flow`).
//!
//! Igual que `semantico::spec::SemanticSpec`, es lo único específico de una
//! gramática; el generador consulta esto en vez de nombrar producciones.

use std::collections::HashMap;

use crate::sintactico::gramatica::flow::{FlowDirective, FlowKind, FlowRole};
use crate::sintactico::gramatica::grammar::Grammar;
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Las directivas de la fase intermedia de una gramática, indexadas por
/// producción. `Grammar` ya las validó al leer el `.yalp`, así que armar
/// esto no puede fallar.
#[derive(Debug, Clone, Default)]
pub struct IntermediateSpec {
    flows: HashMap<String, FlowDirective>,
}

impl IntermediateSpec {
    pub fn from_grammar(grammar: &Grammar) -> Self {
        IntermediateSpec {
            flows: grammar.flow_directives.iter().map(|d| (d.production.clone(), d.clone())).collect(),
        }
    }

    /// La forma de `node` si su producción es una construcción de control de
    /// flujo declarada con `%flow`.
    pub fn flow_of<'s>(&'s self, node: &ParseNode) -> Option<FlowShape<'s>> {
        self.flows.get(&node.symbol).map(|directive| FlowShape { directive })
    }

    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }
}

/// Una construcción de control de flujo concreta: su tipo y dónde encontrar
/// cada una de sus partes dentro de un nodo del árbol.
#[derive(Debug, Clone, Copy)]
pub struct FlowShape<'s> {
    directive: &'s FlowDirective,
}

impl<'s> FlowShape<'s> {
    pub fn kind(&self) -> FlowKind {
        self.directive.kind
    }

    /// El hijo de `node` que cumple `role`, o `None` si esta alternativa no
    /// lo trae — un `if` sin `else` (el índice no existe) o un `for` con
    /// `for_init` vacío (el hijo existe pero solo deriva ε). En los dos casos
    /// la respuesta para quien genera código es la misma: esa parte no emite
    /// nada.
    pub fn child<'n>(&self, node: &'n ParseNode, role: FlowRole) -> Option<&'n ParseNode> {
        let child = node.children.get(self.directive.index_of(role)?)?;
        (!derives_only_epsilon(child)).then_some(child)
    }
}

/// `true` para un no-terminal que no derivó ningún token: sin hijos, o con
/// solo hojas ε (así representa el parser LR una reducción vacía).
fn derives_only_epsilon(node: &ParseNode) -> bool {
    node.lexeme.is_none() && node.children.iter().all(|c| c.lexeme.is_none() && derives_only_epsilon(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(symbol: &str) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(symbol.to_lowercase()), children: vec![], line: 1, col: 1 }
    }

    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        ParseNode::internal(symbol.to_string(), children)
    }

    fn spec() -> IntermediateSpec {
        let yalp = "%token IF LPAREN RPAREN ELSE X FOR SEMI\n\
                    %flow if if_stmt cond=2 then=4 else=6\n\
                    %flow for for_stmt init=2 cond=4 body=6\n\
                    %%\n\
                    if_stmt: IF LPAREN X RPAREN X | IF LPAREN X RPAREN X ELSE X ;\n\
                    for_stmt: FOR LPAREN init SEMI X SEMI X ;\n\
                    init: X | ;\n";
        IntermediateSpec::from_grammar(&Grammar::parse_for_lr_from_str(yalp).expect("gramática válida"))
    }

    #[test]
    fn un_if_sin_else_no_tiene_rama_falsa() {
        let spec = spec();
        let sin_else = internal("if_stmt", vec![leaf("IF"), leaf("LPAREN"), leaf("X"), leaf("RPAREN"), leaf("X")]);
        let shape = spec.flow_of(&sin_else).expect("if_stmt tiene %flow");
        assert_eq!(shape.kind(), FlowKind::If);
        assert_eq!(shape.child(&sin_else, FlowRole::Cond).map(|n| n.symbol.as_str()), Some("X"));
        assert!(shape.child(&sin_else, FlowRole::Else).is_none());
    }

    #[test]
    fn una_inicializacion_vacia_no_existe() {
        let spec = spec();
        let vacio = internal("init", vec![ParseNode::epsilon_leaf()]);
        let for_node = internal(
            "for_stmt",
            vec![leaf("FOR"), leaf("LPAREN"), vacio, leaf("SEMI"), leaf("X"), leaf("SEMI"), leaf("X")],
        );
        let shape = spec.flow_of(&for_node).unwrap();
        assert!(shape.child(&for_node, FlowRole::Init).is_none());
        assert!(shape.child(&for_node, FlowRole::Body).is_some());
    }

    #[test]
    fn una_produccion_sin_flow_no_tiene_forma() {
        assert!(spec().flow_of(&leaf("X")).is_none());
    }

    #[test]
    fn un_rol_obligatorio_fuera_de_una_alternativa_es_un_error_de_la_gramatica() {
        // `then=5` no existe en la primera alternativa (5 hijos: 0..=4).
        let yalp = "%token IF LPAREN RPAREN ELSE X\n\
                    %flow if if_stmt cond=2 then=5\n\
                    %%\n\
                    if_stmt: IF LPAREN X RPAREN X | IF LPAREN X RPAREN X ELSE X ;\n";
        let err = Grammar::parse_for_lr_from_str(yalp).unwrap_err();
        assert!(err.contains("'then' apunta al hijo 5"), "{err}");
    }

    #[test]
    fn un_flow_sobre_una_produccion_inexistente_es_un_error() {
        let yalp = "%token X\n%flow while no_existe cond=0 body=1\n%%\ns: X ;\n";
        assert!(Grammar::parse_for_lr_from_str(yalp).unwrap_err().contains("no existe"));
    }
}
