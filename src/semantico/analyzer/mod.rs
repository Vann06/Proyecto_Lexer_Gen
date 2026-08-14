// Walker genérico (Fase 15): recorre un `ParseNode` real y llama a
// `symbols::SymbolTable` según lo que diga un `spec::SemanticSpec` — este
// archivo NUNCA menciona el nombre de una producción concreta (ni
// "var_decl" ni "func_decl" ni nada por el estilo). Toda la especificidad
// de una gramática dada vive en el `SemanticSpec` que se le pasa a
// `analyze`, no acá.
use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::{SemanticError, SymbolTable};
use crate::sintactico::runtime::parse_tree::ParseNode;

pub struct AnalysisResult {
    pub table: SymbolTable,
    pub errors: Vec<SemanticError>,
}

/// Punto de entrada: recorre `tree` según `spec` y devuelve la tabla de
/// símbolos resultante junto con todos los errores semánticos encontrados
/// (no se detiene en el primero — sigue recorriendo para reportar todos,
/// mismo espíritu que el modo pánico del parser LR).
pub fn analyze(tree: &ParseNode, spec: &SemanticSpec) -> AnalysisResult {
    let mut table = SymbolTable::new();
    let mut errors = Vec::new();
    walk(tree, spec, &mut table, &mut errors);
    AnalysisResult { table, errors }
}

/// Busca, entre los hijos DIRECTOS de `node`, el que representa el nombre
/// declarado: en el índice explícito si se dio uno, o si no el primer hijo
/// cuyo `symbol` sea `identifier_token`. Devuelve también su índice, para
/// que el llamador pueda excluirlo de la recursión genérica (ya fue
/// consumido como declaración, no debe procesarse de nuevo como uso).
fn find_identifier_child<'a>(
    node: &'a ParseNode,
    identifier_token: &str,
    explicit_index: Option<usize>,
) -> Option<(usize, &'a ParseNode)> {
    match explicit_index {
        Some(i) => node.children.get(i).filter(|c| c.symbol == identifier_token).map(|c| (i, c)),
        None => node.children.iter().enumerate().find(|(_, c)| c.symbol == identifier_token),
    }
}

fn walk(node: &ParseNode, spec: &SemanticSpec, table: &mut SymbolTable, errors: &mut Vec<SemanticError>) {
    // Una hoja de identificador solo llega hasta acá si su padre NO la
    // consumió como nombre de una declaración (esos hijos se saltan antes
    // de recursar, ver el bucle de abajo) — así que es un uso real.
    if node.children.is_empty() && node.symbol == spec.identifier_token {
        let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
        if let Err(e) = table.lookup_or_err(name, node.line, node.col) {
            errors.push(e);
        }
        return;
    }

    let decl_rule = spec.declarations.iter().find(|r| r.production == node.symbol);
    let scope_rule = spec.scopes.iter().find(|r| r.production == node.symbol);

    let mut consumed_index: Option<usize> = None;

    if let Some(rule) = decl_rule {
        if let Some((idx, name_node)) = find_identifier_child(node, &spec.identifier_token, rule.name_child) {
            consumed_index = Some(idx);
            let name = name_node.lexeme.as_deref().unwrap_or(&name_node.symbol).to_string();

            // `implicit`: si ya es visible en algún scope, esto es una
            // reasignación, no una declaración nueva — no tocar la tabla.
            let should_declare = !rule.implicit || table.lookup(&name).is_none();
            if should_declare {
                if let Err(e) = table.declare(&name, rule.kind.clone(), name_node.line, name_node.col) {
                    errors.push(e);
                }
            }
        }
    }

    let entered_scope = if let Some(rule) = scope_rule {
        let label = if rule.with_label {
            // Mismo auto-hallazgo que la declaración — si esta producción
            // también declaraba (p.ej. func_decl), reusa el índice para no
            // buscarlo dos veces; si no, busca desde cero.
            find_identifier_child(node, &spec.identifier_token, decl_rule.and_then(|d| d.name_child))
                .and_then(|(_, n)| n.lexeme.clone())
        } else {
            None
        };
        match label {
            Some(l) => table.enter_scope_named(rule.kind, l),
            None => table.enter_scope(rule.kind),
        }
        true
    } else {
        false
    };

    for (i, child) in node.children.iter().enumerate() {
        if Some(i) == consumed_index {
            continue;
        }
        walk(child, spec, table, errors);
    }

    if entered_scope {
        // El scope que se cierra es el que este mismo `walk` acaba de
        // abrir arriba — nunca puede ser el Global, así que esto no falla.
        table
            .exit_scope()
            .expect("el scope recién abierto por este walk debe poder cerrarse");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantico::scopes::ScopeKind;
    use crate::semantico::spec::{DeclarationRule, ScopeRule};
    use crate::semantico::symbols::SymbolKind;

    fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(lexeme.to_string()), children: vec![], line, col }
    }

    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: None, children, line: 0, col: 0 }
    }

    #[test]
    fn declare_via_matched_production_then_lookup_succeeds() {
        // var_decl: tipo ID  ->  [tipo-leaf, ID-leaf]
        let tree = internal("var_decl", vec![
            leaf("INT_T", "int", 1, 1),
            leaf("ID", "x", 1, 5),
        ]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: false,
            }],
            scopes: vec![],
        };

        let result = analyze(&tree, &spec);
        assert!(result.errors.is_empty());
        let sym = result.table.lookup("x").expect("x se declaró");
        assert_eq!(sym.kind, SymbolKind::Variable);
        assert_eq!((sym.line, sym.col), (1, 5));
    }

    #[test]
    fn declaration_and_scope_together_close_cleanly() {
        // func_decl: FUN ID bloque  ->  declara "foo" en el scope EXTERIOR
        // y abre un scope Function para el bloque.
        let bloque = internal("bloque", vec![
            internal("var_decl", vec![leaf("INT_T", "int", 2, 3), leaf("ID", "local", 2, 7)]),
        ]);
        let func_decl = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "foo", 1, 5),
            bloque,
        ]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![
                DeclarationRule {
                    production: "func_decl".to_string(),
                    kind: SymbolKind::Function,
                    name_child: None,
                    implicit: false,
                },
                DeclarationRule {
                    production: "var_decl".to_string(),
                    kind: SymbolKind::Variable,
                    name_child: None,
                    implicit: false,
                },
            ],
            scopes: vec![
                ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
                ScopeRule { production: "bloque".to_string(), kind: ScopeKind::Block, with_label: false },
            ],
        };

        let result = analyze(&func_decl, &spec);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        // "foo" quedó declarada afuera (el walk ya cerró todos los scopes que abrió).
        assert_eq!(result.table.depth(), 1);
        assert_eq!(result.table.lookup("foo").unwrap().kind, SymbolKind::Function);
        // "local" era del bloque interno — ya no es visible desde afuera.
        assert_eq!(result.table.lookup("local"), None);
    }

    #[test]
    fn implicit_declaration_does_not_redeclare_when_already_visible() {
        // Simula foo(x) { x = x + 1; } — "x" ya existe como parámetro
        // (implicit=false), la reasignación (implicit=true) no debe pisarla.
        let param = internal("param", vec![leaf("ID", "x", 1, 5)]);
        let reassign = internal("stmt", vec![leaf("ID", "x", 2, 1)]);
        let seq = internal("seq", vec![param, reassign]); // orden importa: param primero

        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![
                DeclarationRule {
                    production: "param".to_string(),
                    kind: SymbolKind::Parameter,
                    name_child: None,
                    implicit: false,
                },
                DeclarationRule {
                    production: "stmt".to_string(),
                    kind: SymbolKind::Variable,
                    name_child: None,
                    implicit: true,
                },
            ],
            scopes: vec![],
        };

        let result = analyze(&seq, &spec);
        assert!(result.errors.is_empty(), "reasignar un parámetro no debe dar error: {:?}", result.errors);
        // Sigue siendo Parameter — la reasignación implícita no la reemplazó.
        assert_eq!(result.table.lookup("x").unwrap().kind, SymbolKind::Parameter);
    }

    #[test]
    fn implicit_declaration_creates_fresh_when_not_visible() {
        let stmt = internal("stmt", vec![leaf("ID", "y", 1, 1)]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
            }],
            scopes: vec![],
        };

        let result = analyze(&stmt, &spec);
        assert!(result.errors.is_empty());
        assert_eq!(result.table.lookup("y").unwrap().kind, SymbolKind::Variable);
    }

    #[test]
    fn undeclared_identifier_leaf_produces_error() {
        let expr = internal("expr", vec![leaf("ID", "z", 3, 9)]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
        };

        let result = analyze(&expr, &spec);
        assert_eq!(result.errors.len(), 1);
        match &result.errors[0] {
            SemanticError::Undeclared { name, line, col } => {
                assert_eq!(name, "z");
                assert_eq!((*line, *col), (3, 9));
            }
            other => panic!("se esperaba Undeclared, salió {other:?}"),
        }
    }

    #[test]
    fn name_child_none_finds_id_regardless_of_position_under_same_head() {
        // param_list: ID              -> [ID-leaf]                    (índice 0)
        // param_list: param_list COMMA ID -> [param_list, COMMA, ID]  (índice 2)
        let single = internal("param_list", vec![leaf("ID", "a", 1, 1)]);
        let recursive = internal("param_list", vec![
            internal("param_list", vec![leaf("ID", "a", 1, 1)]),
            leaf("COMMA", ",", 1, 2),
            leaf("ID", "b", 1, 4),
        ]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![DeclarationRule {
                production: "param_list".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
            }],
            scopes: vec![],
        };

        let r1 = analyze(&single, &spec);
        assert!(r1.errors.is_empty());
        assert!(r1.table.lookup("a").is_some());

        let r2 = analyze(&recursive, &spec);
        assert!(r2.errors.is_empty(), "{:?}", r2.errors);
        assert!(r2.table.lookup("a").is_some(), "el param_list anidado debe declarar 'a' solo");
        assert!(r2.table.lookup("b").is_some());
    }

    #[test]
    fn production_with_unmatched_shape_does_not_fire_falsely() {
        // stmt: RETURN expr — el mismo head "stmt" que en otras
        // alternativas SÍ declara (ver implicit_*), pero acá el primer
        // hijo directo es RETURN, no ID: la regla no debe disparar.
        let stmt = internal("stmt", vec![
            leaf("RETURN", "return", 1, 1),
            internal("expr", vec![leaf("ID", "x", 1, 8)]),
        ]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
            }],
            scopes: vec![],
        };

        // "x" nunca se declaró en ningún lado — como stmt no disparó la
        // declaración (no hay ID directo), el ID dentro de expr se procesa
        // como uso normal y debe fallar por no-declarada.
        let result = analyze(&stmt, &spec);
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(&result.errors[0], SemanticError::Undeclared { name, .. } if name == "x"));
    }
}
