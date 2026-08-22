// Walker genérico (Fase 15/16), ahora como `impl Visitor for Analyzer` sobre
// el driver de `super::visitor` — este archivo NUNCA menciona el nombre de
// una producción concreta (ni "var_decl" ni "func_decl" ni "class_decl" ni
// nada por el estilo). Toda la especificidad de una gramática dada vive en
// el `SemanticSpec` que se le pasa a `analyze`, no acá — incluida la parte
// de clases/objetos (Fase 16: `this`, acceso a miembros con `.`,
// instanciación con `new`), que solo se activa si `spec` trae esa
// configuración (`this_token`/`member_access`/`instantiation`); si no,
// el walker se comporta exactamente igual que antes de que existiera.
use crate::semantico::classes;
use crate::semantico::errors::ErrorCollector;
use crate::semantico::scopes::ScopeKind;
use crate::semantico::spec::{SemanticSpec, ChildLocator};
use crate::semantico::symbols::{SemanticError, Signature, SymbolKind, SymbolTable};
use crate::semantico::types::{resolve_arithmetic, resolve_assignment, Type};
use crate::semantico::visitor::{self, Flow, Visitor};
use crate::sintactico::runtime::parse_tree::ParseNode;
use std::collections::HashSet;

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
    analyzer.report_unknown_named_types();
    AnalysisResult { table: analyzer.table, errors: analyzer.errors }
}

/// Estado que `enter` deja para que el `exit` del MISMO nodo lo recoja — un
/// scope solo se cierra si este nodo lo abrió, y `members`/`signature` solo
/// se adjuntan si este nodo también declaró un nombre. Se apila un frame por
/// cada nodo visitado (incluidas las hojas) para que `exit` siempre tenga
/// exactamente uno que desapilar, sin tener que volver a inspeccionar el
/// `ParseNode`.
#[derive(Default)]
struct Frame {
    entered_scope: bool,
    declared_name: Option<String>,
    /// El `SymbolKind` que este nodo declaró, si alguno — lo usa `exit` para
    /// decidir si construir una `Signature` (solo tiene sentido para algo
    /// invocable, no para una `Class`/`Variable`/`Parameter`).
    declared_kind: Option<SymbolKind>,
    /// Nombres de los parámetros declarados DIRECTAMENTE dentro del scope
    /// que este frame abrió, en orden de aparición — lo llenan los nodos
    /// hijos de tipo `Parameter` (buscando el frame abierto más cercano
    /// hacia atrás en la pila), y lo consume `exit` para armar `Signature`.
    param_order: Vec<String>,
}

struct Analyzer<'a> {
    spec: &'a SemanticSpec,
    table: SymbolTable,
    errors: ErrorCollector,
    frames: Vec<Frame>,
    /// TODOS los nombres de clase declarados en cualquier punto del archivo.
    /// A diferencia de la tabla de símbolos, este set nunca se vacía cuando
    /// un scope se cierra — ver `report_unknown_named_types`.
    declared_class_names: HashSet<String>,
    /// Cada `Type::Named` que apareció como anotación de tipo, con su
    /// posición, para validarlo al terminar el recorrido.
    pending_named_types: Vec<(String, usize, usize)>,
}

impl<'a> Analyzer<'a> {
    fn new(spec: &'a SemanticSpec) -> Self {
        Analyzer {
            spec,
            table: SymbolTable::new(),
            errors: ErrorCollector::new(),
            frames: Vec::new(),
            declared_class_names: HashSet::new(),
            pending_named_types: Vec::new(),
        }
    }

    /// Reporta las anotaciones de tipo que nombran una clase que nunca se
    /// declaró (`let x: NoExiste`). Corre DESPUÉS del recorrido completo, no
    /// durante: la gramática no exige declarar antes de usar, así que una
    /// clase puede aparecer más abajo en el archivo que su primer uso como
    /// tipo.
    ///
    /// Se contrasta contra `declared_class_names` (acumulado, nunca vaciado)
    /// y no contra la tabla de símbolos, porque al terminar el walk todos los
    /// scopes están cerrados: una clase declarada dentro de un bloque ya no
    /// sería visible ahí y daría un FALSO POSITIVO. El set acumulado solo
    /// puede errar por permisivo (nunca marca como inexistente algo que sí se
    /// declaró en algún lado), que es la dirección segura.
    ///
    /// Supuesto: un `Type::Named` nombra una clase. Una gramática con alias
    /// de tipo necesitaría distinguirlos antes de usar esta validación.
    fn report_unknown_named_types(&mut self) {
        for (name, line, col) in &self.pending_named_types {
            if !self.declared_class_names.contains(name) {
                self.errors.push_semantic(&SemanticError::UnknownClass {
                    name: name.clone(),
                    line: *line,
                    col: *col,
                });
            }
        }
    }
}

impl<'a> Visitor for Analyzer<'a> {
    fn enter(&mut self, node: &ParseNode) -> Flow {
        // 1. Una hoja de identificador solo llega hasta acá si su padre NO la
        // consumió con otro significado (nombre declarado, nombre de
        // miembro, nombre de clase en `new`...) — esos hijos se saltan con
        // `Flow::SkipChildIndices`, ver más abajo — así que es un uso real.
        if node.children.is_empty() && node.symbol == self.spec.identifier_token {
            let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
            if let Err(e) = self.table.lookup_or_err(name, node.line, node.col) {
                self.errors.push_semantic(&e);
            }
            self.frames.push(Frame::default());
            return Flow::SkipChildren;
        }

        // 2. Hoja `this`: resuelve al símbolo "this" auto-declarado al abrir
        // el scope del método actual (ver más abajo) — si no existe, `this`
        // se usó fuera de un método de clase.
        if node.children.is_empty() && Some(&node.symbol) == self.spec.this_token.as_ref() {
            if self.table.lookup("this").is_none() {
                self.errors.push_semantic(&SemanticError::ThisOutsideClass { line: node.line, col: node.col });
            }
            self.frames.push(Frame::default());
            return Flow::SkipChildren;
        }

        // 3. Acceso a miembro, en sus dos formas (según `spec.member_access`):
        // LECTURA (`primary: primary DOT ID`) y ASIGNACIÓN
        // (`assign_stmt: primary DOT ID ASSIGN expr`). Se distinguen por la
        // forma del nodo, no por configuración: si hay hijos MÁS ALLÁ del
        // nombre del miembro, el último es el valor que se le asigna.
        // Si esta instancia de la producción no trae el punto (p.ej.
        // `primary: atom`, `assign_stmt: ID ASSIGN expr`), no dispara — sigue
        // como producción normal más abajo.
        if let Some((_, member_idx)) = classes::find_member_access(node, self.spec) {
            if let (Some(base), Some(member_id)) = (node.children.first(), node.children.get(member_idx)) {
                let base_ty = classes::resolve_expr_type(base, &self.table, self.spec);
                if let Some(Type::Named(class_name)) = base_ty {
                    let member_name = member_id.lexeme.as_deref().unwrap_or(&member_id.symbol);
                    match classes::resolve_member(&self.table, &class_name, member_name, member_id.line, member_id.col) {
                        Ok(member) => {
                            // Forma de asignación: chequear el valor contra el
                            // tipo declarado del miembro, con la misma tabla de
                            // coerciones y el mismo diagnóstico que ya usa
                            // `SymbolTable::assign` para una variable suelta.
                            // Un valor cuyo tipo no sabemos resolver (expresión
                            // compuesta) simplemente no se chequea.
                            let expected = member.ty.clone();
                            if let (Some(value_node), Some(expected)) =
                                (node.children.last().filter(|_| node.children.len() > member_idx + 1), expected)
                            {
                                if let Some(found) = classes::resolve_expr_type(value_node, &self.table, self.spec) {
                                    if resolve_assignment(&expected, &found).is_err() {
                                        self.errors.push_semantic(&SemanticError::AssignmentTypeMismatch {
                                            name: format!("{class_name}.{member_name}"),
                                            expected,
                                            found,
                                            line: member_id.line,
                                            col: member_id.col,
                                        });
                                    }
                                }
                            }
                        }
                        Err(e) => self.errors.push_semantic(&e),
                    }
                }
                // `base_ty` es `None` o no es `Named`: no se reporta
                // nada — no sabemos el tipo de la base (expresión
                // compuesta), no es un error de por sí.
            }
            self.frames.push(Frame::default());
            return Flow::SkipChildIndices(vec![member_idx]);
        }

        // 2b. Operación aritmética (`izq OP der`, según `spec.arith_tokens`):
        // valida los operandos con la tabla de compatibilidad de `types`.
        // No hace early-return — los operandos siguen recorriéndose normal, y
        // si alguno es a su vez una operación inválida también se reporta.
        // Un operando cuyo tipo no sabemos resolver no se chequea.
        if let Some((op, left, right)) = classes::find_arithmetic(node, self.spec) {
            let left_ty = classes::resolve_expr_type(left, &self.table, self.spec);
            let right_ty = classes::resolve_expr_type(right, &self.table, self.spec);
            if let (Some(l), Some(r)) = (left_ty, right_ty) {
                if resolve_arithmetic(op, &l, &r).is_err() {
                    self.errors.push_semantic(&SemanticError::InvalidArithmetic {
                        operator: op.to_string(),
                        left: l,
                        right: r,
                        line: node.children[1].line,
                        col: node.children[1].col,
                    });
                }
            }
        }

        // 3a. Asignación a una variable suelta (`assign_stmt: ID ASSIGN expr`,
        // según `spec.assign`). Solo dispara si el destino es una HOJA
        // identificador: la otra alternativa del mismo head
        // (`primary DOT ID ASSIGN expr`) ya la manejó el bloque de acceso a
        // miembro de arriba, que devuelve antes de llegar acá.
        if let Some(rule) = &self.spec.assign {
            if node.symbol == rule.production {
                let target = node.children.get(rule.target_index).filter(|c| {
                    c.children.is_empty() && c.symbol == self.spec.identifier_token
                });
                if let (Some(target), Some(value_node)) = (target, node.children.get(rule.value_index)) {
                    let name = target.lexeme.as_deref().unwrap_or(&target.symbol).to_string();
                    let value_type = classes::resolve_expr_type(value_node, &self.table, self.spec);
                    if let Err(e) = self.table.assign(&name, value_type.as_ref(), target.line, target.col) {
                        self.errors.push_semantic(&e);
                    }
                    // El destino ya se consumió acá (y `assign` ya reportó si
                    // no existía): excluirlo del recorrido evita un segundo
                    // diagnóstico "no declarada" por el mismo identificador.
                    self.frames.push(Frame::default());
                    return Flow::SkipChildIndices(vec![rule.target_index]);
                }
            }
        }

        // 3b. Invocación (`primary: primary LPAREN arg_list RPAREN`, según
        // `spec.call`) — cubre por igual `f(args)` y `obj.metodo(args)`,
        // porque lo invocado es simplemente la subexpresión de la izquierda.
        // A diferencia de los bloques de arriba, este NO hace early-return ni
        // salta ningún hijo: el nodo interno de acceso a miembro tiene que
        // seguir validando que el método exista, y los identificadores dentro
        // de los argumentos siguen siendo usos normales.
        if let Some(rule) = &self.spec.call {
            if node.symbol == rule.production
                && node.children.iter().any(|c| c.symbol == rule.open_paren_token)
            {
                if let (Some(callee_node), Some(arg_list_node), Some(args_list_symbol)) = (
                    node.children.get(rule.callee_index),
                    node.children.get(rule.arg_list_index),
                    self.spec.args_list_symbol.as_deref(),
                ) {
                    if let Some((callee, label)) = classes::resolve_callee(callee_node, &self.table, self.spec) {
                        let arg_nodes = classes::flatten_arg_list(arg_list_node, args_list_symbol);
                        let errs = classes::validate_call(
                            &self.table,
                            self.spec,
                            callee,
                            &label,
                            (callee_node.line, callee_node.col),
                            &arg_nodes,
                        );
                        for e in errs {
                            self.errors.push_semantic(&e);
                        }
                    }
                }
            }
        }

        // 4. Instanciación (`atom: NEW ID LPAREN arg_list RPAREN`, según
        // `spec.instantiation`). Se identifica por la producción Y por que
        // el primer hijo sea literalmente el token `new` — así no dispara
        // para otras alternativas de la misma producción (`atom: ID`, etc.).
        if let Some(rule) = &self.spec.instantiation {
            if node.symbol == rule.production
                && node.children.first().map(|c| c.symbol.as_str()) == Some(rule.new_token.as_str())
            {
                if let (Some(class_id), Some(arg_list_node)) =
                    (node.children.get(rule.class_name_index), node.children.get(rule.arg_list_index))
                {
                    let class_name = class_id.lexeme.as_deref().unwrap_or(&class_id.symbol).to_string();
                    let args_list_symbol = self.spec.args_list_symbol.as_deref().unwrap_or_default();
                    let arg_nodes = classes::flatten_arg_list(arg_list_node, args_list_symbol);
                    for e in classes::validate_instantiation(
                        &self.table,
                        self.spec,
                        &class_name,
                        (class_id.line, class_id.col),
                        &arg_nodes,
                    ) {
                        self.errors.push_semantic(&e);
                    }
                    self.frames.push(Frame::default());
                    return Flow::SkipChildIndices(vec![rule.class_name_index]);
                }
            }
        }

        // 5. Declaración/scope genérico — como en la Fase 15, extendido con
        // tipo (`type_child`), herencia de clase, tracking de parámetros
        // para `Signature`, y auto-declaración de `this`.
        let decl_rule = self.spec.declarations.iter().find(|r| r.production == node.symbol);
        let scope_rule = self.spec.scopes.iter().find(|r| r.production == node.symbol);

        let mut skip_indices: Vec<usize> = Vec::new();
        // Nombre/kind recién declarados por ESTE nodo, si los hubo — se usan
        // en `exit` para adjuntarle a ese símbolo sus propios `members` (y su
        // `Signature`, si aplica) una vez cerrado el scope que este mismo
        // nodo abrió.
        let mut declared_name: Option<String> = None;
        let mut declared_kind: Option<SymbolKind> = None;
        let mut parent_name: Option<String> = None;

        if let Some(rule) = decl_rule {
            if let Some((idx, name_node)) = find_identifier_child(node, &self.spec.identifier_token, rule.name_child) {
                skip_indices.push(idx);
                let name = name_node.lexeme.as_deref().unwrap_or(&name_node.symbol).to_string();

                // `implicit`: si ya es visible en algún scope, esto es una
                // reasignación, no una declaración nueva — no tocar la tabla.
                let should_declare = !rule.implicit || self.table.lookup(&name).is_none();
                if should_declare {
                    // `type_child` marca cuál hijo directo es el nodo de tipo
                    // (p.ej. el `tipo` de `var_decl: tipo ID`). Si la regla lo
                    // trae y esta alternativa concreta lo tiene, se resuelve
                    // ese subárbol a un `Type` y se declara con
                    // `declare_typed` para dejar `ty` poblado (y se excluye
                    // ese hijo de la recursión genérica, para no re-visitarlo
                    // como uso) — si no, sigue siendo el `declare` de
                    // siempre, sin tipo.
                    let type_idx = rule.type_child.as_ref().and_then(|locator| find_child_index(node, locator));

                    // El inicializador (`= expr`), si esta alternativa lo
                    // trae. NO se agrega a `skip_indices`: es una expresión
                    // real, y los identificadores que use tienen que seguir
                    // validándose por el recorrido normal.
                    let init_node = rule
                        .init_child
                        .as_ref()
                        .and_then(|locator| find_child_index(node, locator))
                        .map(|i| &node.children[i]);
                    let init_type = init_node.and_then(|n| classes::resolve_expr_type(n, &self.table, self.spec));

                    let decl_result = match type_idx {
                        Some(i) => {
                            skip_indices.push(i);
                            let ty = resolve_declared_type(&node.children[i], self.spec);
                            // Un tipo con nombre (una clase usada como tipo)
                            // se anota para validarlo al final del recorrido
                            // — acá todavía no se puede, la clase podría
                            // declararse más abajo en el archivo.
                            if let Type::Named(class_name) = &ty {
                                let type_node = &node.children[i];
                                self.pending_named_types.push((class_name.clone(), type_node.line, type_node.col));
                            }
                            self.table.declare_typed(
                                &name,
                                rule.kind.clone(),
                                ty,
                                !rule.immutable,
                                init_node.is_some(),
                                init_type,
                                name_node.line,
                                name_node.col,
                            )
                        }
                        // Sin tipo declarado: si hay inicializador y su tipo
                        // se pudo resolver, se INFIERE de él (`let x = 5`
                        // hace `x: integer`, y entonces `x = "texto"` sí se
                        // detecta más adelante). Si no, declaración sin tipo,
                        // igual que siempre.
                        None => match &init_type {
                            Some(inferred) => self.table.declare_typed(
                                &name,
                                rule.kind.clone(),
                                inferred.clone(),
                                !rule.immutable,
                                true,
                                None,
                                name_node.line,
                                name_node.col,
                            ),
                            None if rule.immutable => self.table.declare_typed(
                                &name,
                                rule.kind.clone(),
                                Type::Unknown,
                                false,
                                init_node.is_some(),
                                None,
                                name_node.line,
                                name_node.col,
                            ),
                            None => self.table.declare(&name, rule.kind.clone(), name_node.line, name_node.col),
                        },
                    };
                    match decl_result {
                        Ok(()) => {
                            // Parámetro declarado con éxito: registrarlo en el
                            // `param_order` del scope contenedor (el frame
                            // abierto más cercano hacia atrás en la pila) —
                            // lo usa `exit` para armar la `Signature` de la
                            // función/método que lo contiene.
                            if rule.kind == SymbolKind::Parameter {
                                if let Some(parent_frame) = self.frames.iter_mut().rev().find(|f| f.entered_scope) {
                                    parent_frame.param_order.push(name.clone());
                                }
                            }
                        }
                        Err(e) => self.errors.push_semantic(&e),
                    }
                }
                declared_name = Some(name.clone());
                declared_kind = Some(rule.kind.clone());

                // Herencia: si esta declaración es una clase, un SEGUNDO
                // identificador directo (aparte del propio) es el nombre de
                // la clase padre (`class Hijo : Padre { ... }`).
                if rule.kind == SymbolKind::Class {
                    // Registro permanente para `report_unknown_named_types`:
                    // sobrevive al cierre del scope donde se declaró.
                    self.declared_class_names.insert(name);


                    if let Some((parent_idx, parent_node)) =
                        node.children.iter().enumerate().find(|(i, c)| *i != idx && c.symbol == self.spec.identifier_token)
                    {
                        skip_indices.push(parent_idx);
                        parent_name = Some(parent_node.lexeme.as_deref().unwrap_or(&parent_node.symbol).to_string());
                    }
                }
            }
        }

        // Setea `parent` en el símbolo de la clase que se acaba de declarar
        // (no valida acá que el padre exista todavía — Compiscript no exige
        // orden de declaración; se valida cuando de verdad se usa, en
        // `classes::resolve_member`).
        if let (Some(name), Some(parent)) = (&declared_name, &parent_name) {
            if let Some(sym) = self.table.lookup_mut(name) {
                sym.parent = Some(parent.clone());
            }
        }

        // Auto-declaración de `this`: si esta producción abre un scope
        // Function Y el scope en el que estamos parados AHORA MISMO (antes
        // de abrir el nuevo) es una Class, este scope es el cuerpo de un
        // método — captura el nombre de esa clase para declarar `this`
        // apenas se abra el scope nuevo.
        let enclosing_class_for_this = scope_rule.and_then(|rule| {
            if rule.kind == ScopeKind::Function && self.table.current_scope_kind() == ScopeKind::Class {
                self.table.current_scope_label().map(str::to_string)
            } else {
                None
            }
        });

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

            if let Some(class_name) = enclosing_class_for_this {
                self.table
                    .declare("this", SymbolKind::Variable, node.line, node.col)
                    .expect("el scope recién abierto está vacío, 'this' no puede estar ya declarado ahí");
                let this_sym = self.table.lookup_mut("this").expect("recién declarado");
                this_sym.ty = Some(Type::Named(class_name));
                this_sym.mutable = false;
                this_sym.initialized = true;
            }

            true
        } else {
            false
        };

        self.frames.push(Frame { entered_scope, declared_name, declared_kind, param_order: Vec::new() });

        if skip_indices.is_empty() {
            Flow::Continue
        } else {
            Flow::SkipChildIndices(skip_indices)
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

                    // `Signature`: efecto colateral puramente aditivo para
                    // cualquier símbolo "invocable" (Function/Other — nunca
                    // Class/Variable/Parameter, para los que no tiene
                    // sentido). Usa el orden de parámetros que fueron
                    // registrando sus propios nodos, y el tipo de retorno que
                    // `type_child` ya haya poblado en este mismo símbolo (o
                    // `Void` si no declaró ninguno). La usa
                    // `classes::constructor_signature` para validar `new`.
                    if matches!(frame.declared_kind, Some(SymbolKind::Function) | Some(SymbolKind::Other(_))) {
                        let params = frame
                            .param_order
                            .iter()
                            .map(|n| closed.get_own(n).and_then(|s| s.ty.clone()).unwrap_or(Type::Unknown))
                            .collect();
                        let returns = sym.ty.clone().unwrap_or(Type::Void);
                        sym.signature = Some(Signature { params, returns });
                    }
                }
            }
        }
    }
}

/// Resuelve el subárbol de un nodo de tipo (p.ej. el `tipo` que cuelga de
/// `var_decl: tipo ID`) al `Type` semántico que representa. Si la hoja
/// resultante es el propio `identifier_token` (un nombre de clase usado como
/// tipo, p.ej. `tipo: ID`), se resuelve a `Type::Named` — seguro porque
/// `enter` ya excluye ese hijo de la recursión genérica vía
/// `Flow::SkipChildIndices`, así que no se re-visita como uso. Si no,
/// desciende por el primer hijo hasta llegar a una hoja — la forma típica de
/// `tipo: BOOL_T | INT_T | ...`, una sola alternativa terminal por regla — y
/// la busca en `spec.type_tokens`. Un terminal que no está ahí no es un
/// error de por sí — cae en `Type::Unknown`, mismo significado de "todavía
/// no lo sabemos" que usa el resto del módulo.
fn resolve_declared_type(node: &ParseNode, spec: &SemanticSpec) -> Type {
    if node.symbol == spec.identifier_token {
        let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
        return Type::Named(name.to_string());
    }
    if let Some(first) = node.children.first() {
        return resolve_declared_type(first, spec);
    }
    spec.type_tokens.get(&node.symbol).cloned().unwrap_or(Type::Unknown)
}

/// Encuentra, entre los hijos DIRECTOS de `node`, el índice del nodo de tipo
/// según `locator`: `Index(i)` lo confirma con `i` si existe ese hijo;
/// `BySymbol(s)` busca el primer hijo directo cuyo `symbol` sea `s` — para
/// producciones con el nodo de tipo en posiciones distintas según la
/// alternativa (o ausente en algunas), como `var_decl` en Compiscript.
fn find_child_index(node: &ParseNode, locator: &ChildLocator) -> Option<usize> {
    match locator {
        ChildLocator::Index(i) => node.children.get(*i).map(|_| *i),
        ChildLocator::BySymbol(symbol) => node.children.iter().position(|c| &c.symbol == symbol),
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
    use crate::semantico::spec::{CallRule, DeclarationRule, MemberAccessRule, ScopeRule};
    use crate::semantico::symbols::SymbolKind;
    use std::collections::HashMap;

    fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(lexeme.to_string()), children: vec![], line, col }
    }

    fn internal(symbol: &str, children: Vec<ParseNode>) -> ParseNode {
        // Ojo: a diferencia de ParseNode::internal, los tests usan 0/0 fijo
        // para no acoplar las aserciones de posición del walker a la herencia
        // de posición de nodos internos (probada aparte en parse_tree.rs).
        ParseNode { symbol: symbol.to_string(), lexeme: None, children, line: 0, col: 0 }
    }

    /// Un `SemanticSpec` mínimo, sin nada de clases/objetos activado — el
    /// caso común de la mayoría de estos tests (Fase 15, previa a Fase 16).
    fn spec_without_classes(declarations: Vec<DeclarationRule>, scopes: Vec<ScopeRule>, type_tokens: HashMap<String, Type>) -> SemanticSpec {
        SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations,
            scopes,
            type_tokens,
            this_token: None,
            member_access: Vec::new(),
            instantiation: None,
            call: None,
            args_list_symbol: None,
            constructor_name: None,
            assign: None,
            arith_tokens: Default::default(),
        }
    }

    #[test]
    fn declare_via_matched_production_then_lookup_succeeds() {
        // var_decl: tipo ID  ->  [tipo-leaf, ID-leaf]
        let tree = internal("var_decl", vec![
            leaf("INT_T", "int", 1, 1),
            leaf("ID", "x", 1, 5),
        ]);
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: false,
                type_child: None,
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

        let result = analyze(&tree, &spec);
        assert!(result.errors.is_empty());
        let sym = result.table.lookup("x").expect("x se declaró");
        assert_eq!(sym.kind, SymbolKind::Variable);
        assert_eq!((sym.line, sym.col), (1, 5));
    }

    #[test]
    fn type_child_populates_declared_type() {
        // var_decl: tipo ID  ->  [tipo, ID-leaf], "tipo" envuelve el terminal
        // real (INT_T) tal como lo arma el parser sobre `tipo: INT_T | ...`.
        let tree = internal("var_decl", vec![
            internal("tipo", vec![leaf("INT_T", "int", 1, 1)]),
            leaf("ID", "x", 1, 5),
        ]);
        let mut type_tokens = HashMap::new();
        type_tokens.insert("INT_T".to_string(), Type::Int);
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: false,
                type_child: Some(ChildLocator::Index(0)),
                init_child: None,
                immutable: false,
            }],
            vec![],
            type_tokens,
        );

        let result = analyze(&tree, &spec);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let sym = result.table.lookup("x").expect("x se declaró");
        assert_eq!(sym.ty, Some(Type::Int), "el tipo debe quedar poblado desde el nodo `tipo`");
        assert!(sym.mutable);
    }

    #[test]
    fn type_child_by_symbol_finds_type_node_regardless_of_position() {
        // var_decl: ID | ID COLON tipo -- "tipo" está ausente en la primera
        // alternativa y en el índice 2 en la segunda; BySymbol debe manejar
        // ambas sin índice fijo.
        let mut type_tokens = HashMap::new();
        type_tokens.insert("STR_T".to_string(), Type::Str);
        let rule = DeclarationRule {
            production: "var_decl".to_string(),
            kind: SymbolKind::Variable,
            name_child: None,
            implicit: false,
            type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
            init_child: None,
            immutable: false,
        };

        let sin_tipo = internal("var_decl", vec![leaf("ID", "a", 1, 1)]);
        let spec = spec_without_classes(vec![rule], vec![], type_tokens.clone());
        let result = analyze(&sin_tipo, &spec);
        assert!(result.errors.is_empty());
        assert_eq!(result.table.lookup("a").unwrap().ty, None, "sin nodo tipo, no debe poblarse");

        let rule2 = DeclarationRule {
            production: "var_decl".to_string(),
            kind: SymbolKind::Variable,
            name_child: None,
            implicit: false,
            type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
            init_child: None,
            immutable: false,
        };
        let con_tipo = internal("var_decl", vec![
            leaf("ID", "b", 2, 1),
            leaf("COLON", ":", 2, 2),
            internal("tipo", vec![leaf("STR_T", "string", 2, 4)]),
        ]);
        let spec2 = spec_without_classes(vec![rule2], vec![], type_tokens);
        let result2 = analyze(&con_tipo, &spec2);
        assert!(result2.errors.is_empty(), "{:?}", result2.errors);
        assert_eq!(result2.table.lookup("b").unwrap().ty, Some(Type::Str));
    }

    #[test]
    fn type_child_with_unmapped_terminal_falls_back_to_unknown() {
        // Mismo árbol, pero el spec no trae "INT_T" en type_tokens — no debe
        // ser un error, solo Type::Unknown ("todavía no lo sabemos").
        let tree = internal("var_decl", vec![
            internal("tipo", vec![leaf("INT_T", "int", 1, 1)]),
            leaf("ID", "x", 1, 5),
        ]);
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: false,
                type_child: Some(ChildLocator::Index(0)),
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

        let result = analyze(&tree, &spec);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.table.lookup("x").unwrap().ty, Some(Type::Unknown));
    }

    #[test]
    fn type_child_as_bare_class_name_resolves_to_named_and_is_not_revisited_as_use() {
        // var_decl: tipo ID  ->  el "tipo" acá es directamente un ID suelto
        // (nombre de clase usado como tipo, p.ej. `MyClass obj;`) -- antes
        // era un límite conocido (doble visita -> Undeclared falso); ahora
        // Flow::SkipChildIndices lo excluye correctamente.
        let tree = internal("var_decl", vec![
            leaf("ID", "MyClass", 1, 1),
            leaf("ID", "obj", 1, 9),
        ]);
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "var_decl".to_string(),
                kind: SymbolKind::Variable,
                name_child: Some(1),
                implicit: false,
                type_child: Some(ChildLocator::Index(0)),
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

        let result = analyze(&tree, &spec);
        assert_eq!(result.table.lookup("obj").unwrap().ty, Some(Type::Named("MyClass".to_string())));

        // "MyClass" no se declaró en ninguna parte de este árbol, así que la
        // validación diferida la reporta como clase desconocida (S007). Lo
        // que NO debe aparecer es un S002 "no declarada": eso significaría
        // que el nodo de tipo se volvió a visitar como un uso normal, que es
        // justo lo que `Flow::SkipChildIndices` evita.
        let codes: Vec<&str> = result.errors.iter().map(|d| d.code.as_str()).collect();
        assert_eq!(codes, vec!["S007"], "{:?}", result.errors);
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
        let spec = spec_without_classes(
            vec![
                DeclarationRule {
                    production: "func_decl".to_string(),
                    kind: SymbolKind::Function,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
                DeclarationRule {
                    production: "var_decl".to_string(),
                    kind: SymbolKind::Variable,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
            ],
            vec![
                ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
                ScopeRule { production: "bloque".to_string(), kind: ScopeKind::Block, with_label: false },
            ],
            HashMap::new(),
        );

        let result = analyze(&func_decl, &spec);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        // "foo" quedó declarada afuera (el walk ya cerró todos los scopes que abrió).
        assert_eq!(result.table.depth(), 1);
        let foo = result.table.lookup("foo").unwrap();
        assert_eq!(foo.kind, SymbolKind::Function);
        // Signature aditiva: sin parámetros, sin tipo de retorno declarado -> Void.
        assert_eq!(foo.signature, Some(Signature { params: vec![], returns: Type::Void }));
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

        let spec = spec_without_classes(
            vec![
                DeclarationRule {
                    production: "param".to_string(),
                    kind: SymbolKind::Parameter,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
                DeclarationRule {
                    production: "stmt".to_string(),
                    kind: SymbolKind::Variable,
                    name_child: None,
                    implicit: true,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
            ],
            vec![],
            HashMap::new(),
        );

        let result = analyze(&seq, &spec);
        assert!(result.errors.is_empty(), "reasignar un parámetro no debe dar error: {:?}", result.errors);
        // Sigue siendo Parameter — la reasignación implícita no la reemplazó.
        assert_eq!(result.table.lookup("x").unwrap().kind, SymbolKind::Parameter);
    }

    #[test]
    fn implicit_declaration_creates_fresh_when_not_visible() {
        let stmt = internal("stmt", vec![leaf("ID", "y", 1, 1)]);
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
                type_child: None,
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

        let result = analyze(&stmt, &spec);
        assert!(result.errors.is_empty());
        assert_eq!(result.table.lookup("y").unwrap().kind, SymbolKind::Variable);
    }

    #[test]
    fn undeclared_identifier_leaf_produces_one_ambito_diagnostic() {
        let expr = internal("expr", vec![leaf("ID", "z", 3, 9)]);
        let spec = spec_without_classes(vec![], vec![], HashMap::new());

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
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "param_list".to_string(),
                kind: SymbolKind::Parameter,
                name_child: None,
                implicit: false,
                type_child: None,
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

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
        let spec = spec_without_classes(
            vec![DeclarationRule {
                production: "stmt".to_string(),
                kind: SymbolKind::Variable,
                name_child: None,
                implicit: true,
                type_child: None,
                init_child: None,
                immutable: false,
            }],
            vec![],
            HashMap::new(),
        );

        // "x" nunca se declaró en ningún lado — como stmt no disparó la
        // declaración (no hay ID directo), el ID dentro de expr se procesa
        // como uso normal y debe fallar por no-declarada.
        let result = analyze(&stmt, &spec);
        assert_eq!(result.errors.len(), 1);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.kind, ErrorKind::Ambito);
        assert!(diag.message.contains('x'));
    }

    // ===================== Fase 16: clases y objetos =====================

    /// Spec compartido por los tests de clases: una gramática de juguete con
    /// `class_decl: CLASS ID (COLON ID)? LBRACE class_members RBRACE`,
    /// `func_decl` como método, `param`/`var_decl` con tipo, `this`, acceso a
    /// miembros (`primary DOT ID`) y `new`.
    fn class_spec() -> SemanticSpec {
        let mut type_tokens = HashMap::new();
        type_tokens.insert("INT_T".to_string(), Type::Int);

        SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![
                DeclarationRule {
                    production: "class_decl".to_string(),
                    kind: SymbolKind::Class,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
                DeclarationRule {
                    production: "func_decl".to_string(),
                    kind: SymbolKind::Function,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
                DeclarationRule {
                    production: "param".to_string(),
                    kind: SymbolKind::Parameter,
                    name_child: None,
                    implicit: false,
                    type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
                    init_child: None,
                    immutable: false,
                },
                DeclarationRule {
                    production: "var_decl".to_string(),
                    kind: SymbolKind::Variable,
                    name_child: None,
                    implicit: false,
                    type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
                    init_child: None,
                    immutable: false,
                },
            ],
            scopes: vec![
                ScopeRule { production: "class_decl".to_string(), kind: ScopeKind::Class, with_label: true },
                ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
            ],
            type_tokens,
            this_token: Some("THIS".to_string()),
            member_access: vec![MemberAccessRule { production: "primary".to_string(), dot_token: "DOT".to_string() }],
            instantiation: None,
            call: Some(CallRule {
                production: "primary".to_string(),
                open_paren_token: "LPAREN".to_string(),
                callee_index: 0,
                arg_list_index: 2,
            }),
            args_list_symbol: Some("args".to_string()),
            constructor_name: None,
            assign: None,
            arith_tokens: Default::default(),
        }
    }

    fn tipo(token: &str, lexeme: &str) -> ParseNode {
        internal("tipo", vec![leaf(token, lexeme, 0, 0)])
    }

    /// Arma `arg_list -> args` con la forma recursiva izquierda que genera el
    /// parser (`args: args COMMA expr | expr`).
    fn arg_list(args: Vec<ParseNode>) -> ParseNode {
        let mut iter = args.into_iter();
        let first = match iter.next() {
            Some(a) => a,
            None => return internal("arg_list", vec![]),
        };
        let mut acc = internal("args", vec![first]);
        for a in iter {
            acc = internal("args", vec![acc, leaf("COMMA", ",", 0, 0), a]);
        }
        internal("arg_list", vec![acc])
    }

    #[test]
    fn this_inside_a_method_resolves_to_the_enclosing_class() {
        // class Contador { function leer() { return this; } }
        let this_leaf = leaf("THIS", "this", 3, 12);
        let ret = internal("return_stmt", vec![leaf("RETURN", "return", 3, 5), this_leaf]);
        let bloque = internal("bloque", vec![ret]);
        let metodo = internal("func_decl", vec![leaf("FUNCTION", "function", 2, 3), leaf("ID", "leer", 2, 12), bloque]);
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Contador", 1, 7), metodo]);

        let result = analyze(&clase, &class_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn this_outside_any_class_is_an_error() {
        let stray_this = leaf("THIS", "this", 1, 1);
        let result = analyze(&stray_this, &class_spec());
        assert_eq!(result.errors.len(), 1);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S009");
    }

    #[test]
    fn member_access_on_declared_variable_resolves_own_attribute() {
        // class Contador { var valor: int; }
        // función externa: c.valor  (c ya declarada de tipo Contador)
        let attr = internal("var_decl", vec![leaf("ID", "valor", 2, 7), tipo("INT_T", "int")]);
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Contador", 1, 7), attr]);

        let c_decl = internal("var_decl", vec![leaf("ID", "c", 4, 5), tipo("ID", "Contador")]);

        let member_id = leaf("ID", "valor", 5, 3);
        let base = internal("primary", vec![internal("atom", vec![leaf("ID", "c", 5, 1)])]);
        let access = internal("primary", vec![base, leaf("DOT", ".", 5, 2), member_id]);

        let programa = internal("programa", vec![clase, c_decl, access]);
        let result = analyze(&programa, &class_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn member_access_unknown_member_is_reported() {
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Contador", 1, 7)]);
        let c_decl = internal("var_decl", vec![leaf("ID", "c", 2, 5), tipo("ID", "Contador")]);

        let member_id = leaf("ID", "noExiste", 3, 3);
        let base = internal("primary", vec![internal("atom", vec![leaf("ID", "c", 3, 1)])]);
        let access = internal("primary", vec![base, leaf("DOT", ".", 3, 2), member_id]);

        let programa = internal("programa", vec![clase, c_decl, access]);
        let result = analyze(&programa, &class_spec());
        assert_eq!(result.errors.len(), 1);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S010");
    }

    #[test]
    fn member_access_walks_inheritance_chain() {
        // class Figura { var area: int; }
        // class Circulo : Figura { }
        // c: Circulo;  c.area  (heredado)
        let figura_attr = internal("var_decl", vec![leaf("ID", "area", 1, 20), tipo("INT_T", "int")]);
        let figura = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Figura", 1, 7), figura_attr]);
        let circulo = internal("class_decl", vec![
            leaf("CLASS", "class", 2, 1),
            leaf("ID", "Circulo", 2, 7),
            leaf("COLON", ":", 2, 15),
            leaf("ID", "Figura", 2, 17),
        ]);

        let c_decl = internal("var_decl", vec![leaf("ID", "c", 4, 5), tipo("ID", "Circulo")]);
        let member_id = leaf("ID", "area", 5, 3);
        let base = internal("primary", vec![internal("atom", vec![leaf("ID", "c", 5, 1)])]);
        let access = internal("primary", vec![base, leaf("DOT", ".", 5, 2), member_id]);

        let programa = internal("programa", vec![figura, circulo, c_decl, access]);
        let result = analyze(&programa, &class_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        // Circulo quedó con su padre seteado.
        assert_eq!(result.table.lookup("Circulo").unwrap().parent.as_deref(), Some("Figura"));
    }

    #[test]
    fn unknown_class_used_as_a_type_annotation_is_reported() {
        // var_decl: ID tipo   con tipo -> ID(NoExiste), una clase que nadie
        // declaró en ningún punto del archivo.
        let decl = internal("var_decl", vec![leaf("ID", "x", 1, 5), tipo("ID", "NoExiste")]);
        let result = analyze(&decl, &class_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S007");
        assert!(diag.message.contains("NoExiste"));
    }

    #[test]
    fn class_declared_after_its_use_as_a_type_is_not_reported() {
        // La gramática no exige declarar antes de usar: `x: Tardia` aparece
        // ANTES de `class Tardia`. La validación corre al terminar el walk,
        // justamente para que este caso no sea un falso positivo.
        let decl = internal("var_decl", vec![leaf("ID", "x", 1, 5), tipo("ID", "Tardia")]);
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 2, 1), leaf("ID", "Tardia", 2, 7)]);
        let programa = internal("programa", vec![decl, clase]);

        let result = analyze(&programa, &class_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn method_call_with_wrong_arity_is_reported() {
        // class Contador { function inc(n: int) {} }
        // c: Contador;  c.inc(1, 2)   -> espera 1, recibe 2
        let param = internal("param", vec![leaf("ID", "n", 2, 10), tipo("INT_T", "int")]);
        let metodo = internal("func_decl", vec![
            leaf("FUNCTION", "function", 2, 3),
            leaf("ID", "inc", 2, 12),
            param,
            internal("bloque", vec![]),
        ]);
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Contador", 1, 7), metodo]);
        let c_decl = internal("var_decl", vec![leaf("ID", "c", 4, 1), tipo("ID", "Contador")]);

        let callee = internal("primary", vec![
            internal("primary", vec![internal("atom", vec![leaf("ID", "c", 5, 1)])]),
            leaf("DOT", ".", 5, 2),
            leaf("ID", "inc", 5, 3),
        ]);
        let call = internal("primary", vec![
            callee,
            leaf("LPAREN", "(", 5, 6),
            arg_list(vec![leaf("INT_LIT", "1", 5, 7), leaf("INT_LIT", "2", 5, 9)]),
            leaf("RPAREN", ")", 5, 10),
        ]);

        let programa = internal("programa", vec![clase, c_decl, call]);
        let result = analyze(&programa, &class_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S013");
        assert!(diag.message.contains("Contador.inc"), "{}", diag.message);
    }

    #[test]
    fn method_call_with_correct_arity_and_types_is_clean() {
        let param = internal("param", vec![leaf("ID", "n", 2, 10), tipo("INT_T", "int")]);
        let metodo = internal("func_decl", vec![
            leaf("FUNCTION", "function", 2, 3),
            leaf("ID", "inc", 2, 12),
            param,
            internal("bloque", vec![]),
        ]);
        let clase = internal("class_decl", vec![leaf("CLASS", "class", 1, 1), leaf("ID", "Contador", 1, 7), metodo]);
        let c_decl = internal("var_decl", vec![leaf("ID", "c", 4, 1), tipo("ID", "Contador")]);

        let callee = internal("primary", vec![
            internal("primary", vec![internal("atom", vec![leaf("ID", "c", 5, 1)])]),
            leaf("DOT", ".", 5, 2),
            leaf("ID", "inc", 5, 3),
        ]);
        let call = internal("primary", vec![
            callee,
            leaf("LPAREN", "(", 5, 6),
            arg_list(vec![leaf("INT_LIT", "1", 5, 7)]),
            leaf("RPAREN", ")", 5, 8),
        ]);

        let programa = internal("programa", vec![clase, c_decl, call]);
        let result = analyze(&programa, &class_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn free_function_call_with_wrong_arity_is_reported() {
        // function suma(a: int) {}   suma(1, 2)
        let param = internal("param", vec![leaf("ID", "a", 1, 15), tipo("INT_T", "int")]);
        let func = internal("func_decl", vec![
            leaf("FUNCTION", "function", 1, 1),
            leaf("ID", "suma", 1, 10),
            param,
            internal("bloque", vec![]),
        ]);
        let call = internal("primary", vec![
            internal("primary", vec![internal("atom", vec![leaf("ID", "suma", 2, 1)])]),
            leaf("LPAREN", "(", 2, 5),
            arg_list(vec![leaf("INT_LIT", "1", 2, 6), leaf("INT_LIT", "2", 2, 8)]),
            leaf("RPAREN", ")", 2, 9),
        ]);

        let programa = internal("programa", vec![func, call]);
        let result = analyze(&programa, &class_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S013");
        assert!(diag.message.contains("suma"), "{}", diag.message);
    }
}
