// Clases y objetos (Fase 16): resolución de miembros vía `.` (con herencia),
// tipo estático de una subexpresión simple, y validación de `new Clase(args)`
// contra el constructor de esa clase. A diferencia de `spec`/`analyzer`, este
// módulo SÍ razona sobre símbolos ya declarados (no es config declarativa) —
// por eso vive aparte: es la política semántica de "buscar un nombre DENTRO
// de otra cosa" en vez de "buscar un nombre subiendo la pila de scopes", que
// es todo lo que sabía hacer el walker hasta ahora.
//
// Agnóstico a la gramática igual que el resto de `semantico`: nada acá
// menciona "class_decl" ni "primary" — todo lo específico de una gramática
// concreta llega vía `SemanticSpec` (`member_access`, `instantiation`,
// `constructor_name`, `this_token`) o como parámetros (`class_name`).
use std::collections::HashSet;

use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::{SemanticError, Signature, Symbol, SymbolKind, SymbolTable};
use crate::semantico::types::{resolve_assignment, Type};
use crate::sintactico::gramatica::first::EPSILON;
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Busca `member_name` en la clase `class_name`. Orden de resolución:
/// 1. Si `members` de la clase ya está cerrado (`Some`), busca ahí.
/// 2. Si `members` sigue `None` — la clase todavía se está recorriendo, caso
///    de un acceso a un miembro de la PROPIA clase desde dentro de uno de
///    sus métodos (p.ej. `this.otroAtributo`) — busca con
///    `SymbolTable::lookup_in_enclosing_class`, restringido al scope Class
///    todavía abierto (nunca se cuela un nombre de otro lado: como Compiscript
///    no permite clases anidadas, a lo sumo un scope Class puede estar
///    abierto a la vez, así que "el Class más cercano" es siempre ESTA
///    clase mientras `members` sea `None`).
/// 3. Si no aparece en ninguno de los dos casos, sube por `Symbol.parent`
///    (herencia) — funciona incluso a mitad de la propia declaración, porque
///    `parent` se setea al declarar la clase, ANTES de recorrer su cuerpo,
///    sin depender de que `members` ya esté cerrado.
///
/// `Err(UnknownClass)` si `class_name` no resuelve a un símbolo
/// `SymbolKind::Class`; `Err(UnknownMember)` si se agota la cadena de
/// herencia. Guarda de ciclos con un set de nombres visitados (defensiva —
/// la gramática actual no puede producir un ciclo con una sola cadena
/// `CLASS ID COLON ID`, pero es barata y evita un loop infinito si algún día
/// se permite más de un padre por directiva).
pub fn resolve_member<'a>(
    table: &'a SymbolTable,
    class_name: &str,
    member_name: &str,
    line: usize,
    col: usize,
) -> Result<&'a Symbol, SemanticError> {
    let mut visited = HashSet::new();
    resolve_member_rec(table, class_name, class_name, member_name, line, col, &mut visited)
}

/// `original_class` es la clase por la que se PREGUNTÓ (la del objeto en
/// `obj.miembro`), mientras que `class_name` va cambiando al subir por la
/// herencia. El `UnknownMember` final nombra la original: decirle al usuario
/// que "no es miembro de Figura" cuando escribió `c.x` con `c: Circulo`
/// sería confuso — Figura es un detalle interno de la búsqueda.
#[allow(clippy::too_many_arguments)]
fn resolve_member_rec<'a>(
    table: &'a SymbolTable,
    original_class: &str,
    class_name: &str,
    member_name: &str,
    line: usize,
    col: usize,
    visited: &mut HashSet<String>,
) -> Result<&'a Symbol, SemanticError> {
    let unknown_member = || SemanticError::UnknownMember {
        class_name: original_class.to_string(),
        name: member_name.to_string(),
        line,
        col,
    };

    let class_symbol = table
        .lookup(class_name)
        .filter(|s| s.kind == SymbolKind::Class)
        .ok_or_else(|| SemanticError::UnknownClass { name: class_name.to_string(), line, col })?;

    let found_in_own = match &class_symbol.members {
        Some(members) => members.iter().find(|m| m.name == member_name),
        None => table.lookup_in_enclosing_class(member_name),
    };
    if let Some(found) = found_in_own {
        return Ok(found);
    }

    if !visited.insert(class_name.to_string()) {
        return Err(unknown_member());
    }

    match &class_symbol.parent {
        Some(parent) => resolve_member_rec(table, original_class, parent, member_name, line, col, visited),
        None => Err(unknown_member()),
    }
}

/// Si `node` es una de las producciones de acceso a miembro configuradas en
/// `spec.member_access` Y esta instancia concreta trae el token `.` entre sus
/// hijos, devuelve `(índice del punto, índice del nombre del miembro)`. Una
/// misma producción puede tener alternativas SIN punto (p.ej.
/// `primary: atom` o `assign_stmt: ID ASSIGN expr`) — para esas devuelve
/// `None`, y el llamador las trata como un nodo normal.
pub fn find_member_access(node: &ParseNode, spec: &SemanticSpec) -> Option<(usize, usize)> {
    let rule = spec.member_access.iter().find(|r| r.production == node.symbol)?;
    let dot_idx = node.children.iter().position(|c| c.symbol == rule.dot_token)?;
    let member_idx = dot_idx + 1;
    if member_idx < node.children.len() {
        Some((dot_idx, member_idx))
    } else {
        None
    }
}

/// Tipo estático de una subexpresión SIMPLE, para poder encadenar `obj.a.b`
/// y para el chequeo de argumentos de `new`. Desciende por la cadena de
/// producciones de un solo hijo que genera el parser (la jerarquía de
/// precedencia or_expr->...->primary->atom) y reconoce:
///  - un nodo con la forma de `spec.member_access` (p.ej. `primary DOT ID`):
///    resuelve el tipo de la base recursivamente y busca el miembro con
///    `resolve_member` — silencioso (`None`) si algo falla, el diagnóstico
///    real de un `.` inválido lo emite `analyzer::enter` al visitar ESE
///    nodo directamente, no esta función.
///  - una hoja == `identifier_token`: el `.ty` de lo que `lookup` encuentre.
///  - una hoja == `this_token`: el `.ty` de lo que `lookup("this")` encuentre.
///  - una hoja cuyo símbolo está en `spec.type_tokens` (un literal, p.ej.
///    INT_LIT/STR_LIT/TRUE/FALSE): el `Type` que ese token representa.
///  - cualquier otra forma (operador binario, llamada, `new`, paréntesis
///    con contenido compuesto): `None` — "no lo sabemos", no es error.
pub fn resolve_expr_type(node: &ParseNode, table: &SymbolTable, spec: &SemanticSpec) -> Option<Type> {
    if let Some((dot_idx, _)) = find_member_access(node, spec) {
        // Nota: aunque la producción de ASIGNACIÓN (`primary DOT ID ASSIGN
        // expr`) también matchea acá, no es una expresión con valor — nunca
        // se la pide a `resolve_expr_type` desde un contexto de expresión.
        // Devolver el tipo del miembro accedido es igualmente correcto.
        let base = node.children.first()?;
        let member_id = node.children.get(dot_idx + 1)?;
        let base_ty = resolve_expr_type(base, table, spec)?;
        let class_name = match base_ty {
            Type::Named(name) => name,
            _ => return None,
        };
        let member_name = member_id.lexeme.as_deref().unwrap_or(&member_id.symbol);
        let member = resolve_member(table, &class_name, member_name, member_id.line, member_id.col).ok()?;
        return member.ty.clone();
    }

    if node.children.is_empty() {
        if node.symbol == spec.identifier_token {
            let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
            return table.lookup(name).and_then(|s| s.ty.clone());
        }
        if Some(&node.symbol) == spec.this_token.as_ref() {
            return table.lookup("this").and_then(|s| s.ty.clone());
        }
        return spec.type_tokens.get(&node.symbol).cloned();
    }

    if node.children.len() == 1 {
        return resolve_expr_type(&node.children[0], table, spec);
    }

    None
}

/// Aplana `arg_list: args | ε` / `args: args COMMA expr | expr` a los
/// subárboles `expr` en orden, usando solo `args_list_symbol` (el nombre de
/// la producción recursiva "args") — no necesita conocer el nombre "expr".
pub fn flatten_arg_list<'a>(arg_list_node: &'a ParseNode, args_list_symbol: &str) -> Vec<&'a ParseNode> {
    match arg_list_node.children.first() {
        Some(child) if child.symbol != EPSILON => flatten_args_rec(child, args_list_symbol),
        _ => Vec::new(),
    }
}

fn flatten_args_rec<'a>(node: &'a ParseNode, args_list_symbol: &str) -> Vec<&'a ParseNode> {
    if node.symbol == args_list_symbol {
        match node.children.len() {
            // args: args COMMA expr
            3 => {
                let mut result = flatten_args_rec(&node.children[0], args_list_symbol);
                result.push(&node.children[2]);
                result
            }
            // args: expr
            1 => vec![&node.children[0]],
            _ => vec![node],
        }
    } else {
        // Forma inesperada (no es el wrapper "args") — tratarlo como un
        // único argumento en vez de fallar silenciosamente sin reportar nada.
        vec![node]
    }
}

/// La `Signature` del constructor de `class_symbol`: busca entre sus
/// `members` propios (NO en los del padre — el constructor no se hereda,
/// evita tener que resolver `super()`) uno llamado `spec.constructor_name`
/// con `SymbolKind::Function`. Si no existe, o si la gramática no tiene
/// `%constructor` configurado, o si la clase todavía no cerró su scope
/// (`members` sigue `None`): el implícito `Signature{params: vec![],
/// returns: Type::Void}` — aridad 0.
pub fn constructor_signature(class_symbol: &Symbol, spec: &SemanticSpec) -> Signature {
    let implicit = || Signature { params: Vec::new(), returns: Type::Void };

    let constructor_name = match &spec.constructor_name {
        Some(name) => name,
        None => return implicit(),
    };

    class_symbol
        .members
        .as_ref()
        .and_then(|members| members.iter().find(|m| m.name == *constructor_name && m.kind == SymbolKind::Function))
        .and_then(|m| m.signature.clone())
        .unwrap_or_else(implicit)
}

/// Un desajuste entre los argumentos de una invocación y la firma que se
/// invoca, SIN comprometerse todavía con una variante de `SemanticError` —
/// la misma comprobación sirve para un constructor (`new Clase(...)`) y para
/// una llamada normal (`f(...)`, `obj.m(...)`), pero cada una reporta con su
/// propio mensaje. Cada llamador mapea estos casos a sus variantes.
enum ArgProblem {
    Arity { expected: usize, found: usize },
    ArgType { index: usize, expected: Type, found: Type, line: usize, col: usize },
}

/// Comprueba `arg_nodes` contra `signature`: aridad exacta, y tipo de cada
/// argumento "simple" (el que `resolve_expr_type` sabe resolver: un literal,
/// una variable ya tipada, un acceso a miembro) contra el parámetro
/// correspondiente, vía `resolve_assignment` — la misma tabla de coerciones
/// que ya usa `SymbolTable::assign`. Un argumento compuesto (`None`) solo
/// cuenta para la aridad.
///
/// Si la aridad ya está mal devuelve SOLO ese problema, sin comparar tipos
/// posicionalmente: los pares parámetro/argumento ya están desalineados y
/// cualquier diferencia de tipo sería ruido derivado del error real.
fn check_args(
    table: &SymbolTable,
    spec: &SemanticSpec,
    signature: &Signature,
    arg_nodes: &[&ParseNode],
) -> Vec<ArgProblem> {
    if signature.params.len() != arg_nodes.len() {
        return vec![ArgProblem::Arity { expected: signature.params.len(), found: arg_nodes.len() }];
    }

    let mut problems = Vec::new();
    for (i, (param_ty, arg_node)) in signature.params.iter().zip(arg_nodes.iter()).enumerate() {
        if let Some(found_ty) = resolve_expr_type(arg_node, table, spec) {
            if resolve_assignment(param_ty, &found_ty).is_err() {
                problems.push(ArgProblem::ArgType {
                    index: i + 1,
                    expected: param_ty.clone(),
                    found: found_ty,
                    line: arg_node.line,
                    col: arg_node.col,
                });
            }
        }
    }
    problems
}

/// Valida `new class_name(args)`: existencia de la clase, más aridad y tipos
/// contra `constructor_signature` (ver `check_args`). Devuelve TODOS los
/// errores encontrados, no se detiene en el primero.
pub fn validate_instantiation(
    table: &SymbolTable,
    spec: &SemanticSpec,
    class_name: &str,
    class_name_pos: (usize, usize),
    arg_nodes: &[&ParseNode],
) -> Vec<SemanticError> {
    let (line, col) = class_name_pos;

    let class_symbol = match table.lookup(class_name).filter(|s| s.kind == SymbolKind::Class) {
        Some(s) => s,
        None => return vec![SemanticError::UnknownClass { name: class_name.to_string(), line, col }],
    };

    let signature = constructor_signature(class_symbol, spec);

    check_args(table, spec, &signature, arg_nodes)
        .into_iter()
        .map(|p| match p {
            ArgProblem::Arity { expected, found } => SemanticError::ConstructorArityMismatch {
                class_name: class_name.to_string(),
                expected,
                found,
                line,
                col,
            },
            ArgProblem::ArgType { index, expected, found, line, col } => SemanticError::ConstructorArgTypeMismatch {
                class_name: class_name.to_string(),
                index,
                expected,
                found,
                line,
                col,
            },
        })
        .collect()
}

/// Resuelve QUÉ se está invocando en `callee_node` (la subexpresión a la
/// izquierda del paréntesis de apertura), devolviendo el símbolo invocado y
/// una etiqueta legible para los mensajes de error. Dos formas:
///  - un acceso a miembro (`obj.metodo`): resuelve el tipo de la base y busca
///    el miembro en esa clase (subiendo la herencia) — etiqueta `Clase.metodo`.
///  - un identificador suelto (`f`), posiblemente envuelto en la cadena de
///    producciones de un solo hijo que genera el parser: `lookup` normal —
///    etiqueta el nombre.
///
/// `None` para cualquier otra forma (invocar el resultado de otra llamada,
/// un paréntesis, etc.): no sabemos qué se invoca, y no saberlo NO es un
/// error — simplemente no se valida.
pub fn resolve_callee<'a>(
    callee_node: &ParseNode,
    table: &'a SymbolTable,
    spec: &SemanticSpec,
) -> Option<(&'a Symbol, String)> {
    if let Some((_, member_idx)) = find_member_access(callee_node, spec) {
        let base = callee_node.children.first()?;
        let member_id = callee_node.children.get(member_idx)?;
        let class_name = match resolve_expr_type(base, table, spec)? {
            Type::Named(name) => name,
            _ => return None,
        };
        let member_name = member_id.lexeme.as_deref().unwrap_or(&member_id.symbol);
        let member = resolve_member(table, &class_name, member_name, member_id.line, member_id.col).ok()?;
        return Some((member, format!("{class_name}.{member_name}")));
    }

    // Bajar por la cadena de un solo hijo hasta la hoja del identificador.
    let mut node = callee_node;
    while node.children.len() == 1 {
        node = &node.children[0];
    }
    if node.children.is_empty() && node.symbol == spec.identifier_token {
        let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
        let symbol = table.lookup(name)?;
        return Some((symbol, name.to_string()));
    }
    None
}

/// Valida los argumentos de una invocación (`f(args)`, `obj.metodo(args)`)
/// contra la firma del símbolo invocado. Si el símbolo no tiene `signature`
/// —no es invocable, o es un método de una clase que todavía se está
/// recorriendo y cuya firma aún no se construyó— no se valida nada.
pub fn validate_call(
    table: &SymbolTable,
    spec: &SemanticSpec,
    callee: &Symbol,
    callee_label: &str,
    call_pos: (usize, usize),
    arg_nodes: &[&ParseNode],
) -> Vec<SemanticError> {
    let signature = match &callee.signature {
        Some(s) => s,
        None => return Vec::new(),
    };
    let (line, col) = call_pos;

    check_args(table, spec, signature, arg_nodes)
        .into_iter()
        .map(|p| match p {
            ArgProblem::Arity { expected, found } => {
                SemanticError::CallArityMismatch { callee: callee_label.to_string(), expected, found, line, col }
            }
            ArgProblem::ArgType { index, expected, found, line, col } => SemanticError::CallArgTypeMismatch {
                callee: callee_label.to_string(),
                index,
                expected,
                found,
                line,
                col,
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(symbol: &str, lexeme: &str) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(lexeme.to_string()), children: vec![], line: 1, col: 1 }
    }

    fn class_symbol(name: &str, parent: Option<&str>, members: Vec<Symbol>) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Class,
            line: 1,
            col: 1,
            ty: None,
            mutable: true,
            initialized: false,
            used: false,
            signature: None,
            storage: None,
            members: Some(members),
            parent: parent.map(str::to_string),
        }
    }

    fn attr(name: &str, ty: Type) -> Symbol {
        Symbol {
            name: name.to_string(),
            kind: SymbolKind::Variable,
            line: 1,
            col: 1,
            ty: Some(ty),
            mutable: true,
            initialized: true,
            used: false,
            signature: None,
            storage: None,
            members: None,
            parent: None,
        }
    }

    fn table_with_classes(classes: Vec<Symbol>) -> SymbolTable {
        let mut t = SymbolTable::new();
        for c in classes {
            t.declare(&c.name.clone(), c.kind.clone(), c.line, c.col).unwrap();
            let slot = t.lookup_mut(&c.name).unwrap();
            slot.members = c.members;
            slot.parent = c.parent;
        }
        t
    }

    #[test]
    fn resolve_member_finds_own_closed_member() {
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![attr("valor", Type::Int)])]);
        let found = resolve_member(&t, "Contador", "valor", 1, 1).expect("valor es miembro de Contador");
        assert_eq!(found.ty, Some(Type::Int));
    }

    #[test]
    fn resolve_member_unknown_class_errors() {
        let t = table_with_classes(vec![]);
        let err = resolve_member(&t, "NoExiste", "x", 3, 4).unwrap_err();
        assert_eq!(err, SemanticError::UnknownClass { name: "NoExiste".to_string(), line: 3, col: 4 });
    }

    #[test]
    fn resolve_member_missing_member_errors() {
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![attr("valor", Type::Int)])]);
        let err = resolve_member(&t, "Contador", "noExiste", 5, 6).unwrap_err();
        assert_eq!(
            err,
            SemanticError::UnknownMember { class_name: "Contador".to_string(), name: "noExiste".to_string(), line: 5, col: 6 }
        );
    }

    #[test]
    fn resolve_member_walks_up_the_inheritance_chain() {
        let t = table_with_classes(vec![
            class_symbol("Figura", None, vec![attr("area", Type::Int)]),
            class_symbol("Circulo", Some("Figura"), vec![attr("radio", Type::Int)]),
        ]);
        // "area" no es propio de Circulo -- debe subir a Figura y encontrarlo.
        let found = resolve_member(&t, "Circulo", "area", 1, 1).expect("area se hereda de Figura");
        assert_eq!(found.ty, Some(Type::Int));
        // "radio" sigue siendo propio.
        assert!(resolve_member(&t, "Circulo", "radio", 1, 1).is_ok());
    }

    #[test]
    fn unknown_member_names_the_class_that_was_asked_for_not_an_ancestor() {
        let t = table_with_classes(vec![
            class_symbol("Figura", None, vec![attr("area", Type::Int)]),
            class_symbol("Circulo", Some("Figura"), vec![attr("radio", Type::Int)]),
        ]);
        // La búsqueda sube hasta Figura, pero el usuario escribió `c.x` con
        // `c: Circulo` — el mensaje debe nombrar Circulo, no el ancestro
        // donde terminó la búsqueda.
        let err = resolve_member(&t, "Circulo", "x", 1, 1).unwrap_err();
        assert_eq!(
            err,
            SemanticError::UnknownMember {
                class_name: "Circulo".to_string(),
                name: "x".to_string(),
                line: 1,
                col: 1
            }
        );
    }

    #[test]
    fn flatten_arg_list_handles_empty_one_and_many_args() {
        let empty = ParseNode::internal("arg_list".into(), vec![ParseNode::epsilon_leaf()]);
        assert_eq!(flatten_arg_list(&empty, "args").len(), 0);

        let one = ParseNode::internal(
            "arg_list".into(),
            vec![ParseNode::internal("args".into(), vec![leaf("INT_LIT", "5")])],
        );
        assert_eq!(flatten_arg_list(&one, "args").len(), 1);

        let two = ParseNode::internal(
            "arg_list".into(),
            vec![ParseNode::internal(
                "args".into(),
                vec![
                    ParseNode::internal("args".into(), vec![leaf("INT_LIT", "1")]),
                    leaf("COMMA", ","),
                    leaf("INT_LIT", "2"),
                ],
            )],
        );
        let flat = flatten_arg_list(&two, "args");
        assert_eq!(flat.len(), 2);
        assert_eq!(flat[0].lexeme.as_deref(), Some("1"));
        assert_eq!(flat[1].lexeme.as_deref(), Some("2"));
    }

    #[test]
    fn constructor_signature_falls_back_to_implicit_when_no_constructor_member() {
        let class = class_symbol("Vacia", None, vec![]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
            type_tokens: Default::default(),
            this_token: None,
            member_access: Vec::new(),
            instantiation: None,
            call: None,
            args_list_symbol: None,
            constructor_name: Some("constructor".to_string()),
        };
        let sig = constructor_signature(&class, &spec);
        assert!(sig.params.is_empty());
        assert_eq!(sig.returns, Type::Void);
    }

    #[test]
    fn validate_instantiation_reports_arity_mismatch_without_type_noise() {
        let ctor = Symbol {
            name: "constructor".to_string(),
            kind: SymbolKind::Function,
            line: 1,
            col: 1,
            ty: None,
            mutable: true,
            initialized: false,
            used: false,
            signature: Some(Signature { params: vec![Type::Int], returns: Type::Void }),
            storage: None,
            members: None,
            parent: None,
        };
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![ctor])]);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
            type_tokens: Default::default(),
            this_token: None,
            member_access: Vec::new(),
            instantiation: None,
            call: None,
            args_list_symbol: None,
            constructor_name: Some("constructor".to_string()),
        };

        // Cero argumentos, se esperaba 1.
        let errs = validate_instantiation(&t, &spec, "Contador", (2, 3), &[]);
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            SemanticError::ConstructorArityMismatch { class_name: "Contador".to_string(), expected: 1, found: 0, line: 2, col: 3 }
        );
    }

    #[test]
    fn validate_instantiation_checks_simple_literal_argument_types() {
        let ctor = Symbol {
            name: "constructor".to_string(),
            kind: SymbolKind::Function,
            line: 1,
            col: 1,
            ty: None,
            mutable: true,
            initialized: false,
            used: false,
            signature: Some(Signature { params: vec![Type::Int], returns: Type::Void }),
            storage: None,
            members: None,
            parent: None,
        };
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![ctor])]);
        let mut type_tokens = std::collections::HashMap::new();
        type_tokens.insert("STR_LIT".to_string(), Type::Str);
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
            type_tokens,
            this_token: None,
            member_access: Vec::new(),
            instantiation: None,
            call: None,
            args_list_symbol: None,
            constructor_name: Some("constructor".to_string()),
        };

        let arg = leaf("STR_LIT", "\"texto\"");
        let errs = validate_instantiation(&t, &spec, "Contador", (2, 3), &[&arg]);
        assert_eq!(errs.len(), 1);
        match &errs[0] {
            SemanticError::ConstructorArgTypeMismatch { expected, found, index, .. } => {
                assert_eq!(*expected, Type::Int);
                assert_eq!(*found, Type::Str);
                assert_eq!(*index, 1);
            }
            other => panic!("se esperaba ConstructorArgTypeMismatch, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn resolve_expr_type_resolves_bare_identifier_through_passthrough_chain() {
        let mut t = SymbolTable::new();
        t.declare_typed("x", SymbolKind::Variable, Type::Int, true, Some(Type::Int), 1, 1).unwrap();
        let spec = SemanticSpec {
            identifier_token: "ID".to_string(),
            declarations: vec![],
            scopes: vec![],
            type_tokens: Default::default(),
            this_token: None,
            member_access: vec![crate::semantico::spec::MemberAccessRule {
                production: "primary".to_string(),
                dot_token: "DOT".to_string(),
            }],
            instantiation: None,
            call: None,
            args_list_symbol: None,
            constructor_name: None,
        };
        // primary -> atom -> ID(x) : cadena de un solo hijo, "primary" es
        // TAMBIÉN el nombre de la producción de member_access, pero esta
        // instancia no tiene DOT -- debe caer al passthrough, no a None.
        let tree = ParseNode::internal(
            "primary".into(),
            vec![ParseNode::internal("atom".into(), vec![leaf("ID", "x")])],
        );
        assert_eq!(resolve_expr_type(&tree, &t, &spec), Some(Type::Int));
    }
}
