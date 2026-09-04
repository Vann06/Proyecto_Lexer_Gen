// Listas y arreglos (Proyecto 2): tipo homogéneo de los elementos de un
// literal de lista y validación de índices en un acceso `arr[i]`. Multi-
// dimensional (`int[][]`) sale gratis de `Type::Array(Box<Type>)` anidado —
// indexar un `Array(Array(Int))` devuelve `Array(Int)`, e indexar de nuevo
// devuelve `Int`.
//
// Agnóstico a la gramática igual que `classes`: nada acá menciona "atom" ni
// "primary" — la forma concreta llega vía `SemanticSpec::array_literal`/
// `index_access`, reconocida por FORMA (mismo criterio que `%new`/`%call`/
// `%struct_literal`).
use crate::semantico::classes::{self, flatten_arg_list};
use crate::semantico::spec::SemanticSpec;
use crate::semantico::symbols::{SemanticError, SymbolTable};
use crate::semantico::types::{resolve_assignment, Type};
use crate::sintactico::runtime::parse_tree::ParseNode;

/// Si `node` es la producción de literal de lista configurada en
/// `spec.array_literal` Y esta instancia concreta trae el corchete de
/// apertura entre sus hijos, devuelve el nodo de la lista de elementos
/// (`arg_list`). Otras alternativas del mismo head (sin corchete) devuelven
/// `None`.
pub fn find_array_literal<'a>(node: &'a ParseNode, spec: &SemanticSpec) -> Option<&'a ParseNode> {
    let rule = spec.array_literal.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.open_bracket_token) {
        return None;
    }
    node.children.get(rule.elements_index)
}

/// Aplana la lista de elementos de un literal, reusando el mismo mecanismo
/// que `arg_list`/`args` de una llamada — un literal de lista es sintáctica-
/// mente la misma lista separada por comas.
pub fn flatten_array_elements<'a>(elements_node: &'a ParseNode, spec: &SemanticSpec) -> Vec<&'a ParseNode> {
    match spec.args_list_symbol.as_deref() {
        Some(symbol) => flatten_arg_list(elements_node, symbol),
        None => Vec::new(),
    }
}

/// Tipo del literal de lista (`Array(tipo común)`) y los errores de
/// homogeneidad encontrados. El tipo común se infiere del primer elemento
/// tipable y se ENSANCHA (`integer` -> `float`) si un elemento posterior lo
/// exige, con la misma tabla de coerciones que usa una asignación — así
/// `[1, 2.5]` tipa como `float[]` en vez de reportar un falso error. Un
/// elemento cuyo tipo no se puede resolver (expresión compuesta) se ignora,
/// igual que el resto del módulo `classes`: no saberlo no es un error.
///
/// Lista vacía: tipo `None` — no hay de dónde inferirlo. No es un error del
/// literal en sí; la incompatibilidad, si la hay, sale cuando se lo asigna
/// contra un tipo declarado.
pub fn resolve_array_literal(
    elements: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> (Option<Type>, Vec<SemanticError>) {
    let (common, errors) = common_type(elements, table, spec);
    (common.map(|t| Type::Array(Box::new(t))), errors)
}

/// Tipo común de una secuencia de elementos, ensanchando `integer -> float`
/// igual que hace un literal de arreglo, más los errores de heterogeneidad.
///
/// Es el núcleo compartido por el arreglo, el conjunto y —dos veces— el
/// mapa: sus claves y sus valores se comprueban con esta misma regla, cada
/// grupo por separado.
fn common_type(
    elements: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> (Option<Type>, Vec<SemanticError>) {
    let mut errors = Vec::new();
    let mut common: Option<Type> = None;

    for element in elements {
        let Some(found) = classes::resolve_expr_type(element, table, spec) else {
            continue;
        };
        match &common {
            None => common = Some(found),
            Some(expected) => {
                if resolve_assignment(expected, &found).is_ok() {
                    // Compatible tal cual (incluida la ampliación integer -> float).
                } else if resolve_assignment(&found, expected).is_ok() {
                    // El elemento nuevo es el tipo más ancho: ensanchar.
                    common = Some(found);
                } else {
                    errors.push(SemanticError::HeterogeneousArrayElements {
                        expected: expected.clone(),
                        found,
                        line: element.line,
                        col: element.col,
                    });
                }
            }
        }
    }

    (common, errors)
}

/// Igual que `find_array_literal`, para la producción de literal de conjunto.
pub fn find_set_literal<'a>(node: &'a ParseNode, spec: &SemanticSpec) -> Option<&'a ParseNode> {
    let rule = spec.set_literal.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.marker_token) {
        return None;
    }
    node.children.get(rule.elements_index)
}

/// Igual que `find_array_literal`, para la producción de literal de tupla.
pub fn find_tuple_literal<'a>(node: &'a ParseNode, spec: &SemanticSpec) -> Option<&'a ParseNode> {
    let rule = spec.tuple_literal.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.marker_token) {
        return None;
    }
    node.children.get(rule.elements_index)
}

/// Igual que `find_array_literal`, para la producción de literal de mapa.
pub fn find_map_literal<'a>(node: &'a ParseNode, spec: &SemanticSpec) -> Option<&'a ParseNode> {
    let rule = spec.map_literal.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.marker_token) {
        return None;
    }
    node.children.get(rule.entries_index)
}

/// Aplana la lista de entradas `clave: valor` de un literal de mapa, con el
/// mismo mecanismo de lista recursiva que usan los argumentos y los campos de
/// un literal de struct.
pub fn flatten_map_entries<'a>(entries_node: &'a ParseNode, spec: &SemanticSpec) -> Vec<&'a ParseNode> {
    match spec.map_list_symbol.as_deref() {
        Some(symbol) => flatten_arg_list(entries_node, symbol),
        None => Vec::new(),
    }
}

/// Tipo de un literal de conjunto: `Set(tipo común)`, con la misma regla de
/// homogeneidad y ensanchamiento que un arreglo.
pub fn resolve_set_literal(
    elements: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> (Option<Type>, Vec<SemanticError>) {
    let (common, errors) = common_type(elements, table, spec);
    (common.map(|t| Type::Set(Box::new(t))), errors)
}

/// Tipo de un literal de tupla: `Tuple([t0, t1, ...])`, en orden.
///
/// A diferencia del arreglo y el conjunto NO se busca un tipo común: una
/// tupla es heterogénea por definición, así que mezclar tipos es lo normal y
/// nunca produce un error de homogeneidad. Un elemento que no se puede tipar
/// se registra como `Unknown` en vez de saltarse, para no correr de posición
/// a los que vienen después — el índice es lo único que identifica a cada
/// elemento de una tupla.
pub fn resolve_tuple_literal(
    elements: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> Option<Type> {
    if elements.is_empty() {
        return None;
    }
    let items: Vec<Type> = elements
        .iter()
        .map(|e| classes::resolve_expr_type(e, table, spec).unwrap_or(Type::Unknown))
        .collect();
    Some(Type::Tuple(items))
}

/// Tipo de un literal de mapa: `Map(clave común, valor común)`.
///
/// Las claves y los valores se comprueban por separado con la misma regla de
/// homogeneidad del arreglo, así que `mapa{ "a": 1, 2: 3 }` reporta la clave
/// incompatible y `mapa{ "a": 1, "b": "x" }` el valor.
pub fn resolve_map_literal(
    entries: &[&ParseNode],
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> (Option<Type>, Vec<SemanticError>) {
    let Some(rule) = spec.map_entry.as_ref() else {
        return (None, Vec::new());
    };

    let mut keys = Vec::new();
    let mut values = Vec::new();
    for entry in entries {
        if entry.symbol != rule.production {
            continue;
        }
        if let Some(key) = entry.children.get(rule.key_index) {
            keys.push(key);
        }
        if let Some(value) = entry.children.get(rule.value_index) {
            values.push(value);
        }
    }

    let (key_ty, mut errors) = common_type(&keys, table, spec);
    let (value_ty, value_errors) = common_type(&values, table, spec);
    errors.extend(value_errors);

    let ty = match (key_ty, value_ty) {
        (Some(k), Some(v)) => Some(Type::Map(Box::new(k), Box::new(v))),
        // Un mapa vacío, o uno cuyas claves/valores no se pudieron tipar, no
        // tiene tipo inferible — igual que un arreglo vacío.
        _ => None,
    };
    (ty, errors)
}

/// Si `node` es la producción de acceso indexado configurada en
/// `spec.index_access` Y esta instancia concreta trae el corchete de
/// apertura entre sus hijos, devuelve `(nodo base, nodo del índice)`. Otras
/// alternativas del mismo head (sin corchete) devuelven `None`.
pub fn find_index_access<'a>(node: &'a ParseNode, spec: &SemanticSpec) -> Option<(&'a ParseNode, &'a ParseNode)> {
    let rule = spec.index_access.as_ref()?;
    if node.symbol != rule.production {
        return None;
    }
    if !node.children.iter().any(|c| c.symbol == rule.open_bracket_token) {
        return None;
    }
    let base = node.children.get(rule.base_index)?;
    let index = node.children.get(rule.index_index)?;
    Some((base, index))
}

/// Tipo de los elementos que produce ITERAR una colección.
///
/// - `Array(T)`/`Set(T)` -> `T`.
/// - `Map(K, _)` -> `K`: recorrer un mapa recorre sus CLAVES, como en Python
///   o JavaScript. Iterar los valores es otra operación, no ésta.
/// - `Tuple(..)` -> `None`: es heterogénea, no existe "el" tipo de sus
///   elementos, así que iterarla no tiene un tipo que ofrecer.
///
/// `None` significa "esto no se puede iterar": o no se resolvió el tipo
/// (`None`/`Unknown`, y entonces callar es lo correcto), o el tipo es
/// conocido y no es iterable — el llamador distingue los dos casos mirando el
/// tipo que pasó. Vive aquí para que `foreach` no duplique la regla.
pub fn element_type(ty: &Type) -> Option<Type> {
    match ty {
        Type::Array(inner) | Type::Set(inner) => Some(inner.as_ref().clone()),
        Type::Map(key, _) => Some(key.as_ref().clone()),
        _ => None,
    }
}

/// Índice literal constante de un subíndice, si lo es.
///
/// Baja por cadenas de un solo hijo hasta la hoja y parsea su lexema. Solo
/// hace falta para la tupla: `t[0]` y `t[1]` devuelven tipos DISTINTOS, así
/// que sin saber el valor no hay tipo que devolver. `t[i]` o `t[1+1]` dan
/// `None` — y eso no es un error, es "no lo sabemos", igual que en el resto
/// del módulo.
fn constant_index(node: &ParseNode) -> Option<usize> {
    let mut current = node;
    while current.children.len() == 1 {
        current = &current.children[0];
    }
    if !current.children.is_empty() {
        return None;
    }
    current.lexeme.as_deref()?.parse::<usize>().ok()
}

/// Valida `base[subíndice]` y devuelve el tipo del resultado, para que
/// `resolve_expr_type` pueda seguir tipando `arr[0] + 1` o `m[0][1]`.
///
/// Ramifica según lo que sea la base, que es lo que distingue a las cuatro
/// colecciones al indexarlas:
///
/// | Base | Subíndice válido | Resultado | Si no |
/// |---|---|---|---|
/// | `Array(T)` | `integer` | `T` | `IndexNotInteger` |
/// | `Map(K, V)` | compatible con `K` | `V` | `MapKeyTypeMismatch` |
/// | `Tuple(ts)` | literal entero en rango | `ts[i]` | `TupleIndexOutOfRange` |
/// | `Set(_)` y cualquier otro | — | — | `NotIndexable` |
///
/// Un conjunto NO es indexable a propósito: no tiene orden ni claves. Es la
/// diferencia observable entre `Set(T)` y `Array(T)`.
///
/// `index_node` solo lo necesita la tupla; el resto de los casos se deciden
/// con el tipo. `None` en cualquiera de los dos —base o subíndice sin
/// resolver— no reporta nada: mismo silencio que el resto de `classes`.
pub fn validate_index_access(
    base_ty: Option<&Type>,
    index_ty: Option<&Type>,
    index_node: Option<&ParseNode>,
    line: usize,
    col: usize,
) -> Result<Option<Type>, SemanticError> {
    match base_ty {
        None | Some(Type::Unknown) => Ok(None),

        Some(Type::Array(inner)) => {
            if let Some(index_ty) = index_ty {
                if !matches!(index_ty, Type::Int | Type::Unknown) {
                    return Err(SemanticError::IndexNotInteger { found: index_ty.clone(), line, col });
                }
            }
            Ok(Some(inner.as_ref().clone()))
        }

        Some(Type::Map(key, value)) => {
            if let Some(index_ty) = index_ty {
                if resolve_assignment(key, index_ty).is_err() {
                    return Err(SemanticError::MapKeyTypeMismatch {
                        expected: key.as_ref().clone(),
                        found: index_ty.clone(),
                        line,
                        col,
                    });
                }
            }
            Ok(Some(value.as_ref().clone()))
        }

        Some(Type::Tuple(items)) => {
            if let Some(index_ty) = index_ty {
                if !matches!(index_ty, Type::Int | Type::Unknown) {
                    return Err(SemanticError::IndexNotInteger { found: index_ty.clone(), line, col });
                }
            }
            // Sin un índice constante no se puede decir QUÉ posición se está
            // pidiendo, y cada posición tiene su propio tipo: se devuelve
            // `None` (desconocido) en vez de inventar uno.
            let Some(index) = index_node.and_then(constant_index) else {
                return Ok(None);
            };
            match items.get(index) {
                Some(ty) => Ok(Some(ty.clone())),
                None => Err(SemanticError::TupleIndexOutOfRange {
                    index,
                    len: items.len(),
                    line,
                    col,
                }),
            }
        }

        Some(other) => Err(SemanticError::NotIndexable { found: other.clone(), line, col }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantico::spec::{ArrayLiteralRule, IndexAccessRule};

    fn leaf(symbol: &str, lexeme: &str, line: usize, col: usize) -> ParseNode {
        ParseNode { symbol: symbol.to_string(), lexeme: Some(lexeme.to_string()), children: vec![], line, col }
    }

    fn spec_with_arrays() -> SemanticSpec {
        let mut type_tokens = std::collections::HashMap::new();
        type_tokens.insert("INT_LIT".to_string(), Type::Int);
        type_tokens.insert("STR_LIT".to_string(), Type::Str);
        SemanticSpec {
            identifier_token: "ID".to_string(),
            type_tokens,
            args_list_symbol: Some("args".to_string()),
            array_literal: Some(ArrayLiteralRule {
                production: "atom".to_string(),
                open_bracket_token: "LBRACKET".to_string(),
                elements_index: 1,
            }),
            index_access: Some(IndexAccessRule {
                production: "primary".to_string(),
                open_bracket_token: "LBRACKET".to_string(),
                base_index: 0,
                index_index: 2,
            }),
            ..Default::default()
        }
    }

    fn args_of(elements: Vec<ParseNode>) -> ParseNode {
        // args: args COMMA expr | expr -- arma la lista recursiva a mano.
        let mut iter = elements.into_iter();
        let mut acc = ParseNode::internal("args".into(), vec![iter.next().expect("al menos un elemento")]);
        for el in iter {
            acc = ParseNode::internal("args".into(), vec![acc, leaf("COMMA", ",", 0, 0), el]);
        }
        ParseNode::internal("arg_list".into(), vec![acc])
    }

    #[test]
    fn homogeneous_literal_types_as_array_of_the_common_type() {
        let table = SymbolTable::new();
        let spec = spec_with_arrays();
        let elements = vec![leaf("INT_LIT", "1", 1, 1), leaf("INT_LIT", "2", 1, 4)];
        let refs: Vec<&ParseNode> = elements.iter().collect();
        let (ty, errors) = resolve_array_literal(&refs, &table, &spec);
        assert_eq!(ty, Some(Type::Array(Box::new(Type::Int))));
        assert!(errors.is_empty());
    }

    #[test]
    fn mixed_int_and_float_widens_instead_of_erroring() {
        let table = SymbolTable::new();
        let mut spec = spec_with_arrays();
        spec.type_tokens.insert("FLOAT_LIT".to_string(), Type::Float);
        let elements = vec![leaf("INT_LIT", "1", 1, 1), leaf("FLOAT_LIT", "2.5", 1, 4)];
        let refs: Vec<&ParseNode> = elements.iter().collect();
        let (ty, errors) = resolve_array_literal(&refs, &table, &spec);
        assert_eq!(ty, Some(Type::Array(Box::new(Type::Float))));
        assert!(errors.is_empty());
    }

    #[test]
    fn heterogeneous_literal_reports_the_mismatching_element() {
        let table = SymbolTable::new();
        let spec = spec_with_arrays();
        let elements = vec![leaf("INT_LIT", "1", 1, 1), leaf("STR_LIT", "\"x\"", 1, 4)];
        let refs: Vec<&ParseNode> = elements.iter().collect();
        let (ty, errors) = resolve_array_literal(&refs, &table, &spec);
        assert_eq!(ty, Some(Type::Array(Box::new(Type::Int))));
        assert_eq!(errors.len(), 1);
        match &errors[0] {
            SemanticError::HeterogeneousArrayElements { expected, found, line, col } => {
                assert_eq!(*expected, Type::Int);
                assert_eq!(*found, Type::Str);
                assert_eq!((*line, *col), (1, 4));
            }
            other => panic!("se esperaba HeterogeneousArrayElements, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn empty_literal_has_no_inferable_type_and_no_error() {
        let table = SymbolTable::new();
        let spec = spec_with_arrays();
        let (ty, errors) = resolve_array_literal(&[], &table, &spec);
        assert_eq!(ty, None);
        assert!(errors.is_empty());
    }

    #[test]
    fn indexing_an_array_with_an_integer_returns_the_element_type() {
        let base_ty = Type::Array(Box::new(Type::Int));
        let result = validate_index_access(Some(&base_ty), Some(&Type::Int), None, 2, 3);
        assert_eq!(result, Ok(Some(Type::Int)));
    }

    #[test]
    fn indexing_two_dimensional_array_once_returns_the_inner_array() {
        let base_ty = Type::Array(Box::new(Type::Array(Box::new(Type::Int))));
        let result = validate_index_access(Some(&base_ty), Some(&Type::Int), None, 2, 3);
        assert_eq!(result, Ok(Some(Type::Array(Box::new(Type::Int)))));
    }

    #[test]
    fn indexing_with_a_non_integer_is_rejected() {
        let base_ty = Type::Array(Box::new(Type::Int));
        let result = validate_index_access(Some(&base_ty), Some(&Type::Str), None, 2, 3);
        assert_eq!(result, Err(SemanticError::IndexNotInteger { found: Type::Str, line: 2, col: 3 }));
    }

    #[test]
    fn indexing_a_non_array_is_rejected() {
        let result = validate_index_access(Some(&Type::Int), Some(&Type::Int), None, 2, 3);
        assert_eq!(result, Err(SemanticError::NotIndexable { found: Type::Int, line: 2, col: 3 }));
    }

    #[test]
    fn find_array_literal_matches_only_the_bracketed_alternative() {
        let spec = spec_with_arrays();
        let literal = ParseNode::internal(
            "atom".into(),
            vec![leaf("LBRACKET", "[", 1, 1), args_of(vec![leaf("INT_LIT", "1", 1, 2)]), leaf("RBRACKET", "]", 1, 3)],
        );
        assert!(find_array_literal(&literal, &spec).is_some());

        let other = ParseNode::internal("atom".into(), vec![leaf("ID", "x", 1, 1)]);
        assert!(find_array_literal(&other, &spec).is_none());
    }

    // ---------- mapa, conjunto y tupla ----------
    //
    // Estos tests trabajan directo sobre los tipos, sin árbol: lo que se
    // valida acá es la POLÍTICA (qué acepta cada colección al indexarse y qué
    // devuelve), no el reconocimiento por forma, que ya está probado arriba y
    // end-to-end en `tests/colecciones_tests.rs`.

    fn mapa(k: Type, v: Type) -> Type {
        Type::Map(Box::new(k), Box::new(v))
    }

    #[test]
    fn indexing_a_map_with_the_declared_key_returns_the_value_type() {
        let base = mapa(Type::Str, Type::Int);
        let got = validate_index_access(Some(&base), Some(&Type::Str), None, 1, 1)
            .expect("clave correcta");
        assert_eq!(got, Some(Type::Int), "devuelve el VALOR, no la clave");
    }

    #[test]
    fn indexing_a_map_with_the_wrong_key_type_is_rejected() {
        let base = mapa(Type::Str, Type::Int);
        let err = validate_index_access(Some(&base), Some(&Type::Int), None, 2, 3).unwrap_err();
        assert!(
            matches!(err, SemanticError::MapKeyTypeMismatch { .. }),
            "una clave del tipo equivocado no es un 'índice no entero': {err:?}"
        );
    }

    #[test]
    fn a_set_is_not_indexable() {
        // Es la diferencia observable entre `Set(T)` y `Array(T)`: mismos
        // elementos, pero el conjunto no tiene orden ni claves.
        let base = Type::Set(Box::new(Type::Int));
        let err = validate_index_access(Some(&base), Some(&Type::Int), None, 1, 1).unwrap_err();
        assert!(matches!(err, SemanticError::NotIndexable { .. }), "{err:?}");
    }

    #[test]
    fn indexing_a_tuple_types_by_position() {
        let base = Type::Tuple(vec![Type::Str, Type::Int]);
        let cero = leaf("INT_LIT", "0", 1, 1);
        let uno = leaf("INT_LIT", "1", 1, 1);

        assert_eq!(
            validate_index_access(Some(&base), Some(&Type::Int), Some(&cero), 1, 1).unwrap(),
            Some(Type::Str),
            "la posición 0 de esta tupla es texto"
        );
        assert_eq!(
            validate_index_access(Some(&base), Some(&Type::Int), Some(&uno), 1, 1).unwrap(),
            Some(Type::Int),
            "y la 1 es entero — por eso hace falta el valor del literal"
        );
    }

    #[test]
    fn a_constant_tuple_index_out_of_range_is_rejected() {
        let base = Type::Tuple(vec![Type::Str, Type::Int]);
        let cinco = leaf("INT_LIT", "5", 4, 9);
        let err = validate_index_access(Some(&base), Some(&Type::Int), Some(&cinco), 4, 9).unwrap_err();
        match err {
            SemanticError::TupleIndexOutOfRange { index, len, .. } => {
                assert_eq!((index, len), (5, 2));
            }
            other => panic!("se esperaba TupleIndexOutOfRange: {other:?}"),
        }
    }

    #[test]
    fn a_non_constant_tuple_index_is_unknown_but_not_an_error() {
        // `t[i]`: sin el valor no se sabe QUÉ posición se pide, y cada una
        // tiene su propio tipo. Callar es lo correcto — inventar uno sería
        // peor que no responder.
        let base = Type::Tuple(vec![Type::Str, Type::Int]);
        let variable = leaf("ID", "i", 1, 1);
        let got = validate_index_access(Some(&base), Some(&Type::Int), Some(&variable), 1, 1);
        assert_eq!(got.expect("no es un error"), None);
    }

    #[test]
    fn element_type_covers_every_collection() {
        assert_eq!(element_type(&Type::Array(Box::new(Type::Int))), Some(Type::Int));
        assert_eq!(element_type(&Type::Set(Box::new(Type::Str))), Some(Type::Str));
        assert_eq!(
            element_type(&mapa(Type::Str, Type::Int)),
            Some(Type::Str),
            "iterar un mapa recorre sus CLAVES"
        );
        assert_eq!(
            element_type(&Type::Tuple(vec![Type::Str, Type::Int])),
            None,
            "una tupla es heterogénea: no hay un tipo de elemento único"
        );
        assert_eq!(element_type(&Type::Int), None);
    }
}
