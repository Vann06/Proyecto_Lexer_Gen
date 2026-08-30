//! Validación semántica de expresiones binarias y unarias.
//!
//! Cubre las tres familias que la tabla aritmética de `types` no toca:
//! operadores lógicos (`&& || !`), comparaciones (`== != < <= > >=`) y
//! operadores unarios (`!`, `-`). Además concentra la regla de "sentido
//! semántico" de una expresión: qué puede aparecer como OPERANDO, más allá de
//! qué tipo tiene.
//!
//! Igual que `flow`, este módulo no conoce ninguna palabra reservada ni ningún
//! nombre de producción: el `.yalp` declara qué token es cada operador con
//! `%logic`/`%compare`/`%unary`, y acá solo llegan tipos ya resueltos y
//! posiciones. La detección de la FORMA del nodo (`find_*`) también es
//! genérica — un nodo de tres hijos cuyo hijo del medio sea un token
//! declarado, o de dos hijos cuyo primero lo sea.
//!
//! División de responsabilidades con `types`: acá se decide QUÉ OPERANDOS
//! acepta cada operador; toda pregunta de compatibilidad entre dos tipos se
//! delega a `types::resolve_assignment`, para no abrir una segunda tabla de
//! coerciones en paralelo a la que ya es el punto único.

use std::fmt;

use thiserror::Error;

use super::spec::SemanticSpec;
use super::symbols::{SymbolKind, SymbolTable};
use super::types::{resolve_assignment, Type};
use crate::sintactico::runtime::parse_tree::ParseNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOperator {
    And,
    Or,
}

impl fmt::Display for LogicalOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogicalOperator::And => "&&",
            LogicalOperator::Or => "||",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOperator {
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
}

impl ComparisonOperator {
    /// `<`, `<=`, `>`, `>=` ordenan, y ordenar exige operandos numéricos.
    /// `==`/`!=` solo exigen que los dos lados sean comparables entre sí.
    fn is_ordering(&self) -> bool {
        matches!(
            self,
            ComparisonOperator::Lt
                | ComparisonOperator::Lte
                | ComparisonOperator::Gt
                | ComparisonOperator::Gte
        )
    }
}

impl fmt::Display for ComparisonOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ComparisonOperator::Eq => "==",
            ComparisonOperator::Neq => "!=",
            ComparisonOperator::Lt => "<",
            ComparisonOperator::Lte => "<=",
            ComparisonOperator::Gt => ">",
            ComparisonOperator::Gte => ">=",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Negación lógica (`!`): exige `bool`.
    Not,
    /// Negación aritmética (`-`): exige un tipo numérico.
    Negate,
}

impl fmt::Display for UnaryOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            UnaryOperator::Not => "!",
            UnaryOperator::Negate => "-",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OperatorError {
    #[error("el operador '{operator}' requiere operandos bool, se encontró {found}")]
    LogicalOperandNotBoolean {
        operator: String,
        found: Type,
        line: usize,
        col: usize,
    },

    #[error("no se pueden comparar con '{operator}' operandos {left} y {right}")]
    IncompatibleComparison {
        operator: String,
        left: Type,
        right: Type,
        line: usize,
        col: usize,
    },

    #[error("el operador unario '{operator}' no acepta un operando {found}")]
    UnaryOperandMismatch {
        operator: String,
        found: Type,
        line: usize,
        col: usize,
    },

    #[error("'{name}' es {kind} y no puede usarse como valor en una expresión")]
    NonValueOperand {
        name: String,
        kind: String,
        line: usize,
        col: usize,
    },
}

/// `&&` / `||`: los dos operandos deben ser `bool`.
///
/// El resultado es `Bool` SIEMPRE —incluso si un operando no se pudo tipar—
/// porque una conjunción es booleana por construcción; lo único que se saltea
/// con un tipo desconocido es la validación del operando, no el tipo del
/// resultado. `Unknown` es neutro, igual que en `types::arithmetic`.
pub fn resolve_logical(
    operator: LogicalOperator,
    left: &Type,
    right: &Type,
    line: usize,
    col: usize,
) -> Result<Type, OperatorError> {
    for operand in [left, right] {
        if matches!(operand, Type::Unknown) {
            continue;
        }
        if !matches!(operand, Type::Bool) {
            return Err(OperatorError::LogicalOperandNotBoolean {
                operator: operator.to_string(),
                found: operand.clone(),
                line,
                col,
            });
        }
    }
    Ok(Type::Bool)
}

/// `== != < <= > >=`: siempre producen `bool`; lo que se valida son los
/// operandos.
///
/// Ordenar (`< <= > >=`) exige que los dos lados sean numéricos. Igualar
/// (`== !=`) solo exige que sean compatibles, y esa pregunta se delega a
/// `types::resolve_assignment` —el punto único de coerciones— probando en los
/// dos sentidos: `integer == float` es válido aunque `resolve_assignment` solo
/// acepte el ensanchamiento en una dirección.
pub fn resolve_comparison(
    operator: ComparisonOperator,
    left: &Type,
    right: &Type,
    line: usize,
    col: usize,
) -> Result<Type, OperatorError> {
    if matches!(left, Type::Unknown) || matches!(right, Type::Unknown) {
        return Ok(Type::Bool);
    }

    let compatible = if operator.is_ordering() {
        left.is_numeric() && right.is_numeric()
    } else {
        resolve_assignment(left, right).is_ok() || resolve_assignment(right, left).is_ok()
    };

    if compatible {
        Ok(Type::Bool)
    } else {
        Err(OperatorError::IncompatibleComparison {
            operator: operator.to_string(),
            left: left.clone(),
            right: right.clone(),
            line,
            col,
        })
    }
}

/// `!` exige `bool` y produce `bool`; `-` exige un numérico y CONSERVA su tipo
/// (negar un `integer` da `integer`, no `float`).
pub fn resolve_unary(
    operator: UnaryOperator,
    operand: &Type,
    line: usize,
    col: usize,
) -> Result<Type, OperatorError> {
    if matches!(operand, Type::Unknown) {
        return Ok(match operator {
            UnaryOperator::Not => Type::Bool,
            UnaryOperator::Negate => Type::Unknown,
        });
    }

    match operator {
        UnaryOperator::Not if matches!(operand, Type::Bool) => Ok(Type::Bool),
        UnaryOperator::Not => Err(OperatorError::LogicalOperandNotBoolean {
            operator: operator.to_string(),
            found: operand.clone(),
            line,
            col,
        }),
        UnaryOperator::Negate if operand.is_numeric() => Ok(operand.clone()),
        UnaryOperator::Negate => Err(OperatorError::UnaryOperandMismatch {
            operator: operator.to_string(),
            found: operand.clone(),
            line,
            col,
        }),
    }
}

/// Sentido semántico de un operando: una función, una clase o un struct
/// NOMBRADOS A SECAS no son valores, aunque tengan un tipo asociado.
///
/// Es el caso "no multiplicar funciones" del enunciado, y sin esta regla pasa
/// desapercibido: `classes::resolve_expr_type` sobre el identificador de una
/// función devuelve su TIPO DE RETORNO, así que `f * 2` con `f(): integer` se
/// ve idéntico a multiplicar un entero.
///
/// La regla mira la FORMA del nodo —solo dispara sobre una HOJA identificador—
/// así que `f(1) * 2` sigue siendo válido: ahí el operando es el nodo de
/// llamada, no la hoja con el nombre.
pub fn non_value_operand(
    node: &ParseNode,
    table: &SymbolTable,
    spec: &SemanticSpec,
) -> Option<OperatorError> {
    // El operando no llega como la hoja pelada: el parser interpone toda la
    // cadena de precedencia (`term -> unary -> primary -> atom -> ID`), que es
    // una sucesión de nodos de UN solo hijo. Se desciende por ella igual que
    // hace `classes::resolve_expr_type`. Un nodo con varios hijos corta el
    // descenso, que es justo lo que hace que una llamada (`f(1)`) o un
    // paréntesis no se confundan con el nombre pelado.
    let node = innermost_value(node);
    if !node.children.is_empty() || node.symbol != spec.identifier_token {
        return None;
    }
    let name = node.lexeme.as_deref().unwrap_or(&node.symbol);
    let symbol = table.lookup(name)?;
    let kind = match symbol.kind {
        SymbolKind::Function => "una función",
        SymbolKind::Class => "una clase",
        SymbolKind::Struct => "un tipo registro",
        _ => return None,
    };
    Some(OperatorError::NonValueOperand {
        name: name.to_string(),
        kind: kind.to_string(),
        line: node.line,
        col: node.col,
    })
}

/// Baja por la cadena de producciones de un solo hijo hasta el primer nodo que
/// tenga otra forma (una hoja, o un nodo con varios hijos).
fn innermost_value(node: &ParseNode) -> &ParseNode {
    let mut current = node;
    while current.children.len() == 1 {
        current = &current.children[0];
    }
    current
}

/// Nodo con forma `izquierda OP derecha` donde `OP` es un token declarado con
/// `%logic`. Mismo criterio que `classes::find_arithmetic`: se reconoce por la
/// FORMA (tres hijos, el del medio es el operador), no por el nombre de la
/// producción, así una gramática no tiene que enumerar `or_expr`/`and_expr`.
pub fn find_logical<'a>(
    node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Option<(LogicalOperator, &'a ParseNode, &'a ParseNode)> {
    if node.children.len() != 3 {
        return None;
    }
    let op = *spec.logic_tokens.get(&node.children[1].symbol)?;
    Some((op, &node.children[0], &node.children[2]))
}

/// Igual que `find_logical`, para los tokens declarados con `%compare`.
pub fn find_comparison<'a>(
    node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Option<(ComparisonOperator, &'a ParseNode, &'a ParseNode)> {
    if node.children.len() != 3 {
        return None;
    }
    let op = *spec.compare_tokens.get(&node.children[1].symbol)?;
    Some((op, &node.children[0], &node.children[2]))
}

/// Nodo con forma `OP operando` (`unary: NOT unary | MINUS unary`): dos hijos,
/// el PRIMERO es un token declarado con `%unary`.
pub fn find_unary<'a>(
    node: &'a ParseNode,
    spec: &SemanticSpec,
) -> Option<(UnaryOperator, &'a ParseNode)> {
    if node.children.len() != 2 {
        return None;
    }
    let op = *spec.unary_tokens.get(&node.children[0].symbol)?;
    Some((op, &node.children[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_operators_require_boolean_operands() {
        for op in [LogicalOperator::And, LogicalOperator::Or] {
            assert_eq!(resolve_logical(op, &Type::Bool, &Type::Bool, 1, 1), Ok(Type::Bool));

            assert!(matches!(
                resolve_logical(op, &Type::Int, &Type::Bool, 3, 5),
                Err(OperatorError::LogicalOperandNotBoolean { line: 3, col: 5, .. })
            ));
            assert!(matches!(
                resolve_logical(op, &Type::Bool, &Type::Str, 1, 1),
                Err(OperatorError::LogicalOperandNotBoolean { .. })
            ));

            // Un operando sin tipar no inventa un diagnóstico, pero el
            // resultado sigue siendo booleano por construcción.
            assert_eq!(resolve_logical(op, &Type::Unknown, &Type::Bool, 1, 1), Ok(Type::Bool));
            assert_eq!(resolve_logical(op, &Type::Unknown, &Type::Unknown, 1, 1), Ok(Type::Bool));

            // Que un operando sea desconocido NO tapa al otro: el `Int` es
            // incorrecto con independencia de lo que sea el de al lado.
            assert!(matches!(
                resolve_logical(op, &Type::Unknown, &Type::Int, 1, 1),
                Err(OperatorError::LogicalOperandNotBoolean { found: Type::Int, .. })
            ));
        }
    }

    #[test]
    fn ordering_comparisons_require_numeric_operands() {
        let ordering = [
            ComparisonOperator::Lt,
            ComparisonOperator::Lte,
            ComparisonOperator::Gt,
            ComparisonOperator::Gte,
        ];
        for op in ordering {
            assert_eq!(resolve_comparison(op, &Type::Int, &Type::Int, 1, 1), Ok(Type::Bool));
            assert_eq!(resolve_comparison(op, &Type::Int, &Type::Float, 1, 1), Ok(Type::Bool));

            // Ordenar booleanos, textos o instancias no tiene sentido.
            for bad in [Type::Bool, Type::Str, Type::Named("Punto".to_string())] {
                assert!(
                    matches!(
                        resolve_comparison(op, &bad, &Type::Int, 2, 4),
                        Err(OperatorError::IncompatibleComparison { line: 2, col: 4, .. })
                    ),
                    "{op} no debería ordenar {bad}"
                );
            }
        }
    }

    #[test]
    fn equality_accepts_any_compatible_pair_but_not_unrelated_types() {
        for op in [ComparisonOperator::Eq, ComparisonOperator::Neq] {
            // Mismo primitivo, y el par numérico en AMBOS sentidos.
            for ty in [Type::Int, Type::Float, Type::Bool, Type::Str] {
                assert_eq!(resolve_comparison(op, &ty, &ty, 1, 1), Ok(Type::Bool));
            }
            assert_eq!(resolve_comparison(op, &Type::Int, &Type::Float, 1, 1), Ok(Type::Bool));
            assert_eq!(resolve_comparison(op, &Type::Float, &Type::Int, 1, 1), Ok(Type::Bool));

            // Mismo tipo nominal sí; nominales distintos no.
            let punto = Type::Named("Punto".to_string());
            let vector = Type::Named("Vector".to_string());
            assert_eq!(resolve_comparison(op, &punto, &punto, 1, 1), Ok(Type::Bool));
            assert!(matches!(
                resolve_comparison(op, &punto, &vector, 1, 1),
                Err(OperatorError::IncompatibleComparison { .. })
            ));

            assert!(matches!(
                resolve_comparison(op, &Type::Bool, &Type::Str, 1, 1),
                Err(OperatorError::IncompatibleComparison { .. })
            ));

            // Sin tipar: sin diagnóstico, resultado booleano igual.
            assert_eq!(resolve_comparison(op, &Type::Unknown, &Type::Str, 1, 1), Ok(Type::Bool));
        }
    }

    #[test]
    fn unary_not_needs_bool_and_negate_preserves_the_numeric_type() {
        assert_eq!(resolve_unary(UnaryOperator::Not, &Type::Bool, 1, 1), Ok(Type::Bool));
        assert!(matches!(
            resolve_unary(UnaryOperator::Not, &Type::Int, 7, 2),
            Err(OperatorError::LogicalOperandNotBoolean { line: 7, col: 2, .. })
        ));

        // Negar conserva el tipo: `-entero` sigue siendo entero.
        assert_eq!(resolve_unary(UnaryOperator::Negate, &Type::Int, 1, 1), Ok(Type::Int));
        assert_eq!(resolve_unary(UnaryOperator::Negate, &Type::Float, 1, 1), Ok(Type::Float));
        assert!(matches!(
            resolve_unary(UnaryOperator::Negate, &Type::Str, 4, 9),
            Err(OperatorError::UnaryOperandMismatch { line: 4, col: 9, .. })
        ));

        // Sin tipar: `!` es booleano igual, `-` se queda sin saber.
        assert_eq!(resolve_unary(UnaryOperator::Not, &Type::Unknown, 1, 1), Ok(Type::Bool));
        assert_eq!(resolve_unary(UnaryOperator::Negate, &Type::Unknown, 1, 1), Ok(Type::Unknown));
    }
}
