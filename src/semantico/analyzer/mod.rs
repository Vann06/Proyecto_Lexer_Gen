// Walker genérico (Fase 15), ahora como `impl Visitor for Analyzer` sobre el
// driver de `super::visitor` — este archivo NUNCA menciona el nombre de una
// producción concreta (ni "var_decl" ni "func_decl" ni nada por el estilo).
// Toda la especificidad de una gramática dada vive en el `SemanticSpec` que
// se le pasa a `analyze`, no acá.
use crate::semantico::errors::ErrorCollector;
use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::SymbolTable;
use crate::semantico::visitor::{self, Flow, Visitor};
use crate::sintactico::runtime::parse_tree::ParseNode;

pub struct AnalysisResult {
    pub table: SymbolTable,
    pub errors: ErrorCollector,
}

/// Punto de entrada: recorre `tree` según `spec` y devuelve la tabla de
/// símbolos resultante junto con todos los diagnósticos encontrados (no se
/// detiene en el primero — sigue recorriendo para reportar todos, mismo
/// espíritu que el modo pánico del parser LR).
pub fn analyze(tree: &ParseNode, spec: &SemanticSpec) -> AnalysisResult {
    let mut analyzer = Analyzer::new(spec);
    visitor::walk(tree, &mut analyzer);
    AnalysisResult { table: analyzer.table, errors: analyzer.errors }
}

/// Estado que `enter` deja para que el `exit` del MISMO nodo lo recoja — un
/// scope solo se cierra si este nodo lo abrió, y `members` solo se adjunta si
/// este nodo también declaró un nombre. Se apila un frame por cada nodo
/// visitado (incluidas las hojas) para que `exit` siempre tenga exactamente
/// uno que desapilar, sin tener que volver a inspeccionar el `ParseNode`.
#[derive(Default)]
struct Frame {
    entered_scope: bool,
    declared_name: Option<String>,
}

struct Analyzer<'a> {
    spec: &'a SemanticSpec,
    table: SymbolTable,
    errors: ErrorCollector,
    frames: Vec<Frame>,
}

impl<'a> Analyzer<'a> {
    fn new(spec: &'a SemanticSpec) -> Self {
        Analyzer { spec, table: SymbolTable::new(), errors: ErrorCollector::new(), frames: Vec::new() }
    }
}

impl<'a> Visitor for Analyzer<'a> {
    fn enter(&mut self, node: &ParseNode) -> Flow {
        // Una hoja de identificador solo llega hasta acá si su padre NO la
        // consumió como nombre de una declaración (esos hijos se saltan con
        // `Flow::SkipChild`, ver abajo) — así que es un uso real.
        if node.children.is_empty() && node.symbol == self.spec.identifier_token {
            let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
            if let Err(e) = self.table.lookup_or_err(name, node.line, node.col) {
                self.errors.push_semantic(&e);
            }
            self.frames.push(Frame::default());
            return Flow::SkipChildren;
        }

        let decl_rule = self.spec.declarations.iter().find(|r| r.production == node.symbol);
        let scope_rule = self.spec.scopes.iter().find(|r| r.production == node.symbol);

        let mut consumed_index: Option<usize> = None;
        // Nombre recién declarado por ESTE nodo, si lo hubo — se usa en `exit`
        // para adjuntarle a ese símbolo los suyos propios como `members` una
        // vez cerrado el scope que este mismo nodo abrió (p.ej. func_decl
        // declara "foo" Y abre el scope de su cuerpo; al cerrarlo, los
        // parámetros/locales de "foo" quedan colgados de "foo").
        let mut declared_name: Option<String> = None;

        if let Some(rule) = decl_rule {
            if let Some((idx, name_node)) =
                find_identifier_child(node, &self.spec.identifier_token, rule.name_child)
            {
                consumed_index = Some(idx);
                let name = name_node.lexeme.as_deref().unwrap_or(&name_node.symbol).to_string();

                // `implicit`: si ya es visible en algún scope, esto es una
                // reasignación, no una declaración nueva — no tocar la tabla.
                let should_declare = !rule.implicit || self.table.lookup(&name).is_none();
                if should_declare {
                    if let Err(e) = self.table.declare(&name, rule.kind.clone(), name_node.line, name_node.col) {
                        self.errors.push_semantic(&e);
                    }
                }
                declared_name = Some(name);
            }
        }

        let entered_scope = if let Some(rule) = scope_rule {
            let label = if rule.with_label {
                // Mismo auto-hallazgo que la declaración — si esta producción
                // también declaraba (p.ej. func_decl), reusa el índice para no
                // buscarlo dos veces; si no, busca desde cero.
                find_identifier_child(node, &self.spec.identifier_token, decl_rule.and_then(|d| d.name_child))
                    .and_then(|(_, n)| n.lexeme.clone())
            } else {
                None
            };
            match label {
                Some(l) => self.table.enter_scope_named(rule.kind, l),
                None => self.table.enter_scope(rule.kind),
            }
            true
        } else {
            false
        };

        self.frames.push(Frame { entered_scope, declared_name });

        match consumed_index {
            Some(idx) => Flow::SkipChild(idx),
            None => Flow::Continue,
        }
    }

    fn exit(&mut self, _node: &ParseNode) {
        let frame = self.frames.pop().expect("enter empujó un frame para cada nodo visitado");
        if frame.entered_scope {
            // El scope que se cierra es el que este mismo `enter` acaba de
            // abrir arriba — nunca puede ser el Global, así que esto no falla.
            let closed = self
                .table
                .exit_scope()
                .expect("el scope recién abierto por este nodo debe poder cerrarse");

            // Límite conocido, no arreglado a propósito: si el cuerpo tiene un
            // scope anónimo anidado adentro (p.ej. un `bloque` que no declara
            // nada por sí mismo, solo abre Block), lo declarado ahí adentro NO
            // se aplana hacia arriba — un local declarado dos niveles adentro
            // de una función no aparece en `members` de la función, solo lo
            // que cuelga directo de su propio scope (sus parámetros).
            // Aplanarlo "hacia arriba a través de scopes anónimos" es viable
            // pero corre el riesgo real de filtrar la visibilidad de ese
            // nombre más allá de su bloque si se hace reinsertándolo en la
            // tabla viva (rompería el lookup con scoping correcto que ya está
            // bien probado) — se deja pendiente para cuando haga falta de
            // verdad, con un mecanismo de acumulación aparte de la tabla de
            // lookup.
            if let Some(name) = &frame.declared_name {
                if let Some(sym) = self.table.lookup_mut(name) {
                    sym.members = Some(closed.symbols().cloned().collect());
                }
            }
        }
    }
}

/// Busca, entre los hijos DIRECTOS de `node`, el que representa el nombre
/// declarado: en el índice explícito si se dio uno, o si no el primer hijo
/// cuyo `symbol` sea `identifier_token`. Devuelve también su índice, para que
/// el llamador pueda excluirlo de la recursión genérica (ya fue consumido
/// como declaración, no debe procesarse de nuevo como uso).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantico::errors::ErrorKind;
    use crate::semantico::scopes::ScopeKind;
    use crate::semantico::spec::{DeclarationRule, ScopeRule};
    use crate::semantico::symbols::SymbolKind;

    fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(lexeme.to_string()), children: vec![], line, col }
    }

    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        // Ojo: a diferencia de ParseNode::internal, los tests usan 0/0 fijo
        // para no acoplar las aserciones de posición del walker a la herencia
        // de posición de nodos internos (probada aparte en parse_tree.rs).
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
    fn undeclared_identifier_leaf_produces_one_ambito_diagnostic() {
        let expr = internal("expr", vec![leaf("ID", "z", 3, 9)]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
        };

        let result = analyze(&expr, &spec);
        assert_eq!(result.errors.len(), 1);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.kind, ErrorKind::Ambito);
        assert_eq!(diag.code, "S002");
        assert_eq!((diag.line, diag.col), (3, 9));
        assert!(diag.message.contains('z'));
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
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.kind, ErrorKind::Ambito);
        assert!(diag.message.contains('x'));
    }
}
