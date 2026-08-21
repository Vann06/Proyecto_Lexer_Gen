// Visitor genérico (Fase 15): abstrae el recorrido de un `ParseNode` para que
// cualquier fase futura (semántica hoy; tipos más finos, listas, control de
// flujo, mañana) implemente SOLO la política de qué hacer en cada nodo — la
// mecánica de bajar a los hijos y subir de vuelta vive acá, una única vez, en
// vez de que cada fase reimplemente su propia recursión sobre `ParseNode`.
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Qué hacer con los hijos de un nodo después de `enter`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Recorrer todos los hijos, en orden.
    Continue,
    /// No recorrer ningún hijo (p. ej. una hoja, o un nodo ya totalmente
    /// resuelto por `enter`).
    SkipChildren,
    /// Recorrer todos los hijos EXCEPTO el de este índice — el caso del
    /// analizador semántico: el identificador que `enter` ya consumió como
    /// nombre de una declaración no debe procesarse de nuevo como un uso.
    SkipChild(usize),
}

/// Política de un recorrido concreto. `enter` decide qué hacer al entrar a un
/// nodo (y con qué hijos seguir); `exit` corre después de haber recorrido esos
/// hijos — el lugar natural para cerrar lo que `enter` abrió (p. ej. un scope).
pub trait Visitor {
    fn enter(&mut self, node: &ParseNode) -> Flow;

    fn exit(&mut self, _node: &ParseNode) {}
}

/// Recorre `root` con la política de `visitor`. Único punto de la mecánica de
/// bajada/subida — ninguna fase debe reimplementar su propia recursión.
pub fn walk<V: Visitor + ?Sized>(root: &ParseNode, visitor: &mut V) {
    match visitor.enter(root) {
        Flow::Continue => {
            for child in &root.children {
                walk(child, visitor);
            }
        }
        Flow::SkipChild(idx) => {
            for (i, child) in root.children.iter().enumerate() {
                if i != idx {
                    walk(child, visitor);
                }
            }
        }
        Flow::SkipChildren => {}
    }
    visitor.exit(root);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recorder {
        events: Vec<String>,
    }

    impl Visitor for Recorder {
        fn enter(&mut self, node: &ParseNode) -> Flow {
            self.events.push(format!("enter {}", node.symbol));
            Flow::Continue
        }
        fn exit(&mut self, node: &ParseNode) {
            self.events.push(format!("exit {}", node.symbol));
        }
    }

    fn leaf(symbol: &str) -> ParseNode {
        ParseNode { symbol: symbol.into(), lexeme: None, children: vec![], line: 0, col: 0 }
    }
    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        ParseNode::internal(symbol.into(), children)
    }

    #[test]
    fn visits_in_preorder_enter_then_postorder_exit() {
        let tree = internal("root", vec![leaf("a"), leaf("b")]);
        let mut rec = Recorder { events: vec![] };
        walk(&tree, &mut rec);
        assert_eq!(
            rec.events,
            vec!["enter root", "enter a", "exit a", "enter b", "exit b", "exit root"]
        );
    }

    struct ChildSkipper {
        skip_idx: usize,
        visited: Vec<String>,
    }
    impl Visitor for ChildSkipper {
        fn enter(&mut self, node: &ParseNode) -> Flow {
            if node.symbol == "root" {
                return Flow::SkipChild(self.skip_idx);
            }
            self.visited.push(node.symbol.clone());
            Flow::Continue
        }
    }

    #[test]
    fn skip_child_visits_every_child_except_the_given_index() {
        let tree = internal("root", vec![leaf("a"), leaf("b"), leaf("c")]);
        let mut v = ChildSkipper { skip_idx: 1, visited: vec![] };
        walk(&tree, &mut v);
        assert_eq!(v.visited, vec!["a", "c"]);
    }

    struct LeafStopper {
        exits: usize,
    }
    impl Visitor for LeafStopper {
        fn enter(&mut self, node: &ParseNode) -> Flow {
            if node.children.is_empty() {
                Flow::SkipChildren
            } else {
                Flow::Continue
            }
        }
        fn exit(&mut self, _node: &ParseNode) {
            self.exits += 1;
        }
    }

    #[test]
    fn skip_children_on_leaf_still_calls_exit_exactly_once() {
        let tree = leaf("x");
        let mut v = LeafStopper { exits: 0 };
        walk(&tree, &mut v);
        assert_eq!(v.exits, 1);
    }
}
