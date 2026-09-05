// Árbol de derivación compartido entre LL(1) y LR (LALR / SLR / LR(0) / LR(1)).

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNode {
    pub symbol: String,            // nombre del NT o del token
    pub lexeme: Option<String>,    // Some sólo en hojas (terminales)
    pub children: Vec<ParseNode>,  // vacío en hojas
    // Posición en el fuente: en hojas reales es la línea/columna del token que
    // originó el nodo; en nodos internos (`internal()`) se hereda del primer
    // hijo con posición real, para que un error anclado a una producción (una
    // expresión, una llamada) también pueda ubicarse. En `epsilon_leaf()` queda
    // en 0 — no hay ningún token del que heredar.
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone)]
pub struct ParseToken {
    pub kind: String,
    pub lexeme: String,
    pub line: usize,
    pub col: usize,
}

impl ParseToken {
    /// Helper para tests / CLI: crea tokens donde el lexema coincide con el kind.
    /// Sin posición real disponible en ese contexto — queda en 0/0.
    pub fn from_kinds(kinds: Vec<String>) -> Vec<ParseToken> {
        kinds.into_iter()
            .map(|k| ParseToken { lexeme: k.clone(), kind: k, line: 0, col: 0 })
            .collect()
    }
}

impl ParseNode {
    pub fn leaf(token: &ParseToken) -> Self {
        ParseNode {
            symbol: token.kind.clone(),
            lexeme: Some(token.lexeme.clone()),
            children: Vec::new(),
            line: token.line,
            col: token.col,
        }
    }

    pub fn epsilon_leaf() -> Self {
        ParseNode {
            symbol: "ε".to_string(),
            lexeme: None,
            children: Vec::new(),
            line: 0,
            col: 0,
        }
    }

    /// Hereda `line`/`col` del primer hijo con posición real (no 0/0) — así un
    /// error anclado a este nodo (no a una hoja) sigue ubicando una línea
    /// concreta del fuente en vez de reportar 0:0.
    pub fn internal(symbol: String, children: Vec<ParseNode>) -> Self {
        let (line, col) = children
            .iter()
            .find(|c| c.line != 0 || c.col != 0)
            .map(|c| (c.line, c.col))
            .unwrap_or((0, 0));
        ParseNode { symbol, lexeme: None, children, line, col }
    }
}

/// Imprime el árbol en formato ASCII estilo Unix `tree`.
pub fn print_ascii(root: &ParseNode) {
    println!("{}", root.symbol);
    let n = root.children.len();
    for (i, child) in root.children.iter().enumerate() {
        print_ascii_rec(child, "", i + 1 == n);
    }
}

fn print_ascii_rec(node: &ParseNode, prefix: &str, is_last: bool) {
    let connector = if is_last { "└── " } else { "├── " };
    let label = match &node.lexeme {
        Some(lx) if lx != &node.symbol => format!("{} ({})", node.symbol, lx),
        _ => node.symbol.clone(),
    };
    println!("{}{}{}", prefix, connector, label);

    let new_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        print_ascii_rec(child, &new_prefix, i + 1 == n);
    }
}

/// Exporta el árbol a formato DOT (Graphviz).
pub fn to_dot(root: &ParseNode) -> String {
    to_dot_with(root, None)
}

/// Cómo le pregunta el dibujante del árbol por el tipo de un nodo.
///
/// Existe para que la capa SINTÁCTICA no dependa de `semantico`: acá no se
/// sabe qué es un `Type` ni cómo se calculó, solo se pide un texto por nodo.
/// `semantico::types::TypeAnnotations` es quien la implementa.
pub trait NodeTypes {
    /// El tipo de `node` ya formateado, o `None` si ese nodo no se tipó.
    fn label_for(&self, node: &ParseNode) -> Option<String>;
}

/// El MISMO árbol, con el tipo inferido de cada nodo debajo de su etiqueta:
/// el *árbol de análisis anotado* del libro del dragón.
///
/// `to_dot` queda intacto a propósito — lo usan los binarios de prueba, que
/// no corren la fase semántica y no tienen anotaciones que mostrar.
pub fn to_dot_annotated(root: &ParseNode, types: &dyn NodeTypes) -> String {
    to_dot_with(root, Some(types))
}

fn to_dot_with(root: &ParseNode, types: Option<&dyn NodeTypes>) -> String {
    let mut out = String::from("digraph ParseTree {\n");
    out.push_str("    node [shape=box, fontname=\"monospace\"];\n");
    let mut next_id = 0usize;
    to_dot_rec(root, &mut out, &mut next_id, types);
    out.push_str("}\n");
    out
}

fn to_dot_rec(
    node: &ParseNode,
    out: &mut String,
    next_id: &mut usize,
    types: Option<&dyn NodeTypes>,
) -> usize {
    let my_id = *next_id;
    *next_id += 1;

    let mut label = match &node.lexeme {
        Some(lx) if lx != &node.symbol => format!("{}\\n\\\"{}\\\"", node.symbol, escape_dot(lx)),
        _ => node.symbol.clone(),
    };
    // Tercera línea de la etiqueta, solo si este nodo se tipó. El `: ` inicial
    // la distingue a simple vista del lexema, que va entre comillas.
    if let Some(ty) = types.and_then(|t| t.label_for(node)) {
        label.push_str("\\n: ");
        label.push_str(&escape_dot(&ty));
    }
    let style = if node.children.is_empty() {
        ", style=filled, fillcolor=\"#e0f0ff\""
    } else {
        ""
    };
    let _ = writeln!(out, "    n{} [label=\"{}\"{}];", my_id, label, style);

    for child in &node.children {
        let cid = to_dot_rec(child, out, next_id, types);
        let _ = writeln!(out, "    n{} -> n{};", my_id, cid);
    }
    my_id
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_node_inherits_position_from_first_positioned_child() {
        let leaf = ParseNode::leaf(&ParseToken { kind: "ID".into(), lexeme: "x".into(), line: 3, col: 7 });
        let node = ParseNode::internal("expr".into(), vec![leaf]);
        assert_eq!((node.line, node.col), (3, 7));
    }

    #[test]
    fn internal_node_skips_epsilon_child_to_find_position() {
        let eps = ParseNode::epsilon_leaf();
        let leaf = ParseNode::leaf(&ParseToken { kind: "ID".into(), lexeme: "y".into(), line: 5, col: 2 });
        let node = ParseNode::internal("stmt".into(), vec![eps, leaf]);
        assert_eq!((node.line, node.col), (5, 2));
    }

    #[test]
    fn internal_node_with_all_positionless_children_stays_zero() {
        let node = ParseNode::internal("empty".into(), vec![ParseNode::epsilon_leaf()]);
        assert_eq!((node.line, node.col), (0, 0));
    }
}
