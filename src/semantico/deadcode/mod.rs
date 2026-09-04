//! Código muerto: instrucciones inalcanzables tras una sentencia terminal.
//!
//! Una sentencia TERMINAL transfiere el control fuera de la secuencia que la
//! contiene — `return`, `break`, `continue`. Todo lo que venga después de ella
//! dentro de la MISMA secuencia no se ejecuta nunca.
//!
//! Igual que `flow` y `duplicates`, este módulo no conoce ninguna palabra
//! reservada ni nombre de producción: recibe la secuencia ya aplanada y el
//! conjunto de producciones que la gramática declaró como terminales. Ese
//! conjunto NO necesita una directiva propia — sale de las que ya existen
//! (`%return`, `%break`, `%continue`), porque son exactamente las mismas
//! sentencias.

use super::errors::{Diagnostic, ErrorKind, Severity};
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Aplana una secuencia de sentencias recursiva por la izquierda
/// (`stmt_list: stmt_list stmt | stmt`) al orden en que se escribieron.
///
/// Devuelve dos cosas: los nodos de la ESPINA (todos los `stmt_list`
/// encadenados) y las sentencias en orden. La espina hace falta para que el
/// llamador marque esos nodos como ya analizados: si no, cada nivel anidado
/// volvería a analizar un prefijo de la misma lista y reportaría el mismo
/// código muerto varias veces.
///
/// El criterio de "esto es parte de la espina" es puramente estructural —el
/// primer hijo tiene el mismo símbolo que el padre— así que funciona para
/// cualquier gramática con listas recursivas por la izquierda, sin nombres
/// concretos. Una alternativa vacía (`case_body: /* vacio */`) simplemente no
/// aporta sentencias.
pub fn flatten_sequence<'a>(node: &'a ParseNode) -> (Vec<*const ParseNode>, Vec<&'a ParseNode>) {
    let mut spine = Vec::new();
    let mut statements = Vec::new();
    collect(node, &mut spine, &mut statements);
    (spine, statements)
}

fn collect<'a>(
    node: &'a ParseNode,
    spine: &mut Vec<*const ParseNode>,
    statements: &mut Vec<&'a ParseNode>,
) {
    spine.push(node as *const ParseNode);
    match node.children.first() {
        Some(first) if first.symbol == node.symbol => {
            collect(first, spine, statements);
            statements.extend(node.children[1..].iter());
        }
        _ => statements.extend(node.children.iter()),
    }
}

/// ¿Esta sentencia transfiere el control SIEMPRE que se ejecuta?
///
/// Se busca una producción terminal dentro del subárbol, pero la búsqueda se
/// detiene al cruzar otra secuencia de sentencias. Eso es lo que evita el
/// falso positivo obvio: en `if (c) { return 1; } print(2);` el `return` vive
/// dentro de la secuencia del bloque del `if`, que solo se ejecuta a veces, así
/// que el `print` de después SÍ es alcanzable.
///
/// El precio es quedarse corto en el otro sentido: un bloque suelto que
/// siempre retorna (`{ return 1; } print(2);`) no se detecta como terminal, y
/// ese `print` no se reporta. Es la dirección segura del error — saber que un
/// bloque termina siempre exigiría propagar la terminalidad hacia arriba y
/// decidir sobre las ramas de un `if`, que ya es análisis de alcanzabilidad
/// completo.
pub fn is_terminal(node: &ParseNode, terminals: &[String], sequences: &[String]) -> bool {
    if terminals.iter().any(|production| production == &node.symbol) {
        return true;
    }
    if sequences.iter().any(|production| production == &node.symbol) {
        return false;
    }
    node.children
        .iter()
        .any(|child| is_terminal(child, terminals, sequences))
}

/// Lo inalcanzable de una secuencia ya aplanada: el diagnóstico de la PRIMERA
/// instrucción muerta y todas las que le siguen.
///
/// Un solo diagnóstico y no uno por sentencia: son todas la misma causa, y
/// llenar el panel con el mismo error repetido no ayuda a nadie. Las
/// sentencias devueltas son las que el recorrido debe saltarse — el "corte de
/// evaluación del bloque" — para que el código que nunca corre no genere
/// además diagnósticos de tipo o de ámbito derivados.
pub fn unreachable_after_terminal<'a>(
    statements: &[&'a ParseNode],
    terminals: &[String],
    sequences: &[String],
) -> Option<(Diagnostic, Vec<&'a ParseNode>)> {
    let terminal_index = statements
        .iter()
        .position(|statement| is_terminal(statement, terminals, sequences))?;

    let dead = &statements[terminal_index + 1..];
    let first = dead.first()?;
    let terminal = statements[terminal_index];

    Some((
        Diagnostic {
            kind: ErrorKind::ControlFlujo,
            code: "W002".to_string(),
            message: format!(
                "código inalcanzable: la ejecución nunca llega acá porque la línea {} sale del bloque",
                terminal.line
            ),
            line: first.line,
            col: first.col,
            severity: Severity::Warning,
        },
        dead.to_vec(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(symbol: &str, line: usize) -> ParseNode {
        ParseNode {
            symbol: symbol.to_string(),
            lexeme: Some(symbol.to_lowercase()),
            children: vec![],
            line,
            col: 1,
        }
    }

    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        ParseNode::internal(symbol.to_string(), children)
    }

    fn stmt(inner: ParseNode) -> ParseNode {
        internal("stmt", vec![inner])
    }

    /// `stmt_list: stmt_list stmt | stmt` — la forma recursiva por la
    /// izquierda que produce cualquier parser LR sobre una lista.
    fn sequence(statements: Vec<ParseNode>) -> ParseNode {
        let mut iter = statements.into_iter();
        let mut node = internal("stmt_list", vec![iter.next().expect("al menos una")]);
        for statement in iter {
            node = internal("stmt_list", vec![node, statement]);
        }
        node
    }

    fn terminals() -> Vec<String> {
        vec!["return_stmt".to_string(), "break_stmt".to_string(), "continue_stmt".to_string()]
    }

    fn sequences() -> Vec<String> {
        vec!["stmt_list".to_string()]
    }

    #[test]
    fn flatten_restores_the_written_order() {
        let tree = sequence(vec![
            stmt(leaf("A", 1)),
            stmt(leaf("B", 2)),
            stmt(leaf("C", 3)),
        ]);
        let (spine, statements) = flatten_sequence(&tree);
        let lines: Vec<usize> = statements.iter().map(|s| s.line).collect();
        assert_eq!(lines, vec![1, 2, 3], "de izquierda a derecha, no al revés");
        assert_eq!(spine.len(), 3, "los tres nodos stmt_list encadenados");
    }

    #[test]
    fn a_return_makes_everything_after_it_unreachable() {
        let tree = sequence(vec![
            stmt(leaf("A", 1)),
            stmt(internal("return_stmt", vec![leaf("RETURN", 2)])),
            stmt(leaf("B", 3)),
            stmt(leaf("C", 4)),
        ]);
        let (_, statements) = flatten_sequence(&tree);
        let (diagnostic, dead) =
            unreachable_after_terminal(&statements, &terminals(), &sequences()).expect("hay muerto");

        assert_eq!(diagnostic.code, "W002");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.line, 3, "señala la PRIMERA inalcanzable");
        assert!(diagnostic.message.contains("línea 2"), "y nombra a la culpable");
        assert_eq!(dead.len(), 2, "las dos que siguen se saltan enteras");
    }

    #[test]
    fn a_terminal_at_the_end_is_not_dead_code() {
        let tree = sequence(vec![
            stmt(leaf("A", 1)),
            stmt(internal("break_stmt", vec![leaf("BREAK", 2)])),
        ]);
        let (_, statements) = flatten_sequence(&tree);
        assert!(unreachable_after_terminal(&statements, &terminals(), &sequences()).is_none());
    }

    #[test]
    fn a_return_inside_a_nested_sequence_does_not_kill_what_follows() {
        // `if (c) { return 1; } print(2);` — el `return` está dentro de la
        // secuencia del bloque, que solo corre a veces.
        let nested = internal(
            "bloque",
            vec![sequence(vec![stmt(internal("return_stmt", vec![leaf("RETURN", 2)]))])],
        );
        let tree = sequence(vec![
            stmt(internal("if_stmt", vec![leaf("IF", 1), nested])),
            stmt(leaf("PRINT", 3)),
        ]);
        let (_, statements) = flatten_sequence(&tree);
        assert!(
            unreachable_after_terminal(&statements, &terminals(), &sequences()).is_none(),
            "cruzar a otra secuencia corta la búsqueda: no hay falso positivo"
        );
    }

    #[test]
    fn without_declared_terminals_nothing_is_dead() {
        let tree = sequence(vec![
            stmt(internal("return_stmt", vec![leaf("RETURN", 1)])),
            stmt(leaf("B", 2)),
        ]);
        let (_, statements) = flatten_sequence(&tree);
        assert!(
            unreachable_after_terminal(&statements, &[], &sequences()).is_none(),
            "una gramática sin %return/%break/%continue se comporta como antes"
        );
    }
}
