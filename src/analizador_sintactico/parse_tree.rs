// Árbol de derivación compartido entre LL(1) y LR (LALR / SLR / LR(0) / LR(1)).
//
// El mismo tipo `ParseNode` lo construyen ambos algoritmos:
//   - LL(1) lo construye top-down conforme expande producciones.
//   - LR(k) lo construye bottom-up: shift = hoja; reduce = nodo interno con los
//     últimos |body| nodos como hijos.
//
// `ParseToken` es la entrada universal para los parsers (kind = lo que decide la
// tabla; lexeme = el texto real que aparece en la hoja del árbol).

use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseNode {
    pub symbol: String,            // nombre del NT o del token
    pub lexeme: Option<String>,    // Some sólo en hojas (terminales)
    pub children: Vec<ParseNode>,  // vacío en hojas
}

#[derive(Debug, Clone)]
pub struct ParseToken {
    pub kind: String,
    pub lexeme: String,
}

impl ParseToken {
    /// Helper para tests / CLI: crea tokens donde el lexema coincide con el kind.
    pub fn from_kinds(kinds: Vec<String>) -> Vec<ParseToken> {
        kinds.into_iter()
            .map(|k| ParseToken { lexeme: k.clone(), kind: k })
            .collect()
    }
}

impl ParseNode {
    pub fn leaf(token: &ParseToken) -> Self {
        ParseNode {
            symbol: token.kind.clone(),
            lexeme: Some(token.lexeme.clone()),
            children: Vec::new(),
        }
    }

    pub fn epsilon_leaf() -> Self {
        ParseNode {
            symbol: "ε".to_string(),
            lexeme: None,
            children: Vec::new(),
        }
    }

    pub fn internal(symbol: String, children: Vec<ParseNode>) -> Self {
        ParseNode { symbol, lexeme: None, children }
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
    let mut out = String::from("digraph ParseTree {\n");
    out.push_str("    node [shape=box, fontname=\"monospace\"];\n");
    let mut next_id = 0usize;
    to_dot_rec(root, &mut out, &mut next_id);
    out.push_str("}\n");
    out
}

fn to_dot_rec(node: &ParseNode, out: &mut String, next_id: &mut usize) -> usize {
    let my_id = *next_id;
    *next_id += 1;

    let label = match &node.lexeme {
        Some(lx) if lx != &node.symbol => format!("{}\\n\\\"{}\\\"", node.symbol, escape_dot(lx)),
        _ => node.symbol.clone(),
    };
    let style = if node.children.is_empty() {
        ", style=filled, fillcolor=\"#e0f0ff\""
    } else {
        ""
    };
    let _ = writeln!(out, "    n{} [label=\"{}\"{}];", my_id, label, style);

    for child in &node.children {
        let cid = to_dot_rec(child, out, next_id);
        let _ = writeln!(out, "    n{} -> n{};", my_id, cid);
    }
    my_id
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
