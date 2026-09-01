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
                    // Compatible tal cual (incluida la ampliación normal
                    // integer -> float ya cubierta por la propia tabla).
                } else if resolve_assignment(&found, expected).is_ok() {
                    // El elemento nuevo es el tipo más ancho (p.ej. el común
                    // era integer y este es float): ensanchar el tipo común.
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

    (common.map(|t| Type::Array(Box::new(t))), errors)
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

/// Valida `base[índice]`: la base debe ser un arreglo y el índice, integer.
/// Devuelve el tipo del elemento (para que `resolve_expr_type` pueda seguir
/// tipando `arr[0] + 1`, y `matrix[0][1]` indexando dos veces).
///
/// `None` en cualquiera de los dos tipos —no se pudo resolver la base o el
/// índice— no reporta nada: mismo silencio que el resto de `classes`.
pub fn validate_index_access(
    base_ty: Option<&Type>,
    index_ty: Option<&Type>,
    line: usize,
    col: usize,
) -> Result<Option<Type>, SemanticError> {
    let inner = match base_ty {
        None | Some(Type::Unknown) => return Ok(None),
        Some(Type::Array(inner)) => Some(inner.as_ref().clone()),
        Some(other) => return Err(SemanticError::NotIndexable { found: other.clone(), line, col }),
    };

    if let Some(index_ty) = index_ty {
        if !matches!(index_ty, Type::Int | Type::Unknown) {
            return Err(SemanticError::IndexNotInteger { found: index_ty.clone(), line, col });
        }
    }

    Ok(inner)
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
        let result = validate_index_access(Some(&base_ty), Some(&Type::Int), 2, 3);
        assert_eq!(result, Ok(Some(Type::Int)));
    }

    #[test]
    fn indexing_two_dimensional_array_once_returns_the_inner_array() {
        let base_ty = Type::Array(Box::new(Type::Array(Box::new(Type::Int))));
        let result = validate_index_access(Some(&base_ty), Some(&Type::Int), 2, 3);
        assert_eq!(result, Ok(Some(Type::Array(Box::new(Type::Int)))));
    }

    #[test]
    fn indexing_with_a_non_integer_is_rejected() {
        let base_ty = Type::Array(Box::new(Type::Int));
        let result = validate_index_access(Some(&base_ty), Some(&Type::Str), 2, 3);
        assert_eq!(result, Err(SemanticError::IndexNotInteger { found: Type::Str, line: 2, col: 3 }));
    }

    #[test]
    fn indexing_a_non_array_is_rejected() {
        let result = validate_index_access(Some(&Type::Int), Some(&Type::Int), 2, 3);
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
}
