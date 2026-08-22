// Walker genérico (Fase 15), ahora como `impl Visitor for Analyzer` sobre el
// driver de `super::visitor` — este archivo NUNCA menciona el nombre de una
// producción concreta (ni "var_decl" ni "func_decl" ni nada por el estilo).
// Toda la especificidad de una gramática dada vive en el `SemanticSpec` que
// se le pasa a `analyze`, no acá.
use crate::semantico::closures::ClosureCollector;
use crate::semantico::errors::ErrorCollector;
use crate::semantico::scopes::ScopeKind;
use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::{SemanticError, SymbolKind, SymbolTable};
use crate::semantico::types::Type;
use crate::semantico::visitor::{self, Flow, Visitor};
use crate::sintactico::runtime::parse_tree::ParseNode;

pub struct AnalysisResult {
    pub table: SymbolTable,
    pub errors: ErrorCollector,
    /// Funciones anidadas que capturan variables/parámetros de una función
    /// encerradora (no globales, no propios) — ver `closures::ClosureCollector`.
    pub closures: ClosureCollector,
}

/// Punto de entrada: recorre `tree` según `spec` y devuelve la tabla de
/// símbolos resultante junto con todos los diagnósticos encontrados (no se
/// detiene en el primero — sigue recorriendo para reportar todos, mismo
/// espíritu que el modo pánico del parser LR).
pub fn analyze(tree: &ParseNode, spec: &SemanticSpec) -> AnalysisResult {
    let mut analyzer = Analyzer::new(spec);
    visitor::walk(tree, &mut analyzer);
    AnalysisResult { table: analyzer.table, errors: analyzer.errors, closures: analyzer.closures }
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
    /// `true` si el scope que este nodo abrió era `Function` — así `exit`
    /// sabe si debe desapilar `function_stack` además de cerrar el scope.
    opened_function: bool,
}

struct Analyzer<'a> {
    spec: &'a SemanticSpec,
    table: SymbolTable,
    errors: ErrorCollector,
    closures: ClosureCollector,
    frames: Vec<Frame>,
    /// Pila de funciones activas: `(profundidad ABSOLUTA del scope propio de
    /// la función, su nombre)`. El tope es la función que se está recorriendo
    /// ahora mismo — cualquier uso que resuelva a un nombre en una profundidad
    /// MENOR (pero no Global) es una variable libre que esa función captura
    /// de su entorno de definición.
    function_stack: Vec<(usize, String)>,
}

impl<'a> Analyzer<'a> {
    fn new(spec: &'a SemanticSpec) -> Self {
        Analyzer {
            spec,
            table: SymbolTable::new(),
            errors: ErrorCollector::new(),
            closures: ClosureCollector::new(),
            frames: Vec::new(),
            function_stack: Vec::new(),
        }
    }

    /// Si la declaración recién hecha trae anotación de tipo (`spec.
    /// type_children`), la resuelve y se la asigna al símbolo — así los
    /// campos de un record/clase (o cualquier variable/parámetro/constante
    /// con tipo) quedan tipados de verdad, no solo declarados. Las clases se
    /// tipan aparte, siempre como `Type::Named(su propio nombre)`: un record
    /// definido por el usuario ES su propio tipo nominal, sin necesitar un
    /// hijo de tipo que lo diga.
    fn apply_declared_type(&mut self, node: &ParseNode, kind: &SymbolKind, name: &str) {
        let ty = if *kind == SymbolKind::Class {
            Some(Type::Named(name.to_string()))
        } else {
            self.spec
                .type_children
                .get(&node.symbol)
                .and_then(|child_symbol| node.children.iter().find(|c| &c.symbol == child_symbol))
                .and_then(|type_node| self.spec.resolve_type(type_node))
        };
        if let Some(ty) = ty {
            if let Some(sym) = self.table.lookup_mut(name) {
                sym.ty = Some(ty);
            }
        }
    }
}

impl<'a> Visitor for Analyzer<'a> {
    fn enter(&mut self, node: &ParseNode) -> Flow {
        // Una hoja de identificador solo llega hasta acá si su padre NO la
        // consumió como nombre de una declaración (esos hijos se saltan con
        // `Flow::SkipChild`, ver abajo) — así que es un uso real.
        if node.children.is_empty() && node.symbol == self.spec.identifier_token {
            let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
            match self.table.lookup_with_scope(name) {
                Some((sym, def_depth, _def_kind)) => {
                    // Resolución de nombres libres: si hay una función activa
                    // (el tope de function_stack) y este nombre vive en una
                    // profundidad MENOR que la del scope propio de esa
                    // función (pero no en el Global, profundidad 0), es una
                    // variable/parámetro del entorno de definición — una
                    // captura, no un local. Las funciones y clases NO cuentan
                    // como captura: llamar a un vecino o a sí misma (recursión)
                    // es resolución de nombre normal, no cerrar sobre datos.
                    if let Some(&(boundary_depth, ref fn_name)) = self.function_stack.last() {
                        let is_capturable = matches!(sym.kind, SymbolKind::Variable | SymbolKind::Parameter);
                        if def_depth < boundary_depth && def_depth > 0 && is_capturable {
                            let fn_name = fn_name.clone();
                            self.closures.record_capture(&fn_name, name, node.line, node.col);
                        }
                    }
                }
                None => {
                    self.errors.push_semantic(&SemanticError::Undeclared {
                        name: name.to_string(),
                        line: node.line,
                        col: node.col,
                    });
                }
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
                    } else {
                        self.apply_declared_type(node, &rule.kind, &name);
                    }
                }
                declared_name = Some(name);
            }
        }

        let mut opened_function = false;
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

            // Profundidad ABSOLUTA que ocupará el scope de esta función — la
            // pila tiene `depth()` elementos antes de empujar el nuevo, así
            // que ese es exactamente el índice donde va a quedar.
            let this_fn_depth = self.table.depth();

            match &label {
                Some(l) => self.table.enter_scope_named(rule.kind, l.clone()),
                None => self.table.enter_scope(rule.kind),
            }

            if rule.kind == ScopeKind::Function {
                opened_function = true;
                // El nombre que declaró ESTE mismo nodo (func_decl declara Y
                // abre scope a la vez); si no hay uno (una función sin nombre
                // no existe en esta gramática, pero no hay por qué asumirlo
                // en un walker agnóstico), usar un rótulo sintético con su
                // posición para que las capturas sigan siendo atribuibles.
                let fn_name = declared_name.clone().or(label).unwrap_or_else(|| format!("<fn@{}:{}>", node.line, node.col));
                self.function_stack.push((this_fn_depth, fn_name));
            }

            true
        } else {
            false
        };

        self.frames.push(Frame { entered_scope, declared_name, opened_function });

        match consumed_index {
            Some(idx) => Flow::SkipChild(idx),
            None => Flow::Continue,
        }
    }

    fn exit(&mut self, _node: &ParseNode) {
        let frame = self.frames.pop().expect("enter empujó un frame para cada nodo visitado");
        if frame.opened_function {
            self.function_stack.pop();
        }
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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

    /// Spec compartida por los tests de closures/tipado de abajo: func_decl/
    /// var_decl/param/class_decl declaran, func_decl/class_decl abren scope,
    /// y var_decl/param traen tipo vía un hijo "tipo".
    fn closures_and_types_spec() -> SemanticSpec {
        SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![
                DeclarationRule { production: "func_decl".to_string(), kind: SymbolKind::Function, name_child: None, implicit: false },
                DeclarationRule { production: "var_decl".to_string(), kind: SymbolKind::Variable, name_child: None, implicit: false },
                DeclarationRule { production: "param".to_string(), kind: SymbolKind::Parameter, name_child: None, implicit: false },
                DeclarationRule { production: "class_decl".to_string(), kind: SymbolKind::Class, name_child: None, implicit: false },
            ],
            scopes: vec![
                ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
                ScopeRule { production: "class_decl".to_string(), kind: ScopeKind::Class, with_label: true },
            ],
            type_children: [("var_decl".to_string(), "tipo".to_string()), ("param".to_string(), "tipo".to_string())]
                .into_iter()
                .collect(),
            type_tokens: [("INT_T".to_string(), Type::Int), ("BOOL_T".to_string(), Type::Bool)].into_iter().collect(),
        }
    }

    fn tipo(leaf_symbol: &str, leaf_lexeme: &str) -> ParseNode {
        internal("tipo", vec![leaf(leaf_symbol, leaf_lexeme, 0, 0)])
    }

    #[test]
    fn nested_function_captures_a_variable_from_the_enclosing_function() {
        // function outer() { var x; function inner() { <uso de x> } }
        let var_x = internal("var_decl", vec![leaf("ID", "x", 2, 3)]);
        let inner = internal("func_decl", vec![
            leaf("FUN", "fun", 3, 3),
            leaf("ID", "inner", 3, 8),
            leaf("ID", "x", 4, 10), // uso de "x", libre respecto de inner
        ]);
        let outer = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "outer", 1, 5),
            var_x,
            inner,
        ]);

        let result = analyze(&outer, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(!result.closures.is_empty());
        let caps = result.closures.captures_of("inner").expect("inner debe capturar algo");
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].name, "x");
        assert_eq!((caps[0].line, caps[0].col), (4, 10));
    }

    #[test]
    fn using_only_its_own_locals_and_params_is_not_a_capture() {
        // function f(p) { var x; <uso de p>; <uso de x> } — nada libre.
        let param_p = internal("param", vec![leaf("ID", "p", 1, 10)]);
        let var_x = internal("var_decl", vec![leaf("ID", "x", 2, 3)]);
        let f = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "f", 1, 5),
            param_p,
            var_x,
            leaf("ID", "p", 3, 1),
            leaf("ID", "x", 3, 5),
        ]);

        let result = analyze(&f, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.closures.is_empty(), "p y x son locales/propios de f, no capturas");
    }

    #[test]
    fn self_recursion_is_not_treated_as_a_capture() {
        // function fact() { <llamada recursiva a fact> } — el nombre de la
        // propia función (SymbolKind::Function) nunca cuenta como captura,
        // solo variables/parámetros.
        let fact = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "fact", 1, 5),
            leaf("ID", "fact", 2, 3), // llamada recursiva
        ]);

        let result = analyze(&fact, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.closures.is_empty(), "llamarse a sí misma no es capturar una variable");
    }

    #[test]
    fn using_a_global_variable_inside_a_function_is_not_a_capture() {
        // var g;  function f() { <uso de g> } — g es global, siempre
        // alcanzable, no necesita capturarse.
        let var_g = internal("var_decl", vec![leaf("ID", "g", 1, 5)]);
        let f = internal("func_decl", vec![
            leaf("FUN", "fun", 2, 1),
            leaf("ID", "f", 2, 5),
            leaf("ID", "g", 3, 3),
        ]);
        let programa = internal("programa", vec![var_g, f]);

        let result = analyze(&programa, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.closures.is_empty(), "una global no se captura");
    }

    #[test]
    fn capture_is_attributed_to_the_innermost_function_two_levels_deep() {
        // function a() { var x; function b() { function c() { <uso de x> } } }
        // "x" es libre para "c" (y transitivamente para "b", pero "b" nunca
        // lo usa directamente — solo "c" lo referencia, así que solo "c"
        // debe aparecer con una captura).
        let var_x = internal("var_decl", vec![leaf("ID", "x", 1, 10)]);
        let c = internal("func_decl", vec![
            leaf("FUN", "fun", 4, 5),
            leaf("ID", "c", 4, 10),
            leaf("ID", "x", 5, 7),
        ]);
        let b = internal("func_decl", vec![
            leaf("FUN", "fun", 3, 3),
            leaf("ID", "b", 3, 8),
            c,
        ]);
        let a = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "a", 1, 5),
            var_x,
            b,
        ]);

        let result = analyze(&a, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.closures.captures_of("b").is_none(), "b nunca usa x directamente");
        let caps = result.closures.captures_of("c").expect("c captura x de 'a', dos niveles arriba");
        assert_eq!(caps[0].name, "x");
    }

    #[test]
    fn var_decl_with_type_annotation_gets_a_real_type() {
        let var_x = internal("var_decl", vec![leaf("ID", "x", 1, 1), tipo("INT_T", "integer")]);
        let result = analyze(&var_x, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.table.lookup("x").unwrap().ty, Some(Type::Int));
    }

    #[test]
    fn param_with_type_annotation_gets_a_real_type() {
        let param_p = internal("param", vec![leaf("ID", "p", 1, 1), tipo("BOOL_T", "boolean")]);
        let result = analyze(&param_p, &closures_and_types_spec());
        assert!(result.errors.is_empty());
        assert_eq!(result.table.lookup("p").unwrap().ty, Some(Type::Bool));
    }

    #[test]
    fn var_decl_without_type_annotation_stays_untyped() {
        let var_x = internal("var_decl", vec![leaf("ID", "x", 1, 1)]);
        let result = analyze(&var_x, &closures_and_types_spec());
        assert_eq!(result.table.lookup("x").unwrap().ty, None);
    }

    #[test]
    fn class_decl_is_typed_as_a_named_type_of_its_own_name() {
        // Records/structs definidos por el usuario reutilizan class_decl: la
        // clase misma ES su tipo nominal, sin necesitar un hijo de tipo.
        let campo = internal("var_decl", vec![leaf("ID", "valor", 2, 3), tipo("INT_T", "integer")]);
        let contador = internal("class_decl", vec![
            leaf("CLASS", "class", 1, 1),
            leaf("ID", "Contador", 1, 7),
            campo,
        ]);

        let result = analyze(&contador, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let sym = result.table.lookup("Contador").expect("Contador se declaró");
        assert_eq!(sym.ty, Some(Type::Named("Contador".to_string())));

        // Y su campo "valor" quedó tipado como Int, colgado como member.
        let members = sym.members.as_ref().expect("class_decl abrió un scope");
        let valor = members.iter().find(|m| m.name == "valor").expect("valor es campo de Contador");
        assert_eq!(valor.ty, Some(Type::Int));
    }
}
