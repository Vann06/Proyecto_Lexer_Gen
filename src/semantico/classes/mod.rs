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

use crate::semantico::functions::{self, ArgProblem};
use crate::semantico::operators;
use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::{SemanticError, Signature, Symbol, SymbolKind, SymbolTable};
use crate::semantico::types::{
    resolve_arithmetic, resolve_assignment, ArithmeticOperator, Type, TypeAnnotations,
};
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
/// `SymbolKind::Class` ni `SymbolKind::Struct`; `Err(UnknownMember)` si se
/// agota la cadena de
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
        .filter(|s| matches!(s.kind, SymbolKind::Class | SymbolKind::Struct))
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
pub fn resolve_expr_type(
    node: &ParseNode,
    table: &SymbolTable,
    spec: &SemanticSpec,
    rec: &mut TypeAnnotations,
) -> Option<Type> {
    let ty = resolve_expr_type_inner(node, table, spec, rec);
    // UN solo punto de registro para las ~12 salidas de `_inner`, y como la
    // recursion pasa por aca, cada subexpresion queda anotada sola. Es el
    // equivalente del atributo sintetizado del libro: el tipo del nodo se
    // guarda cuando termina de calcularse, con la tabla en el ambito correcto.
    if let Some(resolved) = &ty {
        rec.record(node, resolved);
    }
    ty
}

fn resolve_expr_type_inner(
    node: &ParseNode,
    table: &SymbolTable,
    spec: &SemanticSpec,
    rec: &mut TypeAnnotations,
) -> Option<Type> {
    if let Some((dot_idx, _)) = find_member_access(node, spec) {
        // Nota: aunque la producción de ASIGNACIÓN (`primary DOT ID ASSIGN
        // expr`) también matchea acá, no es una expresión con valor — nunca
        // se la pide a `resolve_expr_type` desde un contexto de expresión.
        // Devolver el tipo del miembro accedido es igualmente correcto.
        let base = node.children.first()?;
        let member_id = node.children.get(dot_idx + 1)?;
        let base_ty = resolve_expr_type(base, table, spec, rec)?;
        let class_name = match base_ty {
            Type::Named(name) => name,
            _ => return None,
        };
        let member_name = member_id.lexeme.as_deref().unwrap_or(&member_id.symbol);
        let member = resolve_member(table, &class_name, member_name, member_id.line, member_id.col).ok()?;
        return member.ty.clone();
    }

    // Literal de struct: su tipo es el struct que nombra. Va DESPUÉS del
    // acceso a miembro (para que `Punto{...}.x` siga resolviendo el campo) y
    // ANTES de las ramas de hoja/paso-a-través.
    if let Some((struct_name, _)) = find_struct_literal(node, spec) {
        return Some(Type::Named(struct_name));
    }

    // Literal de lista: su tipo es `Array(tipo común de sus elementos)`. El
    // diagnóstico de heterogeneidad lo emite `analyzer::enter` al visitar
    // este nodo directamente, no esta función (silenciosa por diseño, igual
    // que el resto de `classes`).
    if let Some(elements_node) = crate::semantico::collections::find_array_literal(node, spec) {
        let elements = crate::semantico::collections::flatten_array_elements(elements_node, spec);
        let (ty, _errors) =
            crate::semantico::collections::resolve_array_literal(&elements, table, spec, rec);
        return ty;
    }

    // Conjunto, tupla y mapa: mismo contrato silencioso que el literal de
    // lista — acá solo se TIPA; los diagnósticos los emite `analyzer`.
    if let Some(elements_node) = crate::semantico::collections::find_set_literal(node, spec) {
        let elements = crate::semantico::collections::flatten_array_elements(elements_node, spec);
        let (ty, _errors) =
            crate::semantico::collections::resolve_set_literal(&elements, table, spec, rec);
        return ty;
    }
    if let Some(elements_node) = crate::semantico::collections::find_tuple_literal(node, spec) {
        let elements = crate::semantico::collections::flatten_array_elements(elements_node, spec);
        return crate::semantico::collections::resolve_tuple_literal(&elements, table, spec, rec);
    }
    if let Some(entries_node) = crate::semantico::collections::find_map_literal(node, spec) {
        let entries = crate::semantico::collections::flatten_map_entries(entries_node, spec);
        let (ty, _errors) =
            crate::semantico::collections::resolve_map_literal(&entries, table, spec, rec);
        return ty;
    }

    // Acceso indexado: el tipo es el del ELEMENTO, no el del arreglo —
    // permite que `arr[0] + 1` y `matrix[0][1]` sigan tipando normalmente.
    // Silencioso ante una base que no es arreglo: el diagnóstico lo emite
    // `analyzer::enter`, no esta función.
    if let Some((base, index)) = crate::semantico::collections::find_index_access(node, spec) {
        let base_ty = resolve_expr_type(base, table, spec, rec);
        // El nodo del indice SI se pasa, aunque su tipo no: una tupla se tipa
        // por POSICION (`t[0]` y `t[1]` pueden ser distintos), asi que sin el
        // nodo no habria con que resolverla. El tipo del indice sigue en
        // `None` a proposito — aca solo se quiere el tipo resultante; los
        // diagnosticos los emite `analyzer`.
        return crate::semantico::collections::validate_index_access(base_ty.as_ref(), None, Some(index), 0, 0)
            .ok()
            .flatten();
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
        return resolve_expr_type(&node.children[0], table, spec, rec);
    }

    if let Some((op, left, right)) = find_arithmetic(node, spec) {
        let left_ty = resolve_expr_type(left, table, spec, rec)?;
        let right_ty = resolve_expr_type(right, table, spec, rec)?;
        // Operandos incompatibles: `None`, igual que cualquier otra forma que
        // no sabemos tipar. El diagnóstico lo emite `analyzer::enter` al
        // visitar este nodo, no esta función (que es silenciosa por diseño).
        return resolve_arithmetic(op, &left_ty, &right_ty).ok().map(|r| r.result);
    }

    // Lógicas, comparaciones y unarias — mismo contrato silencioso que la rama
    // aritmética: acá solo se TIPA la expresión; el diagnóstico lo emite
    // `analyzer::enter` al visitar el nodo.
    //
    // Que una comparación tipe como `bool` no es cosmético: antes de esto
    // `x > 4` caía en el `None` de abajo, así que una directiva `%condition`
    // sobre un `if`/`while` nunca veía el tipo real de su condición y se
    // rendía en silencio.
    if let Some((op, left, right)) = operators::find_logical(node, spec) {
        let left_ty = resolve_expr_type(left, table, spec, rec)?;
        let right_ty = resolve_expr_type(right, table, spec, rec)?;
        return operators::resolve_logical(op, &left_ty, &right_ty, node.line, node.col).ok();
    }

    if let Some((op, left, right)) = operators::find_comparison(node, spec) {
        let left_ty = resolve_expr_type(left, table, spec, rec)?;
        let right_ty = resolve_expr_type(right, table, spec, rec)?;
        return operators::resolve_comparison(op, &left_ty, &right_ty, node.line, node.col).ok();
    }

    if let Some((op, operand)) = operators::find_unary(node, spec) {
        let operand_ty = resolve_expr_type(operand, table, spec, rec)?;
        return operators::resolve_unary(op, &operand_ty, node.line, node.col).ok();
    }

    None
}

/// Si `node` tiene la forma `izquierda OP derecha` con `OP` entre los
/// operadores aritméticos configurados en `spec.arith_tokens`, devuelve la
/// operación y los dos operandos. Se reconoce por la FORMA (tres hijos, el
/// del medio es un token aritmético) en vez de por el nombre de la
/// producción, así una gramática no tiene que enumerar `additive_expr`,
/// `term`, etc. — le alcanza con declarar qué tokens son operadores.
pub fn find_arithmetic<'a>(
    node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Option<(ArithmeticOperator, &'a ParseNode, &'a ParseNode)> {
    if node.children.len() != 3 {
        return None;
    }
    let op = *spec.arith_tokens.get(&node.children[1].symbol)?;
    Some((op, &node.children[0], &node.children[2]))
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

/// La `Signature` del constructor de `class_name`, SUBIENDO por la cadena de
/// herencia: se busca `spec.constructor_name` con `SymbolKind::Function`
/// entre los `members` de la clase; si no está, se repite en su
/// `Symbol.parent`. Si se agota la cadena, o la gramática no tiene
/// `%constructor` configurado, o la clase todavía no cerró su scope
/// (`members` sigue `None`): el implícito `Signature{params: vec![],
/// returns: Type::Void}` — aridad 0.
///
/// El constructor PROPIO gana sobre el heredado, porque se miran los
/// `members` de cada clase antes de subir al padre.
///
/// Antes solo se miraba la clase misma, con el argumento de que "el
/// constructor no se hereda, evita resolver `super()`". Pero heredar la FIRMA
/// para comprobar aridad y tipos no obliga a implementar `super()`, y sin eso
/// `class Perro : Animal` sin constructor propio rechazaba
/// `new Perro("Toby")` — que es justo el ejemplo de la especificación de
/// Compiscript.
///
/// No se reusa `resolve_member` aunque también camine la herencia: su paso
/// intermedio consulta `SymbolTable::lookup_in_enclosing_class` cuando
/// `members` sigue `None`, algo pensado para `this.campo` DENTRO de la clase
/// que se está recorriendo. Con un `new Otra(...)` escrito dentro de un
/// método de `Animal`, y `Otra` declarada más abajo en el archivo, ese paso
/// buscaría "constructor" en el scope Class abierto —`Animal`— y le
/// atribuiría a `Otra` una firma ajena, aceptando en silencio una aridad
/// equivocada. Este recorrido solo mira `members` y `parent`.
pub fn constructor_signature(
    table: &SymbolTable,
    class_name: &str,
    spec: &SemanticSpec,
) -> Signature {
    let implicit = || Signature { params: Vec::new(), returns: Type::Void };

    let constructor_name = match &spec.constructor_name {
        Some(name) => name,
        None => return implicit(),
    };

    // Guarda de ciclos por nombre visitado, mismo criterio defensivo que
    // `resolve_member_rec`: la gramática actual no puede producir un ciclo de
    // herencia, pero es barato y evita un loop infinito si algún día se
    // permite más de un padre.
    let mut visited = HashSet::new();
    let mut current = class_name.to_string();

    while visited.insert(current.clone()) {
        let Some(class_symbol) = table.lookup(&current) else {
            break;
        };

        let propio = class_symbol
            .members
            .as_ref()
            .and_then(|members| {
                members
                    .iter()
                    .find(|m| m.name == *constructor_name && m.kind == SymbolKind::Function)
            })
            .and_then(|m| m.signature.clone());

        if let Some(signature) = propio {
            return signature;
        }

        match &class_symbol.parent {
            Some(padre) => current = padre.clone(),
            None => break,
        }
    }

    implicit()
}

/// Si `node` es la producción de literal de struct configurada en
/// `spec.struct_literal` Y esta instancia concreta trae el token de llave
/// entre sus hijos, devuelve `(nombre del struct, nodo de la lista de
/// campos)`. Una misma producción suele tener alternativas SIN llave
/// (`atom: ID`, `atom: NEW ID ...`) — para esas devuelve `None`.
///
/// Se reconoce por la FORMA, igual que `%new` y `%call`: solo la alternativa
/// del literal trae `open_brace_token` como hijo directo.
pub fn find_struct_literal<'a>(
    node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Option<(String, &'a ParseNode)> {
    let rule = spec.struct_literal.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.open_brace_token) {
        return None;
    }
    let name_node = node.children.get(rule.type_name_index)?;
    if name_node.symbol != spec.identifier_token {
        return None;
    }
    let fields = node.children.get(rule.field_list_index)?;
    let name = name_node.lexeme.as_deref().unwrap_or(&name_node.symbol).to_string();
    Some((name, fields))
}

/// Los pares `(hoja del nombre del campo, nodo de la expresión)` de un
/// literal, aplanando la lista recursiva. Reusa `flatten_arg_list`, que ya es
/// genérico en el símbolo de la lista, y saca de cada elemento el nombre y el
/// valor según `spec.field_init`.
///
/// Un elemento con otra forma se descarta en vez de reportar: mantiene la
/// disciplina silenciosa del resto del módulo ante formas inesperadas.
pub fn flatten_field_inits<'a>(
    field_list_node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Vec<(&'a ParseNode, &'a ParseNode)> {
    let (Some(field_list_symbol), Some(rule)) =
        (spec.field_list_symbol.as_deref(), spec.field_init.as_ref())
    else {
        return Vec::new();
    };
    flatten_arg_list(field_list_node, field_list_symbol)
        .into_iter()
        .filter_map(|init| {
            let name = init.children.get(rule.name_index)?;
            let value = init.children.get(rule.value_index)?;
            Some((name, value))
        })
        .collect()
}

/// Valida `Nombre { campo: valor, ... }` contra los campos declarados del
/// struct: campos inexistentes, repetidos, mal tipados y faltantes. Devuelve
/// TODOS los problemas, no se detiene en el primero.
///
/// Si el nombre no resuelve a un struct, devuelve un ÚNICO `UnknownClass` y
/// no dice nada de los campos: un diagnóstico por causa raíz, en vez de un
/// error por campo derivado del mismo problema.
pub fn validate_struct_literal(
    table: &SymbolTable,
    spec: &SemanticSpec,
    struct_name: &str,
    name_pos: (usize, usize),
    field_inits: &[(&ParseNode, &ParseNode)],
    rec: &mut TypeAnnotations,
) -> Vec<SemanticError> {
    let (line, col) = name_pos;

    let struct_symbol = match table.lookup(struct_name).filter(|s| s.kind == SymbolKind::Struct) {
        Some(s) => s,
        None => return vec![SemanticError::UnknownClass { name: struct_name.to_string(), line, col }],
    };

    let mut errors = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for (name_node, value_node) in field_inits {
        let field = name_node.lexeme.as_deref().unwrap_or(&name_node.symbol).to_string();

        if seen.contains(&field) {
            errors.push(SemanticError::DuplicateStructField {
                struct_name: struct_name.to_string(),
                field,
                line: name_node.line,
                col: name_node.col,
            });
            continue;
        }
        seen.push(field.clone());

        let declared = match resolve_member(table, struct_name, &field, name_node.line, name_node.col) {
            Ok(sym) => sym,
            Err(e) => {
                errors.push(e);
                continue;
            }
        };

        // Un valor que no sabemos tipar NO se chequea (`None`, nunca
        // `Some(Unknown)`): `resolve_assignment` trata `Unknown` como
        // incompatible con todo salvo consigo mismo.
        if let (Some(expected), Some(found)) =
            (declared.ty.clone(), resolve_expr_type(value_node, table, spec, rec))
        {
            if resolve_assignment(&expected, &found).is_err() {
                errors.push(SemanticError::StructFieldTypeMismatch {
                    struct_name: struct_name.to_string(),
                    field,
                    expected,
                    found,
                    line: value_node.line,
                    col: value_node.col,
                });
            }
        }
    }

    // Campos faltantes: solo comprobable con los miembros ya cerrados. Si el
    // struct todavía se está recorriendo (`members == None`) no se sabe cuál
    // es el conjunto completo, y suponerlo daría falsos positivos.
    if let Some(members) = &struct_symbol.members {
        for member in members {
            if !seen.contains(&member.name) {
                errors.push(SemanticError::MissingStructField {
                    struct_name: struct_name.to_string(),
                    field: member.name.clone(),
                    line,
                    col,
                });
            }
        }
    }

    errors
}

/// Tipa cada nodo de argumento con `resolve_expr_type`, dejando `None` en las
/// posiciones que esta fase no sabe resolver (una expresión compuesta, una
/// llamada anidada). Es el puente entre este módulo —que sí conoce
/// `ParseNode`— y `functions`, que solo razona sobre tipos.
///
/// Importante: una posición que no se puede tipar va como `None`, NUNCA como
/// `Some(Type::Unknown)`; `resolve_assignment` trata `Unknown` como
/// incompatible con todo salvo consigo mismo, así que confundir ambos casos
/// produciría errores de tipo inventados.
fn argument_types(
    arg_nodes: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
    rec: &mut TypeAnnotations,
) -> Vec<Option<Type>> {
    arg_nodes.iter().map(|n| resolve_expr_type(n, table, spec, rec)).collect()
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
    rec: &mut TypeAnnotations,
) -> Vec<SemanticError> {
    let (line, col) = class_name_pos;

    // Solo se comprueba que la clase EXISTA: la firma del constructor la
    // resuelve `constructor_signature` por su cuenta, porque puede estar en un
    // ancestro y no en esta clase.
    if table.lookup(class_name).filter(|s| s.kind == SymbolKind::Class).is_none() {
        return vec![SemanticError::UnknownClass { name: class_name.to_string(), line, col }];
    }

    let signature = constructor_signature(table, class_name, spec);
    let arg_types = argument_types(arg_nodes, table, spec, rec);

    functions::check_arguments(&signature, &arg_types)
        .into_iter()
        .map(|p| match p {
            ArgProblem::Arity { expected, found } => SemanticError::ConstructorArityMismatch {
                class_name: class_name.to_string(),
                expected,
                found,
                line,
                col,
            },
            // El índice viene 1-based; el argumento que lo produjo es el que
            // aporta la posición, para que el error señale al argumento y no
            // al nombre de la clase.
            ArgProblem::ArgType { index, expected, found } => SemanticError::ConstructorArgTypeMismatch {
                class_name: class_name.to_string(),
                index,
                expected,
                found,
                line: arg_nodes[index - 1].line,
                col: arg_nodes[index - 1].col,
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
    rec: &mut TypeAnnotations,
) -> Option<(&'a Symbol, String)> {
    if let Some((_, member_idx)) = find_member_access(callee_node, spec) {
        let base = callee_node.children.first()?;
        let member_id = callee_node.children.get(member_idx)?;
        let class_name = match resolve_expr_type(base, table, spec, rec)? {
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
    rec: &mut TypeAnnotations,
) -> Vec<SemanticError> {
    let (line, col) = call_pos;
    // Todo símbolo declarado como invocable recibe su firma en `enter`, ANTES
    // de recorrer su cuerpo — es lo que permite validar una llamada recursiva
    // (ver la "firma PROVISIONAL" en `analyzer`). Así que llegar acá sin firma
    // no significa "todavía no la sé": significa que lo invocado no es una
    // función. Una variable, un parámetro, una clase o un struct.
    let signature = match &callee.signature {
        Some(s) => s,
        None => {
            return vec![SemanticError::NotCallable {
                name: callee_label.to_string(),
                line,
                col,
            }]
        }
    };
    let arg_types = argument_types(arg_nodes, table, spec, rec);

    functions::check_arguments(signature, &arg_types)
        .into_iter()
        .map(|p| match p {
            ArgProblem::Arity { expected, found } => {
                SemanticError::CallArityMismatch { callee: callee_label.to_string(), expected, found, line, col }
            }
            // Igual que en el constructor: el índice es 1-based y la posición
            // sale del argumento que falló, no de la invocación entera.
            ArgProblem::ArgType { index, expected, found } => SemanticError::CallArgTypeMismatch {
                callee: callee_label.to_string(),
                index,
                expected,
                found,
                line: arg_nodes[index - 1].line,
                col: arg_nodes[index - 1].col,
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

    /// Un miembro `constructor` con la firma dada. Lo usan los tests de
    /// herencia de constructor y el de aridad.
    fn ctor(params: Vec<Type>) -> Symbol {
        Symbol {
            name: "constructor".to_string(),
            kind: SymbolKind::Function,
            line: 1,
            col: 1,
            ty: None,
            mutable: true,
            initialized: false,
            used: false,
            signature: Some(Signature { params, returns: Type::Void }),
            storage: None,
            members: None,
            parent: None,
        }
    }

    /// `SemanticSpec` mínimo con `%constructor` configurado — lo único que
    /// `constructor_signature` mira del spec.
    fn spec_con_constructor() -> SemanticSpec {
        SemanticSpec {
            identifier_token: "ID".to_string(),
            constructor_name: Some("constructor".to_string()),
            ..Default::default()
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
        let t = table_with_classes(vec![class_symbol("Vacia", None, vec![])]);
        let sig = constructor_signature(&t, "Vacia", &spec_con_constructor());
        assert!(sig.params.is_empty());
        assert_eq!(sig.returns, Type::Void);
    }

    #[test]
    fn constructor_signature_sube_por_la_cadena_de_herencia() {
        // Nieto -> Hijo -> Abuelo, y solo el ABUELO declara constructor.
        let t = table_with_classes(vec![
            class_symbol("Abuelo", None, vec![ctor(vec![Type::Str])]),
            class_symbol("Hijo", Some("Abuelo"), vec![]),
            class_symbol("Nieto", Some("Hijo"), vec![]),
        ]);

        let sig = constructor_signature(&t, "Nieto", &spec_con_constructor());
        assert_eq!(sig.params, vec![Type::Str], "debe heredar la firma del abuelo");
    }

    #[test]
    fn el_constructor_propio_gana_sobre_el_heredado() {
        let t = table_with_classes(vec![
            class_symbol("Padre", None, vec![ctor(vec![Type::Str, Type::Str])]),
            class_symbol("Hija", Some("Padre"), vec![ctor(vec![Type::Int])]),
        ]);

        let sig = constructor_signature(&t, "Hija", &spec_con_constructor());
        assert_eq!(sig.params, vec![Type::Int], "el propio tiene precedencia");
    }

    #[test]
    fn constructor_signature_sin_directiva_constructor_es_implicito() {
        let t = table_with_classes(vec![class_symbol("Algo", None, vec![ctor(vec![Type::Int])])]);
        let sin_directiva = SemanticSpec { identifier_token: "ID".to_string(), ..Default::default() };

        let sig = constructor_signature(&t, "Algo", &sin_directiva);
        assert!(sig.params.is_empty(), "sin %constructor no hay constructor que buscar");
    }

    #[test]
    fn una_cadena_de_herencia_ciclica_no_cuelga() {
        // La gramatica actual no puede producir esto; la guarda es defensiva.
        let t = table_with_classes(vec![
            class_symbol("A", Some("B"), vec![]),
            class_symbol("B", Some("A"), vec![]),
        ]);

        let sig = constructor_signature(&t, "A", &spec_con_constructor());
        assert!(sig.params.is_empty());
    }

    #[test]
    fn validate_instantiation_reports_arity_mismatch_without_type_noise() {
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![ctor(vec![Type::Int])])]);
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
            assign: None,
            arith_tokens: Default::default(),
            ..Default::default()
        };

        // Cero argumentos, se esperaba 1.
        let errs =
            validate_instantiation(&t, &spec, "Contador", (2, 3), &[], &mut TypeAnnotations::new());
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0],
            SemanticError::ConstructorArityMismatch { class_name: "Contador".to_string(), expected: 1, found: 0, line: 2, col: 3 }
        );
    }

    #[test]
    fn validate_instantiation_checks_simple_literal_argument_types() {
        let t = table_with_classes(vec![class_symbol("Contador", None, vec![ctor(vec![Type::Int])])]);
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
            assign: None,
            arith_tokens: Default::default(),
            ..Default::default()
        };

        let arg = leaf("STR_LIT", "\"texto\"");
        let errs = validate_instantiation(
            &t,
            &spec,
            "Contador",
            (2, 3),
            &[&arg],
            &mut TypeAnnotations::new(),
        );
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
        t.declare_typed("x", SymbolKind::Variable, Type::Int, true, true, Some(Type::Int), 1, 1).unwrap();
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
            assign: None,
            arith_tokens: Default::default(),
            ..Default::default()
        };
        // primary -> atom -> ID(x) : cadena de un solo hijo, "primary" es
        // TAMBIÉN el nombre de la producción de member_access, pero esta
        // instancia no tiene DOT -- debe caer al passthrough, no a None.
        let tree = ParseNode::internal(
            "primary".into(),
            vec![ParseNode::internal("atom".into(), vec![leaf("ID", "x")])],
        );
        assert_eq!(
            resolve_expr_type(&tree, &t, &spec, &mut TypeAnnotations::new()),
            Some(Type::Int)
        );
    }
}
