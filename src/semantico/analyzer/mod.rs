// Walker genérico (Fase 15/16), ahora como `impl Visitor for Analyzer` sobre
// el driver de `super::visitor` — este archivo NUNCA menciona el nombre de
// una producción concreta (ni "var_decl" ni "func_decl" ni "class_decl" ni
// nada por el estilo). Toda la especificidad de una gramática dada vive en
// el `SemanticSpec` que se le pasa a `analyze`, no acá — incluida la parte
// de clases/objetos (Fase 16: `this`, acceso a miembros con `.`,
// instanciación con `new`), que solo se activa si `spec` trae esa
// configuración (`this_token`/`member_access`/`instantiation`); si no,
// el walker se comporta exactamente igual que antes de que existiera.
// Funciones anidadas/closures (captura de variables del entorno de
// definición) tampoco necesitan configuración extra: se detectan solas
// mientras el walker ya recorre scopes de función anidados.
use crate::semantico::classes;
use crate::semantico::closures::ClosureCollector;
use crate::semantico::collections;
use crate::semantico::deadcode;
use crate::semantico::duplicates;
use crate::semantico::errors::ErrorCollector;
use crate::semantico::flow::{self, FlowContext};
use crate::semantico::functions::FunctionContext;
use crate::semantico::operators;
use crate::semantico::scopes::{ScopeCollector, ScopeKind};
use crate::semantico::spec::{SemanticSpec, ChildLocator};
use crate::semantico::symbols::{SemanticError, Signature, SymbolKind, SymbolTable};
use crate::semantico::types::{
    resolve_arithmetic, resolve_assignment, Type, TypeAnnotations,
};
use crate::semantico::visitor::{self, Flow, Visitor};
use crate::sintactico::runtime::parse_tree::ParseNode;
use std::collections::HashSet;

pub struct AnalysisResult {
    pub table: SymbolTable,
    pub errors: ErrorCollector,
    /// Funciones anidadas que capturan variables/parámetros de una función
    /// encerradora (no globales, no propios) — ver `closures::ClosureCollector`.
    pub closures: ClosureCollector,
    /// Una foto de cada ámbito en el momento en que se cerró, en orden de
    /// cierre. Es la ÚNICA forma de ver lo declarado dentro de un ámbito
    /// anónimo: `table` solo conserva el Global, y `Symbol::members` solo
    /// cubre los ámbitos con nombre. Ver `scopes::ScopeCollector`.
    pub scopes: ScopeCollector,
    /// El tipo inferido de cada nodo de expresion, indexado por la identidad
    /// del nodo dentro de `tree`. Es lo que una futura fase de generacion de
    /// codigo intermedio necesita para decidir, en cada operacion, que
    /// instruccion emitir y donde hace falta una ampliacion — ver
    /// `types::annotations`.
    pub types: TypeAnnotations,
}

/// Punto de entrada: recorre `tree` según `spec` y devuelve la tabla de
/// símbolos resultante junto con todos los diagnósticos encontrados (no se
/// detiene en el primero — sigue recorriendo para reportar todos, mismo
/// espíritu que el modo pánico del parser LR).
pub fn analyze(tree: &ParseNode, spec: &SemanticSpec) -> AnalysisResult {
    let mut analyzer = Analyzer::new(spec);
    visitor::walk(tree, &mut analyzer);
    analyzer.report_unknown_named_types();
    if spec.warn_unused {
        for warning in duplicates::unused_diagnostics(analyzer.table.current_scope_kind(), analyzer.table.current_symbols()) {
            analyzer.errors.push(warning);
        }
    }
    AnalysisResult {
        table: analyzer.table,
        errors: analyzer.errors,
        closures: analyzer.closures,
        types: analyzer.types,
        scopes: analyzer.scopes,
    }
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
    opened_loop: bool,
    /// `true` si este nodo abrió un contexto de `switch` — `exit` lo desapila
    /// junto con el tipo del discriminante, en lockstep con `opened_loop`.
    opened_switch: bool,
    /// `true` si el scope que este nodo abrió era `Function` — así `exit`
    /// sabe si debe desapilar `function_stack` además de cerrar el scope.
    opened_function: bool,
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
    closures: ClosureCollector,
    scopes: ScopeCollector,
    frames: Vec<Frame>,
    /// Pila de funciones activas: `(profundidad ABSOLUTA del scope propio de
    /// la función, su nombre)`. El tope es la función que se está recorriendo
    /// ahora mismo — cualquier uso que resuelva a un nombre en una profundidad
    /// MENOR (pero no Global) es una variable libre que esa función captura
    /// de su entorno de definición.
    function_stack: Vec<(usize, String)>,
    /// TODOS los nombres de tipo declarados por el usuario —clases y
    /// structs— en cualquier punto del archivo. A diferencia de la tabla de
    /// símbolos, este set nunca se vacía cuando un scope se cierra — ver
    /// `report_unknown_named_types`.
    declared_type_names: HashSet<String>,
    /// Cada `Type::Named` que apareció como anotación de tipo, con su
    /// posición, para validarlo al terminar el recorrido.
    pending_named_types: Vec<(String, usize, usize)>,
    /// `(clase, padre declarado, línea, columna)` de cada `class Hija : Padre`,
    /// para validar al final que el padre exista. Se difiere por el mismo
    /// motivo que `pending_named_types`: la clase padre puede declararse más
    /// abajo en el archivo que la hija.
    pending_parents: Vec<(String, String, usize, usize)>,
    /// Pila de funciones activas con su TIPO DE RETORNO declarado, para
    /// validar cada `return` contra la función que lo contiene.
    ///
    /// Va en lockstep con `function_stack` —se empuja y se saca en los mismos
    /// dos puntos— pero se mantiene aparte a propósito: `function_stack`
    /// guarda `(profundidad, nombre)` y solo le sirve a la detección de
    /// capturas, mientras que esto guarda `(nombre, tipo de retorno)` y solo
    /// le sirve a la validación de retornos. Fusionarlas ataría dos reglas
    /// que no tienen nada que ver.
    fn_context: FunctionContext,
    flow_context: FlowContext,
    /// Tipo del discriminante de cada `switch` activo, del más externo al más
    /// interno. Cada rama `case` se compara contra el tope. Es una pila y no
    /// un solo valor porque un `switch` puede anidarse dentro de otro.
    switch_types: Vec<Option<Type>>,
    /// Nodos de secuencia ya analizados en busca de código muerto. Una lista
    /// recursiva por la izquierda anida un nodo por sentencia, y todos ven un
    /// prefijo de la MISMA secuencia: sin esto, cada nivel volvería a
    /// reportar el mismo código inalcanzable.
    analyzed_sequences: HashSet<*const ParseNode>,
    /// Sentencias que el recorrido debe saltarse enteras por ser
    /// inalcanzables — el "corte de evaluación" tras una sentencia terminal.
    /// Se identifican por dirección porque el árbol no se modifica durante el
    /// recorrido, así que cada nodo tiene una identidad estable.
    unreachable: HashSet<*const ParseNode>,
    /// El tipo de cada nodo de expresion que se llego a tipar durante el
    /// recorrido — el "arbol de analisis anotado" del libro, guardado aparte
    /// del arbol. Se llena solo: `classes::resolve_expr_type` graba cada tipo
    /// que resuelve, asi que basta con pasarle este campo. Ver
    /// `types::annotations` para por que no vive dentro del `ParseNode`.
    types: TypeAnnotations,
}

impl<'a> Analyzer<'a> {
    fn new(spec: &'a SemanticSpec) -> Self {
        Analyzer {
            spec,
            table: SymbolTable::new(),
            errors: ErrorCollector::new(),
            closures: ClosureCollector::new(),
            scopes: ScopeCollector::new(),
            frames: Vec::new(),
            function_stack: Vec::new(),
            declared_type_names: HashSet::new(),
            pending_named_types: Vec::new(),
            pending_parents: Vec::new(),
            fn_context: FunctionContext::new(),
            flow_context: FlowContext::new(),
            switch_types: Vec::new(),
            analyzed_sequences: HashSet::new(),
            unreachable: HashSet::new(),
            types: TypeAnnotations::new(),
        }
    }

    /// Reporta las anotaciones de tipo que nombran una clase que nunca se
    /// declaró (`let x: NoExiste`). Corre DESPUÉS del recorrido completo, no
    /// durante: la gramática no exige declarar antes de usar, así que una
    /// clase puede aparecer más abajo en el archivo que su primer uso como
    /// tipo.
    ///
    /// Se contrasta contra `declared_type_names` (acumulado, nunca vaciado)
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
            if !self.declared_type_names.contains(name) {
                self.errors.push_semantic(&SemanticError::UnknownClass {
                    name: name.clone(),
                    line: *line,
                    col: *col,
                });
            }
        }

        // Misma pasada diferida, mensaje propio: heredar de algo inexistente
        // no es lo mismo que anotar un tipo inexistente, y el error nombra a
        // las DOS clases para que se vea de dónde salió.
        for (class, parent, line, col) in &self.pending_parents {
            if !self.declared_type_names.contains(parent) {
                self.errors.push_semantic(&SemanticError::UnknownParentClass {
                    class: class.clone(),
                    parent: parent.clone(),
                    line: *line,
                    col: *col,
                });
            }
        }
    }
}

impl<'a> Visitor for Analyzer<'a> {
    fn enter(&mut self, node: &ParseNode) -> Flow {
        // 0a. Corte de evaluación: esta sentencia quedó marcada como
        // inalcanzable, así que no se analiza NADA de ella. Sin este corte, el
        // código muerto seguiría generando diagnósticos de tipo y de ámbito
        // derivados sobre instrucciones que nunca se ejecutan.
        if self.unreachable.contains(&(node as *const ParseNode)) {
            self.frames.push(Frame::default());
            return Flow::SkipChildren;
        }

        // 0b. Código muerto: al llegar a una secuencia de sentencias se
        // aplana entera y se busca la primera terminal. Se analiza una sola
        // vez por secuencia — los nodos de la espina quedan marcados, porque
        // en una lista recursiva por la izquierda cada nivel ve un prefijo de
        // la misma secuencia y reportaría el mismo código muerto de nuevo.
        if self.spec.deadcode.sequences.iter().any(|production| production == &node.symbol)
            && !self.analyzed_sequences.contains(&(node as *const ParseNode))
        {
            let (spine, statements) = deadcode::flatten_sequence(node);
            self.analyzed_sequences.extend(spine);
            if let Some((diagnostic, dead)) = deadcode::unreachable_after_terminal(
                &statements,
                &self.spec.deadcode.terminals,
                &self.spec.deadcode.sequences,
            ) {
                self.errors.push(diagnostic);
                self.unreachable
                    .extend(dead.into_iter().map(|statement| statement as *const ParseNode));
            }
        }

        // 1. Una hoja de identificador solo llega hasta acá si su padre NO la
        // consumió con otro significado (nombre declarado, nombre de
        // miembro, nombre de clase en `new`...) — esos hijos se saltan con
        // `Flow::SkipChildIndices`, ver más abajo — así que es un uso real.
        if node.children.is_empty() && node.symbol == self.spec.identifier_token {
            let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
            match self.table.lookup_with_scope(name) {
                Some((sym, def_depth, def_kind)) => {
                    // Resolución de nombres libres: si hay una función activa
                    // (el tope de function_stack) y este nombre vive en una
                    // profundidad MENOR que la del scope propio de esa
                    // función (pero no en el Global, profundidad 0), es una
                    // variable/parámetro del entorno de definición — una
                    // captura, no un local. Las funciones y clases NO cuentan
                    // como captura: llamar a un vecino o a sí misma (recursión)
                    // es resolución de nombre normal, no cerrar sobre datos.
                    //
                    // Un scope de CLASE tampoco cuenta, aunque cumpla las dos
                    // condiciones de profundidad: un método que nombra un
                    // atributo de su propia clase está haciendo un acceso
                    // implícito a `this`, no cerrando sobre el entorno de
                    // definición de una función encerradora. Los scopes de
                    // bloque SÍ siguen contando — una variable de un bloque
                    // que encierra a la función es una captura legítima.
                    if let Some(&(boundary_depth, ref fn_name)) = self.function_stack.last() {
                        let is_capturable = matches!(sym.kind, SymbolKind::Variable | SymbolKind::Parameter);
                        if def_depth < boundary_depth
                            && def_depth > 0
                            && is_capturable
                            && def_kind != ScopeKind::Class
                            && def_kind != ScopeKind::Struct
                        {
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
            // Solo las hojas que representan lecturas llegan a esta rama.
            // Los destinos de asignación y los nombres declarados se omiten.
            if self.spec.warn_unused {
                if let Some(symbol) = self.table.lookup_mut(name) {
                    symbol.used = true;
                }
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
                let base_ty = classes::resolve_expr_type(base, &self.table, self.spec, &mut self.types);
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
                                if let Some(found) = classes::resolve_expr_type(value_node, &self.table, self.spec, &mut self.types) {
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
            let left_ty = classes::resolve_expr_type(left, &self.table, self.spec, &mut self.types);
            let right_ty = classes::resolve_expr_type(right, &self.table, self.spec, &mut self.types);
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

        // 2b-bis. Lógicas, comparaciones y unarias (`spec.logic_tokens` /
        // `compare_tokens` / `unary_tokens`). Como la rama aritmética, ninguna
        // hace early-return ni salta hijos: los operandos siguen recorriéndose
        // y, si alguno es a su vez una expresión inválida, también se reporta.
        // Un operando cuyo tipo no sabemos resolver no se chequea.
        if let Some((op, left, right)) = operators::find_logical(node, self.spec) {
            let left_ty = classes::resolve_expr_type(left, &self.table, self.spec, &mut self.types);
            let right_ty = classes::resolve_expr_type(right, &self.table, self.spec, &mut self.types);
            if let (Some(l), Some(r)) = (left_ty, right_ty) {
                let pos = &node.children[1];
                if let Err(e) = operators::resolve_logical(op, &l, &r, pos.line, pos.col) {
                    self.errors.push_operator(&e);
                }
            }
        }

        if let Some((op, left, right)) = operators::find_comparison(node, self.spec) {
            let left_ty = classes::resolve_expr_type(left, &self.table, self.spec, &mut self.types);
            let right_ty = classes::resolve_expr_type(right, &self.table, self.spec, &mut self.types);
            if let (Some(l), Some(r)) = (left_ty, right_ty) {
                let pos = &node.children[1];
                if let Err(e) = operators::resolve_comparison(op, &l, &r, pos.line, pos.col) {
                    self.errors.push_operator(&e);
                }
            }
        }

        if let Some((op, operand)) = operators::find_unary(node, self.spec) {
            if let Some(ty) = classes::resolve_expr_type(operand, &self.table, self.spec, &mut self.types) {
                let pos = &node.children[0];
                if let Err(e) = operators::resolve_unary(op, &ty, pos.line, pos.col) {
                    self.errors.push_operator(&e);
                }
            }
        }

        // 2b-ter. Sentido semántico de los operandos: una función, clase o
        // struct NOMBRADA A SECAS no es un valor (`f * 2`, `f && ok`). Se
        // aplica a las cuatro familias desde un solo lugar, incluida la
        // aritmética que ya existía — sin esto el caso pasa desapercibido,
        // porque el tipo de la hoja `f` es el tipo de RETORNO de `f`.
        let operand_nodes: Vec<&ParseNode> = classes::find_arithmetic(node, self.spec)
            .map(|(_, l, r)| vec![l, r])
            .or_else(|| operators::find_logical(node, self.spec).map(|(_, l, r)| vec![l, r]))
            .or_else(|| operators::find_comparison(node, self.spec).map(|(_, l, r)| vec![l, r]))
            .or_else(|| operators::find_unary(node, self.spec).map(|(_, o)| vec![o]))
            .unwrap_or_default();
        for operand in operand_nodes {
            if let Some(e) = operators::non_value_operand(operand, &self.table, self.spec) {
                self.errors.push_operator(&e);
            }
        }

        // 2b-quater. Literal de lista (`atom: LBRACKET arg_list RBRACKET`,
        // según `spec.array_literal`): valida que todos los elementos sean
        // de un tipo compatible entre sí. No hace early-return: los
        // elementos siguen recorriéndose para que sus identificadores se
        // validen como usos normales.
        if let Some(elements_node) = collections::find_array_literal(node, self.spec) {
            let elements = collections::flatten_array_elements(elements_node, self.spec);
            let (_, errs) = collections::resolve_array_literal(&elements, &self.table, self.spec, &mut self.types);
            for e in errs {
                self.errors.push_semantic(&e);
            }
        }

        // Conjunto, tupla y mapa: mismo trato que el literal de lista de
        // arriba. Cada uno se reconoce por su token marcador propio, así que
        // solo dispara el que corresponde.
        if let Some(elements_node) = collections::find_set_literal(node, self.spec) {
            let elements = collections::flatten_array_elements(elements_node, self.spec);
            let (_, errs) = collections::resolve_set_literal(&elements, &self.table, self.spec, &mut self.types);
            for e in errs {
                self.errors.push_semantic(&e);
            }
        }
        if let Some(entries_node) = collections::find_map_literal(node, self.spec) {
            let entries = collections::flatten_map_entries(entries_node, self.spec);
            let (_, errs) = collections::resolve_map_literal(&entries, &self.table, self.spec, &mut self.types);
            for e in errs {
                self.errors.push_semantic(&e);
            }
        }
        // La tupla no valida homogeneidad —es heterogénea por definición—, así
        // que no hay errores que empujar: su tipo lo resuelve
        // `classes::resolve_expr_type` cuando alguien se lo pide.

        // Acceso indexado (`primary: primary LBRACKET expr RBRACKET`, según
        // `spec.index_access`): la base debe ser un arreglo y el índice,
        // integer. Tampoco hace early-return: base e índice son expresiones
        // reales que siguen recorriéndose.
        if let Some((base, index)) = collections::find_index_access(node, self.spec) {
            let base_ty = classes::resolve_expr_type(base, &self.table, self.spec, &mut self.types);
            let index_ty = classes::resolve_expr_type(index, &self.table, self.spec, &mut self.types);
            if let Err(e) =
                collections::validate_index_access(base_ty.as_ref(), index_ty.as_ref(), Some(index), index.line, index.col)
            {
                self.errors.push_semantic(&e);
            }
        }

        // 2c. Retorno (`return_stmt: RETURN expr | RETURN`, según
        // `spec.returns`): valida el valor retornado contra el tipo declarado
        // de la función que lo contiene, que vive en el tope de `fn_context`.
        // Como la rama de aritmética, no hace early-return ni salta hijos: la
        // expresión de retorno tiene que seguir recorriéndose para que sus
        // identificadores se validen como usos normales.
        if let Some(rule) = self.spec.returns.iter().find(|r| r.production == node.symbol) {
            let value_node = find_child_index(node, &rule.value_child).map(|i| &node.children[i]);
            let value_ty = value_node.and_then(|n| classes::resolve_expr_type(n, &self.table, self.spec, &mut self.types));

            // Distinción crítica entre "no hay valor" y "hay valor pero no lo
            // sé tipar": las dos llegarían acá como `value_ty == None`, pero
            // solo la primera es un `return;` de verdad. Tratar la segunda
            // como retorno vacío produciría un `MissingReturnValue` falso en
            // cada `return f(x)` — `resolve_expr_type` no tipa llamadas.
            let checkable = value_node.is_none() || value_ty.is_some();
            if checkable {
                // La posición sale de la hoja del token de retorno, no del
                // nodo interno: una hoja siempre trae línea/columna reales.
                let pos_node = node.children.first().unwrap_or(node);
                if let Err(err) =
                    self.fn_context.validate_return(value_ty.as_ref(), pos_node.line, pos_node.col)
                {
                    self.errors.push_function(&err);
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
                    let value_type = classes::resolve_expr_type(value_node, &self.table, self.spec, &mut self.types);
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
                    if let Some((callee, label)) = classes::resolve_callee(callee_node, &self.table, self.spec, &mut self.types) {
                        let arg_nodes = classes::flatten_arg_list(arg_list_node, args_list_symbol);
                        let errs = classes::validate_call(
                            &self.table,
                            self.spec,
                            callee,
                            &label,
                            (callee_node.line, callee_node.col),
                            &arg_nodes,
                            &mut self.types,
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
                        &mut self.types,
                    ) {
                        self.errors.push_semantic(&e);
                    }
                    self.frames.push(Frame::default());
                    return Flow::SkipChildIndices(vec![rule.class_name_index]);
                }
            }
        }

        // 4b. Literal de struct (`atom: ID LBRACE field_inits RBRACE`, según
        // `spec.struct_literal`): valida campos inexistentes, repetidos, mal
        // tipados y faltantes contra los campos declarados del struct.
        if let Some((struct_name, field_list_node)) = classes::find_struct_literal(node, self.spec) {
            let rule = self.spec.struct_literal.as_ref().expect("find_struct_literal ya la consultó");
            let name_node = &node.children[rule.type_name_index];
            let field_inits = classes::flatten_field_inits(field_list_node, self.spec);

            for e in classes::validate_struct_literal(
                &self.table,
                self.spec,
                &struct_name,
                (name_node.line, name_node.col),
                &field_inits,
                &mut self.types,
            ) {
                self.errors.push_semantic(&e);
            }

            // Saltar el nombre del struct: ya se consumió como tipo, no es un
            // uso de variable. Las ETIQUETAS de campo viven dentro del
            // subárbol de la lista y no son hijos directos de este nodo, así
            // que no se pueden excluir por índice desde acá — de eso se
            // encarga la rama 4c, que corta cada `field_init` por separado.
            self.frames.push(Frame::default());
            return Flow::SkipChildIndices(vec![rule.type_name_index]);
        }

        // 4c. Un `field_init` de un literal de struct (`ID COLON expr`): su
        // primer hijo es la ETIQUETA del campo, no un uso de variable, así
        // que hay que excluirlo del recorrido o saldría un `S002` falso. El
        // valor sí se sigue recorriendo normalmente.
        if let Some(rule) = &self.spec.field_init {
            if node.symbol == rule.production && node.children.len() > rule.name_index {
                self.frames.push(Frame::default());
                return Flow::SkipChildIndices(vec![rule.name_index]);
            }
        }

        let position = node.children.first().unwrap_or(node);
        if self.spec.flow.breaks.iter().any(|production| production == &node.symbol) {
            if let Err(error) = self.flow_context.validate_break(position.line, position.col) {
                self.errors.push_flow(&error);
            }
        }
        if self.spec.flow.continues.iter().any(|production| production == &node.symbol) {
            if let Err(error) = self.flow_context.validate_continue(position.line, position.col) {
                self.errors.push_flow(&error);
            }
        }

        // Una rama `case` compara su valor contra el discriminante del switch
        // que la contiene — el tope de `switch_types`. Va ANTES de abrir
        // ningún contexto nuevo: el case pertenece al switch de afuera.
        for rule in self.spec.flow.cases.iter().filter(|r| r.production == node.symbol) {
            if let Some(value_index) = find_child_index(node, &rule.value_child) {
                let value = &node.children[value_index];
                let found = classes::resolve_expr_type(value, &self.table, self.spec, &mut self.types);
                let discriminant = self.switch_types.last().cloned().flatten();
                if let Err(error) =
                    flow::validate_case(discriminant.as_ref(), found.as_ref(), value.line, value.col)
                {
                    self.errors.push_flow(&error);
                }
            }
        }

        let opened_loop = self.spec.flow.loops.iter().any(|production| production == &node.symbol);
        if opened_loop {
            self.flow_context.enter_loop();
        }

        // El discriminante del switch NO se valida como booleano (se
        // selecciona sobre enteros o cadenas): se guarda su tipo para que cada
        // `case` se compare contra él. El contexto que abre admite `break`
        // pero no `continue` — ver `FlowContext::validate_break`.
        let opened_switch = self
            .spec
            .flow
            .switches
            .iter()
            .find(|rule| rule.production == node.symbol)
            .map(|rule| {
                let discriminant = find_child_index(node, &rule.discriminant_child)
                    .and_then(|i| classes::resolve_expr_type(&node.children[i], &self.table, self.spec, &mut self.types));
                self.switch_types.push(discriminant);
                self.flow_context.enter_switch();
            })
            .is_some();

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
                    let init_type = init_node.and_then(|n| classes::resolve_expr_type(n, &self.table, self.spec, &mut self.types));

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
                // Firma PROVISIONAL, antes de recorrer el cuerpo: sin ella
                // una llamada recursiva (`fact(...)` dentro de `fact`) no se
                // podría validar, porque la firma autoritativa se arma recién
                // en `exit`, cuando el cuerpo ya pasó. `exit` la sobrescribe
                // con la versión calculada desde los símbolos de parámetro
                // reales, así que esto es puramente aditivo.
                if matches!(rule.kind, SymbolKind::Function | SymbolKind::Other(_)) {
                    let params = collect_param_types(node, self.spec);
                    if let Some(sym) = self.table.lookup_mut(&name) {
                        let returns = sym.ty.clone().unwrap_or(Type::Void);
                        sym.signature = Some(Signature { params, returns });
                    }
                }

                declared_name = Some(name.clone());
                declared_kind = Some(rule.kind.clone());

                // Registro permanente para `report_unknown_named_types`:
                // sobrevive al cierre del scope donde se declaró. Vale para
                // TODO tipo nombrable por el usuario —clase o struct—, porque
                // cualquiera de los dos puede aparecer luego como anotación
                // de tipo y `Type::Named` no distingue entre ellos.
                if matches!(rule.kind, SymbolKind::Class | SymbolKind::Struct) {
                    self.declared_type_names.insert(name.clone());
                }

                // Herencia: si esta declaración es una clase, un SEGUNDO
                // identificador directo (aparte del propio) es el nombre de
                // la clase padre (`class Hijo : Padre { ... }`). Solo clases:
                // un struct no hereda, y como este escaneo es POSICIONAL
                // ("cualquier segundo ID hijo directo") dejarlo activo para
                // structs sería frágil ante cambios de gramática.
                if rule.kind == SymbolKind::Class {
                    if let Some((parent_idx, parent_node)) =
                        node.children.iter().enumerate().find(|(i, c)| *i != idx && c.symbol == self.spec.identifier_token)
                    {
                        skip_indices.push(parent_idx);
                        let parent = parent_node.lexeme.as_deref().unwrap_or(&parent_node.symbol).to_string();
                        // Anotar para validar al final que el padre exista;
                        // acá todavía no se puede, podría declararse abajo.
                        self.pending_parents.push((name.clone(), parent.clone(), parent_node.line, parent_node.col));
                        parent_name = Some(parent);
                    }
                }
            }
        }

        let mut opened_function = false;

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

            // Profundidad ABSOLUTA que ocupará el scope de esta función — la
            // pila tiene `depth()` elementos antes de empujar el nuevo, así
            // que ese es exactamente el índice donde va a quedar.
            let this_fn_depth = self.table.depth();

            match &label {
                Some(l) => self.table.enter_scope_named(rule.kind, l.clone(), node.line, node.col),
                None => self.table.enter_scope(rule.kind, node.line, node.col),
            }

            if rule.kind == ScopeKind::Function {
                opened_function = true;
                // El nombre que declaró ESTE mismo nodo (func_decl declara Y
                // abre scope a la vez); si no hay uno (una función sin nombre
                // no existe en esta gramática, pero no hay por qué asumirlo
                // en un walker agnóstico), usar un rótulo sintético con su
                // posición para que las capturas sigan siendo atribuibles.
                let fn_name =
                    declared_name.clone().or_else(|| label.clone()).unwrap_or_else(|| format!("<fn@{}:{}>", node.line, node.col));

                // Tipo de retorno declarado, para validar los `return` del
                // cuerpo que estamos por recorrer. Se lee de la tabla porque
                // el símbolo ya se declaró más arriba en este mismo `enter`,
                // en el scope EXTERIOR; el scope que se acaba de abrir está
                // vacío, así que `lookup` sube y lo encuentra igual.
                //
                // Sin anotación de tipo => `Void`: la función es un
                // procedimiento y retornar un valor es un error. Es la misma
                // convención que ya aplica `exit` al armar la `Signature`.
                //
                // No se puede esperar a la `Signature`: esa se arma recién en
                // `exit`, cuando el cuerpo —y sus `return`— ya pasaron.
                let returns = self
                    .table
                    .lookup(&fn_name)
                    .and_then(|s| s.ty.clone())
                    .unwrap_or(Type::Void);
                self.fn_context.enter_returning(fn_name.clone(), returns);
                self.flow_context.enter_function();

                self.function_stack.push((this_fn_depth, fn_name));
            }

            if let Some(class_name) = enclosing_class_for_this {
                self.table
                    .declare("this", SymbolKind::Variable, node.line, node.col)
                    .expect("el scope recién abierto está vacío, 'this' no puede estar ya declarado ahí");
                let this_sym = self.table.lookup_mut("this").expect("recién declarado");
                this_sym.ty = Some(Type::Named(class_name));
                this_sym.mutable = false;
                this_sym.initialized = true;
                // Símbolo sintético del compilador, no una declaración del usuario.
                if self.spec.warn_unused {
                    this_sym.used = true;
                }
            }

            true
        } else {
            false
        };

        // `foreach`: la variable de iteración se declara DENTRO del ámbito que
        // esta misma producción acaba de abrir — por eso va acá y no en el
        // paso de declaración genérico, que corre ANTES de abrir el scope (ahí
        // quedaría visible fuera del bucle). Su tipo tampoco sale de una
        // anotación sino del tipo de ELEMENTO del iterable, así que tampoco
        // puede expresarse con `%type_of`.
        for rule in self.spec.foreaches.iter().filter(|r| r.production == node.symbol) {
            let Some(var_index) = find_child_index(node, &rule.variable_child) else {
                continue;
            };
            let iterable = find_child_index(node, &rule.iterable_child).map(|i| &node.children[i]);
            let iterable_ty =
                iterable.and_then(|n| classes::resolve_expr_type(n, &self.table, self.spec, &mut self.types));

            // Un tipo que no se pudo resolver no es un error acá: la causa
            // real ya tiene su propio diagnóstico. Uno que SÍ se resolvió y no
            // es una colección sí lo es.
            let element = match &iterable_ty {
                None | Some(Type::Unknown) => Type::Unknown,
                Some(ty) => match collections::element_type(ty) {
                    Some(inner) => inner,
                    None => {
                        let position = iterable.unwrap_or(node);
                        self.errors.push_semantic(&SemanticError::NotIterable {
                            found: ty.clone(),
                            line: position.line,
                            col: position.col,
                        });
                        Type::Unknown
                    }
                },
            };

            let var_node = &node.children[var_index];
            let name = var_node.lexeme.as_deref().unwrap_or(&var_node.symbol).to_string();
            // El bucle le asigna un valor en cada vuelta: cuenta como
            // inicializada, igual que un parámetro.
            if let Err(error) = self.table.declare_typed(
                &name,
                SymbolKind::Variable,
                element.clone(),
                true,
                true,
                Some(element),
                var_node.line,
                var_node.col,
            ) {
                self.errors.push_semantic(&error);
            }
            // Ya se consumió como declaración: sin esto se volvería a visitar
            // como un uso y se reportaría como no declarada. El iterable NO se
            // salta — ese sí es una lectura real que debe marcarse.
            skip_indices.push(var_index);
        }

        self.frames.push(Frame {
            entered_scope,
            declared_name,
            opened_loop,
            opened_switch,
            opened_function,
            declared_kind,
            param_order: Vec::new(),
        });

        if skip_indices.is_empty() {
            Flow::Continue
        } else {
            Flow::SkipChildIndices(skip_indices)
        }
    }

    fn exit(&mut self, node: &ParseNode) {
        // Condiciones de control de flujo, configuradas por el `.yalp`; no
        // dependen de nombres concretos de producciones ni palabras reservadas.
        //
        // Se valida en `exit` y no en `enter` porque la condición puede
        // depender de lo que declare un hermano a su IZQUIERDA dentro de la
        // MISMA producción: en `for (let i = 0; i < 3; ...)` la `i` de la
        // condición la declara el inicializador. En `enter` todavía no se
        // recorrió ningún hijo, así que `i` no existía y la condición se
        // tipaba como no resoluble — la regla callaba y el S025 se perdía.
        // Acá los hijos ya pasaron y el scope que abrió este nodo (si abrió
        // uno) sigue vivo: se cierra más abajo en esta misma función.
        for rule in self.spec.flow.conditions.iter().filter(|r| r.production == node.symbol) {
            if let Some(condition_index) = find_child_index(node, &rule.condition_child) {
                let condition = &node.children[condition_index];
                let found = classes::resolve_expr_type(condition, &self.table, self.spec, &mut self.types);
                if let Err(error) = flow::validate_condition(found.as_ref(), condition.line, condition.col) {
                    self.errors.push_flow(&error);
                }
            }
        }

        let frame = self.frames.pop().expect("enter empujó un frame para cada nodo visitado");
        if frame.opened_function {
            self.function_stack.pop();
            // Lockstep con el push de `enter`: las dos pilas de función se
            // mantienen alineadas subiendo y bajando en los mismos puntos.
            self.fn_context.exit();
            // El desapilado va en un `let` y NO adentro del `debug_assert!`:
            // esa macro expande a `if cfg!(debug_assertions) { assert!(..) }`,
            // así que en release la expresión no se evalúa y el pop NUNCA
            // ocurría. La pila de `flow_context` crecía sin parar y
            // `has_loop_in_current_function` devolvía basura: S026/S027
            // dejaban de reportarse en el binario de release —el que compila
            // Dockerfile.api—, aunque toda la suite pasara en debug.
            let exited = self.flow_context.exit_function();
            debug_assert!(exited, "enter_function y exit_function deben ir en lockstep");
        }
        if frame.opened_loop {
            let exited = self.flow_context.exit_loop();
            debug_assert!(exited, "enter_loop y exit_loop deben ir en lockstep");
        }
        if frame.opened_switch {
            // Mismo cuidado que con `exit_function`: el desapilado va en un
            // `let` y NO dentro del `debug_assert!`, que en release no evalúa
            // su expresión — la pila crecería sin parar y `validate_break`
            // aceptaría `break` en cualquier parte del binario de release.
            let exited = self.flow_context.exit_switch();
            debug_assert!(exited, "enter_switch y exit_switch deben ir en lockstep");
            self.switch_types.pop();
        }
        if frame.entered_scope {
            // El scope que se cierra es el que este mismo `enter` acaba de
            // abrir arriba — nunca puede ser el Global, así que esto no falla.
            let closed = self
                .table
                .exit_scope()
                .expect("el scope recién abierto por este nodo debe poder cerrarse");
            // Profundidad que OCUPABA: `depth()` cuenta los scopes de la pila,
            // así que DESPUÉS de desapilar coincide con el índice que tenía el
            // que se acaba de cerrar (0 = Global). Medirla antes daría uno de
            // más, porque el que se cierra todavía estaría contado.
            let closed_depth = self.table.depth();

            // Foto antes de descartarlo. Es lo único que deja rastro de un
            // ámbito ANÓNIMO: lo declarado en un bloque no sobrevive ni en la
            // tabla (que al terminar solo tiene el Global) ni en `members`
            // (que solo puebla los ámbitos con nombre).
            self.scopes.record(&closed, closed_depth);
            if self.spec.warn_unused {
                for warning in duplicates::unused_diagnostics(closed.kind(), closed.symbols()) {
                    self.errors.push(warning);
                }
            }

            // Límite conocido, no arreglado a propósito: si el cuerpo tiene un
            // scope anónimo anidado adentro (p.ej. un `bloque` que no declara
            // nada por sí mismo, solo abre Block), lo declarado ahí adentro NO
            // se aplana hacia arriba — un local declarado dos niveles adentro
            // de una función no aparece en `members` de la función, solo lo
            // que cuelga directo de su propio scope (sus parámetros).
            // Aplanarlo "hacia arriba a través de scopes anónimos" sigue sin
            // hacerse, y a propósito: reinsertar esos nombres en la tabla viva
            // filtraría su visibilidad más allá de su bloque y rompería el
            // lookup con scoping correcto, que ya está bien probado.
            //
            // Lo que SÍ existe ya es el mecanismo de acumulación aparte que
            // este comentario anticipaba: `scopes::ScopeCollector` guarda una
            // foto de cada ámbito al cerrarse (ver el `record` de más abajo),
            // así que lo declarado en un bloque deja rastro sin tocar la
            // tabla. `members` no cambia; quien quiera ver un ámbito anónimo
            // mira `AnalysisResult::scopes`.
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
    // Mapa (`tipo: MAPA LT tipo COMMA tipo GT`): dos hijos de tipo, ubicados
    // por índice. No alcanza con el marcador solo, como sí le basta al
    // arreglo, porque hay que distinguir la clave del valor.
    if let Some((marker, key_idx, value_idx)) = &spec.map_type {
        if node.children.iter().any(|c| &c.symbol == marker) {
            if let (Some(key), Some(value)) = (node.children.get(*key_idx), node.children.get(*value_idx)) {
                return Type::Map(
                    Box::new(resolve_declared_type(key, spec)),
                    Box::new(resolve_declared_type(value, spec)),
                );
            }
        }
    }
    // Conjunto (`tipo: CONJ LT tipo GT`): un solo hijo de tipo, por índice.
    if let Some((marker, elem_idx)) = &spec.set_type {
        if node.children.iter().any(|c| &c.symbol == marker) {
            if let Some(elem) = node.children.get(*elem_idx) {
                return Type::Set(Box::new(resolve_declared_type(elem, spec)));
            }
        }
    }
    // Tupla (`tipo: TUPLA LT lista_tipos GT`): aridad variable, así que sus
    // tipos se aplanan de la lista recursiva en vez de nombrarse por índice.
    if let Some((marker, list_idx, list_symbol)) = &spec.tuple_type {
        if node.children.iter().any(|c| &c.symbol == marker) {
            if let Some(list) = node.children.get(*list_idx) {
                let items: Vec<Type> = classes::flatten_arg_list(list, list_symbol)
                    .into_iter()
                    .map(|t| resolve_declared_type(t, spec))
                    .collect();
                if !items.is_empty() {
                    return Type::Tuple(items);
                }
            }
        }
    }
    // Tipo de arreglo (`tipo: tipo LBRACKET RBRACKET`, según
    // `spec.array_type_token`): el primer hijo es el tipo del elemento —
    // recursivo, así que `int[][]` se resuelve solo anidando dos veces.
    if let Some(marker) = &spec.array_type_token {
        if node.children.iter().any(|c| &c.symbol == marker) {
            if let Some(first) = node.children.first() {
                return Type::Array(Box::new(resolve_declared_type(first, spec)));
            }
        }
    }
    if let Some(first) = node.children.first() {
        return resolve_declared_type(first, spec);
    }
    spec.type_tokens.get(&node.symbol).cloned().unwrap_or(Type::Unknown)
}

/// Tipos de los parámetros que declara ESTA producción, recogidos ANTES de
/// recorrerla, para poder darle una firma provisional a la función y así
/// validar sus llamadas recursivas (ver el uso en `enter`).
///
/// Recorre los descendientes de `node` podando todo subárbol que abra un
/// scope propio: eso saca el cuerpo de la función (y con él los parámetros de
/// cualquier función anidada, que no son de esta). El podado se hace contra
/// `spec.scopes` y no contra un nombre de producción concreto — es la misma
/// regla genérica que usa el resto del walker, así sigue sirviendo para
/// cualquier gramática.
///
/// Es solo provisional: los tipos salen de la anotación escrita en el árbol,
/// no de los símbolos ya declarados. `exit` la reemplaza por la versión
/// autoritativa, calculada desde los parámetros reales del scope cerrado.
fn collect_param_types(node: &ParseNode, spec: &SemanticSpec) -> Vec<Type> {
    fn walk(node: &ParseNode, spec: &SemanticSpec, out: &mut Vec<Type>) {
        for child in &node.children {
            if spec.scopes.iter().any(|r| r.production == child.symbol) {
                continue;
            }
            let is_param = spec
                .declarations
                .iter()
                .find(|r| r.production == child.symbol)
                .is_some_and(|r| r.kind == SymbolKind::Parameter);
            if is_param {
                let rule = spec.declarations.iter().find(|r| r.production == child.symbol).expect("recién encontrada");
                let ty = rule
                    .type_child
                    .as_ref()
                    .and_then(|locator| find_child_index(child, locator))
                    .map(|i| resolve_declared_type(&child.children[i], spec))
                    .unwrap_or(Type::Unknown);
                out.push(ty);
            }
            walk(child, spec, out);
        }
    }

    let mut out = Vec::new();
    walk(node, spec, &mut out);
    out
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
    use crate::semantico::spec::{
        AssignRule, CallRule, DeclarationRule, FieldInitRule, MemberAccessRule, ReturnRule,
        ScopeRule, StructLiteralRule,
    };
    use crate::semantico::symbols::SymbolKind;
    use crate::semantico::types::ArithmeticOperator;
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
            ..Default::default()
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
    fn an_unknown_typed_symbol_does_not_invent_diagnostics_downstream() {
        // `var_decl: tipo ID` donde el terminal del `tipo` (FOO_T) no está en
        // `type_tokens`: `resolve_declared_type` cae en `Type::Unknown`, que
        // significa "no lo sabemos", no "es incompatible". Ni la operación
        // aritmética ni la asignación posterior pueden reportar nada — antes
        // salían un S015 y un S006 falsos, porque la tabla de compatibilidad
        // solo aceptaba `Unknown` contra `Unknown`.
        let declaracion = internal(
            "var_decl",
            vec![internal("tipo", vec![leaf("FOO_T", "foo", 1, 1)]), leaf("ID", "x", 1, 5)],
        );
        let suma = internal(
            "suma",
            vec![leaf("ID", "x", 2, 1), leaf("PLUS", "+", 2, 3), leaf("INT_LIT", "1", 2, 5)],
        );
        let asignacion = internal(
            "assign_stmt",
            vec![leaf("ID", "x", 3, 1), leaf("ASSIGN", "=", 3, 3), leaf("INT_LIT", "2", 3, 5)],
        );
        let tree = internal("programa", vec![declaracion, suma, asignacion]);

        let mut type_tokens = HashMap::new();
        type_tokens.insert("INT_LIT".to_string(), Type::Int);

        let mut spec = spec_without_classes(
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
        spec.arith_tokens.insert("PLUS".to_string(), ArithmeticOperator::Add);
        spec.assign = Some(AssignRule {
            production: "assign_stmt".to_string(),
            target_index: 0,
            value_index: 2,
        });

        let result = analyze(&tree, &spec);
        assert!(
            result.errors.is_empty(),
            "un tipo sin resolver no debe generar diagnósticos: {:?}",
            result.errors
        );
        assert_eq!(result.table.lookup("x").unwrap().ty, Some(Type::Unknown));
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

    // ===================== Closures y tipado de declaraciones =====================

    /// Spec compartida por los tests de closures/tipado de abajo: func_decl/
    /// var_decl/param/class_decl declaran, func_decl/class_decl abren scope,
    /// y var_decl/param traen tipo vía un hijo "tipo".
    fn closures_and_types_spec() -> SemanticSpec {
        spec_without_classes(
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
                    type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
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
                    production: "class_decl".to_string(),
                    kind: SymbolKind::Class,
                    name_child: None,
                    implicit: false,
                    type_child: None,
                    init_child: None,
                    immutable: false,
                },
            ],
            vec![
                ScopeRule { production: "func_decl".to_string(), kind: ScopeKind::Function, with_label: true },
                ScopeRule { production: "class_decl".to_string(), kind: ScopeKind::Class, with_label: true },
            ],
            [("INT_T".to_string(), Type::Int), ("BOOL_T".to_string(), Type::Bool)].into_iter().collect(),
        )
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
    fn class_decl_fields_are_typed_and_queryable_as_members() {
        // Records/structs definidos por el usuario reutilizan class_decl: sus
        // campos son var_decl normales, tipados con la misma directiva
        // `type_child` que cualquier otra declaración — sin un concepto de
        // "record" nuevo en la gramática.
        let campo = internal("var_decl", vec![leaf("ID", "valor", 2, 3), tipo("INT_T", "integer")]);
        let contador = internal("class_decl", vec![
            leaf("CLASS", "class", 1, 1),
            leaf("ID", "Contador", 1, 7),
            campo,
        ]);

        let result = analyze(&contador, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let sym = result.table.lookup("Contador").expect("Contador se declaró");
        assert_eq!(sym.kind, SymbolKind::Class);

        // Su campo "valor" quedó tipado como Int, colgado como member.
        let members = sym.members.as_ref().expect("class_decl abrió un scope");
        let valor = members.iter().find(|m| m.name == "valor").expect("valor es campo de Contador");
        assert_eq!(valor.ty, Some(Type::Int));
    }

    // ===================== Fase 16: clases y objetos =====================
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
            ..Default::default()
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

    // ============== Retornos y capturas de clase (Semana 2) ==============

    /// Como `closures_and_types_spec`, pero con tipo de retorno declarado en
    /// `func_decl` y con la produccion de retorno configurada — el equivalente
    /// a `%type_of func_decl tipo` + `%return return_stmt expr`.
    fn returns_spec() -> SemanticSpec {
        let mut spec = closures_and_types_spec();
        for rule in spec.declarations.iter_mut() {
            if rule.production == "func_decl" {
                rule.type_child = Some(ChildLocator::BySymbol("tipo".to_string()));
            }
        }
        spec.type_tokens.insert("INT_LIT".to_string(), Type::Int);
        spec.type_tokens.insert("STR_LIT".to_string(), Type::Str);
        spec.returns = vec![ReturnRule {
            production: "return_stmt".to_string(),
            value_child: ChildLocator::BySymbol("expr".to_string()),
        }];
        spec
    }

    /// `func_decl: FUN ID [tipo] return_stmt`, con el `return_stmt` que se le
    /// pase. `tipo` ausente => la funcion es un procedimiento (retorno Void).
    fn function_returning(name: &str, tipo_node: Option<ParseNode>, return_stmt: ParseNode) -> ParseNode {
        let mut children = vec![leaf("FUN", "fun", 1, 1), leaf("ID", name, 1, 5)];
        children.extend(tipo_node);
        children.push(return_stmt);
        internal("func_decl", children)
    }

    fn return_with(value: Option<ParseNode>) -> ParseNode {
        let mut children = vec![leaf("RETURN", "return", 2, 3)];
        children.extend(value.map(|v| internal("expr", vec![v])));
        internal("return_stmt", children)
    }

    #[test]
    fn returning_the_wrong_type_is_reported_against_the_declared_one() {
        let f = function_returning(
            "f",
            Some(tipo("INT_T", "integer")),
            return_with(Some(leaf("STR_LIT", "\"texto\"", 2, 10))),
        );

        let result = analyze(&f, &returns_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        let diag = result.errors.iter().next().unwrap();
        assert_eq!(diag.code, "S016");
        // La posicion sale de la hoja RETURN, no del nodo interno (que en
        // este helper de test viene con 0:0).
        assert_eq!((diag.line, diag.col), (2, 3));
    }

    #[test]
    fn returning_the_declared_type_is_clean() {
        let f = function_returning(
            "f",
            Some(tipo("INT_T", "integer")),
            return_with(Some(leaf("INT_LIT", "1", 2, 10))),
        );
        let result = analyze(&f, &returns_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }

    #[test]
    fn an_empty_return_in_a_typed_function_is_reported() {
        let f = function_returning("f", Some(tipo("INT_T", "integer")), return_with(None));
        let result = analyze(&f, &returns_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert_eq!(result.errors.iter().next().unwrap().code, "S017");
    }

    #[test]
    fn returning_a_value_from_a_procedure_is_reported() {
        // Sin nodo `tipo`: la funcion no declara retorno, asi que es un
        // procedimiento y no puede devolver un valor.
        let f = function_returning("p", None, return_with(Some(leaf("INT_LIT", "5", 2, 10))));
        let result = analyze(&f, &returns_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert_eq!(result.errors.iter().next().unwrap().code, "S018");
    }

    #[test]
    fn an_untypable_return_expression_is_not_reported() {
        // La expresion tiene una forma que `resolve_expr_type` no sabe tipar
        // (dos hijos, sin operador aritmetico configurado). No hay que
        // confundirla con un `return;` — eso daria un S017 falso.
        let opaca = internal("expr", vec![leaf("INT_LIT", "1", 2, 10), leaf("INT_LIT", "2", 2, 12)]);
        let f = function_returning(
            "f",
            Some(tipo("INT_T", "integer")),
            internal("return_stmt", vec![leaf("RETURN", "return", 2, 3), opaca]),
        );

        let result = analyze(&f, &returns_spec());
        assert!(result.errors.is_empty(), "una expresion sin tipo resuelto no se chequea: {:?}", result.errors);
    }

    #[test]
    fn a_return_outside_any_function_is_reported() {
        let programa = internal("programa", vec![return_with(Some(leaf("INT_LIT", "1", 1, 8)))]);
        let result = analyze(&programa, &returns_spec());
        assert_eq!(result.errors.len(), 1, "{:?}", result.errors);
        assert_eq!(result.errors.iter().next().unwrap().code, "S019");
    }

    #[test]
    fn a_method_reading_its_own_class_attribute_is_not_a_capture() {
        // class C { var radio; function m() { <uso de radio> } }
        // `radio` vive en el scope de CLASE, que encierra al del metodo, pero
        // leerlo es un acceso implicito a `this`, no cerrar sobre el entorno
        // de definicion de una funcion. Regresion: antes se reportaba
        // "m captura: radio".
        let var_radio = internal("var_decl", vec![leaf("ID", "radio", 2, 7)]);
        let m = internal("func_decl", vec![
            leaf("FUN", "fun", 3, 3),
            leaf("ID", "m", 3, 12),
            leaf("ID", "radio", 4, 12),
        ]);
        let class_c = internal("class_decl", vec![leaf("ID", "C", 1, 7), var_radio, m]);

        let result = analyze(&class_c, &closures_and_types_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(
            result.closures.is_empty(),
            "un atributo de la clase encerradora no es una captura: {}",
            result.closures.dump()
        );
    }

    #[test]
    fn a_variable_from_an_enclosing_block_is_still_a_capture() {
        // El filtro de la prueba anterior es solo para scopes de CLASE: una
        // variable de un bloque que encierra a la funcion sigue capturandose.
        let var_x = internal("var_decl", vec![leaf("ID", "x", 2, 7)]);
        let inner = internal("func_decl", vec![
            leaf("FUN", "fun", 3, 3),
            leaf("ID", "inner", 3, 12),
            leaf("ID", "x", 4, 12),
        ]);
        let bloque = internal("bloque", vec![var_x, inner]);
        let outer = internal("func_decl", vec![
            leaf("FUN", "fun", 1, 1),
            leaf("ID", "outer", 1, 5),
            bloque,
        ]);

        let mut spec = closures_and_types_spec();
        spec.scopes.push(ScopeRule { production: "bloque".to_string(), kind: ScopeKind::Block, with_label: false });

        let result = analyze(&outer, &spec);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        let caps = result.closures.captures_of("inner").expect("inner captura x del bloque encerrador");
        assert_eq!(caps[0].name, "x");
    }

    // ===================== Structs / records =====================

    /// Spec con struct: `struct_decl: ID struct_field*`, campos
    /// `struct_field: ID tipo`, y literal `atom: ID LBRACE inits RBRACE`
    /// con `field_init: ID expr`.
    fn struct_spec() -> SemanticSpec {
        let mut spec = returns_spec();
        spec.declarations.push(DeclarationRule {
            production: "struct_decl".to_string(),
            kind: SymbolKind::Struct,
            name_child: None,
            implicit: false,
            type_child: None,
            init_child: None,
            immutable: false,
        });
        spec.declarations.push(DeclarationRule {
            production: "struct_field".to_string(),
            kind: SymbolKind::Variable,
            name_child: None,
            implicit: false,
            type_child: Some(ChildLocator::BySymbol("tipo".to_string())),
            init_child: None,
            immutable: false,
        });
        spec.scopes.push(ScopeRule {
            production: "struct_decl".to_string(),
            kind: ScopeKind::Struct,
            with_label: true,
        });
        spec.struct_literal = Some(StructLiteralRule {
            production: "atom".to_string(),
            open_brace_token: "LBRACE".to_string(),
            type_name_index: 0,
            field_list_index: 2,
        });
        spec.field_list_symbol = Some("inits".to_string());
        spec.field_init = Some(FieldInitRule {
            production: "field_init".to_string(),
            name_index: 0,
            value_index: 1,
        });
        spec
    }

    /// `struct Punto { x: integer; y: integer; }`
    fn punto_decl() -> ParseNode {
        internal("struct_decl", vec![
            leaf("ID", "Punto", 1, 8),
            internal("struct_field", vec![leaf("ID", "x", 1, 16), tipo("INT_T", "integer")]),
            internal("struct_field", vec![leaf("ID", "y", 1, 28), tipo("INT_T", "integer")]),
        ])
    }

    /// `Nombre { campo: valor, ... }`, con la lista recursiva izquierda que
    /// genera el parser (`inits: inits COMMA field_init | field_init`).
    fn struct_literal(name: &str, fields: Vec<(&str, ParseNode)>) -> ParseNode {
        let mut list: Option<ParseNode> = None;
        for (fname, value) in fields {
            let init = internal("field_init", vec![leaf("ID", fname, 2, 20), value]);
            list = Some(match list {
                None => internal("inits", vec![init]),
                Some(prev) => internal("inits", vec![prev, leaf("COMMA", ",", 2, 19), init]),
            });
        }
        // El hijo de indice 2 es el ENVOLTORIO de la lista
        // (`field_inits: inits | vacio`), igual que en la gramatica real;
        // `flatten_arg_list` espera bajar un nivel antes de recorrerla.
        let envoltorio = internal("field_inits", list.into_iter().collect());
        internal("atom", vec![
            leaf("ID", name, 2, 16),
            leaf("LBRACE", "{", 2, 22),
            envoltorio,
            leaf("RBRACE", "}", 2, 40),
        ])
    }

    fn codes_of(result: &AnalysisResult) -> Vec<&str> {
        result.errors.iter().map(|d| d.code.as_str()).collect()
    }

    #[test]
    fn struct_declaration_is_its_own_kind_with_typed_members() {
        let result = analyze(&punto_decl(), &struct_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);

        let punto = result.table.lookup("Punto").expect("Punto declarado");
        assert_eq!(punto.kind, SymbolKind::Struct, "un struct no debe registrarse como Class");
        let members = punto.members.as_ref().expect("los campos quedan como miembros");
        let mut names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["x", "y"]);
        assert!(members.iter().all(|m| m.ty == Some(Type::Int)), "campos tipados: {members:?}");
    }

    #[test]
    fn a_correct_struct_literal_is_clean_and_typed() {
        let lit = struct_literal("Punto", vec![
            ("x", leaf("INT_LIT", "1", 2, 24)),
            ("y", leaf("INT_LIT", "2", 2, 32)),
        ]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert!(
            result.errors.is_empty(),
            "ni el nombre del struct ni las etiquetas de campo cuentan como usos: {:?}",
            result.errors
        );
    }

    #[test]
    fn struct_literal_with_an_unknown_field_is_reported() {
        let lit = struct_literal("Punto", vec![
            ("z", leaf("INT_LIT", "1", 2, 24)),
            ("x", leaf("INT_LIT", "2", 2, 32)),
            ("y", leaf("INT_LIT", "3", 2, 38)),
        ]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert_eq!(codes_of(&result), vec!["S010"], "{:?}", result.errors);
    }

    #[test]
    fn struct_literal_with_a_wrongly_typed_field_is_reported() {
        let lit = struct_literal("Punto", vec![
            ("x", leaf("STR_LIT", "texto", 2, 24)),
            ("y", leaf("INT_LIT", "2", 2, 32)),
        ]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert_eq!(codes_of(&result), vec!["S022"], "{:?}", result.errors);
    }

    #[test]
    fn struct_literal_missing_a_field_is_reported() {
        let lit = struct_literal("Punto", vec![("x", leaf("INT_LIT", "1", 2, 24))]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert_eq!(codes_of(&result), vec!["S023"], "{:?}", result.errors);
        assert!(result.errors.iter().next().unwrap().message.contains('y'));
    }

    #[test]
    fn struct_literal_with_a_repeated_field_is_reported() {
        let lit = struct_literal("Punto", vec![
            ("x", leaf("INT_LIT", "1", 2, 24)),
            ("x", leaf("INT_LIT", "2", 2, 32)),
            ("y", leaf("INT_LIT", "3", 2, 38)),
        ]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert_eq!(codes_of(&result), vec!["S024"], "{:?}", result.errors);
    }

    #[test]
    fn a_field_value_that_cannot_be_typed_is_not_reported() {
        // Valor con una forma que `resolve_expr_type` no sabe tipar: cuenta
        // como campo presente, pero su tipo no se compara. No confundirlo con
        // `Some(Type::Unknown)`, que `resolve_assignment` rechazaria.
        let opaco = internal("expr", vec![leaf("INT_LIT", "1", 2, 24), leaf("INT_LIT", "2", 2, 26)]);
        let lit = struct_literal("Punto", vec![
            ("x", opaco),
            ("y", leaf("INT_LIT", "2", 2, 32)),
        ]);
        let programa = internal("programa", vec![punto_decl(), lit]);

        let result = analyze(&programa, &struct_spec());
        assert!(result.errors.is_empty(), "un valor sin tipo resuelto no se chequea: {:?}", result.errors);
    }

    #[test]
    fn an_undeclared_struct_in_a_literal_reports_only_the_root_cause() {
        let lit = struct_literal("NoExiste", vec![("x", leaf("INT_LIT", "1", 2, 24))]);
        let result = analyze(&lit, &struct_spec());
        assert_eq!(
            codes_of(&result),
            vec!["S007"],
            "un diagnostico por causa raiz, no uno por campo: {:?}",
            result.errors
        );
    }

    #[test]
    fn a_struct_used_as_a_type_annotation_is_not_an_unknown_class() {
        // `struct Punto {...}` + `var p: Punto` — el nombre del struct entra
        // al mismo registro de tipos declarados que las clases, o la pasada
        // diferida lo reportaria como clase inexistente (S007).
        let var_p = internal("var_decl", vec![
            leaf("ID", "p", 3, 5),
            internal("tipo", vec![leaf("ID", "Punto", 3, 8)]),
        ]);
        let programa = internal("programa", vec![punto_decl(), var_p]);

        let result = analyze(&programa, &struct_spec());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.table.lookup("p").unwrap().ty, Some(Type::Named("Punto".to_string())));
    }
}
