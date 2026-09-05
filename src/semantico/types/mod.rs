//! Sistema de tipos y tabla central de compatibilidad semántica.
//!
//! Las decisiones de coerción viven aquí para que el analizador, la tabla de
//! símbolos y las fases posteriores no dupliquen reglas.

use std::fmt;

use thiserror::Error;

pub mod annotations;
pub use annotations::TypeAnnotations;

/// Tipos que puede almacenar la tabla de símbolos.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Float,
    Bool,
    Str,
    Void,
    Named(String),
    Array(Box<Type>),
    /// Mapa/diccionario: tipo de la clave y tipo del valor. Indexarlo con una
    /// clave compatible devuelve el valor.
    Map(Box<Type>, Box<Type>),
    /// Conjunto: elementos homogéneos, sin orden ni índice. NO es indexable —
    /// esa es justo la diferencia con `Array` a nivel de tipos.
    Set(Box<Type>),
    /// Tupla: longitud fija y tipos HETEROGÉNEOS, uno por posición. Por eso
    /// indexarla solo se puede tipar cuando el índice es un literal constante:
    /// `t[0]` y `t[1]` pueden devolver tipos distintos.
    Tuple(Vec<Type>),
    Unknown,
}

impl Type {
    /// `integer` y `float` — los dos tipos sobre los que operan la tabla
    /// aritmética y los operadores de orden (`< <= > >=`). Vive acá, y no en
    /// `operators`, para que exista UN solo lugar que decida qué cuenta como
    /// numérico.
    pub fn is_numeric(&self) -> bool {
        matches!(self, Type::Int | Type::Float)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Int => write!(f, "integer"),
            Type::Float => write!(f, "float"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "string"),
            Type::Void => write!(f, "void"),
            Type::Named(name) => write!(f, "{name}"),
            Type::Array(inner) => write!(f, "{inner}[]"),
            Type::Map(key, value) => write!(f, "mapa<{key}, {value}>"),
            Type::Set(inner) => write!(f, "conjunto<{inner}>"),
            Type::Tuple(items) => {
                let partes: Vec<String> = items.iter().map(|t| t.to_string()).collect();
                write!(f, "tupla<{}>", partes.join(", "))
            }
            Type::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArithmeticOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl fmt::Display for ArithmeticOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ArithmeticOperator::Add => "+",
            ArithmeticOperator::Subtract => "-",
            ArithmeticOperator::Multiply => "*",
            ArithmeticOperator::Divide => "/",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coercion {
    /// Los operandos ya tienen el tipo requerido.
    Exact,
    /// Conversión segura y automática de `integer` a `float`.
    IntToFloat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArithmeticResolution {
    pub result: Type,
    pub left_coercion: Coercion,
    pub right_coercion: Coercion,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TypeError {
    #[error("el operador '{operator}' no acepta operandos {left} y {right}")]
    InvalidArithmetic {
        operator: ArithmeticOperator,
        left: Type,
        right: Type,
    },
    #[error("no se puede asignar un valor {found} a una declaración {expected}")]
    IncompatibleAssignment { expected: Type, found: Type },
}

/// Una fila de la tabla para operadores binarios numéricos.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ArithmeticRule {
    left: Type,
    right: Type,
    result: Type,
    left_coercion: Coercion,
    right_coercion: Coercion,
}

/// Una fila de la tabla de asignaciones (`found` se asigna a `expected`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct AssignmentRule {
    expected: Type,
    found: Type,
    coercion: Coercion,
}

/// Matriz numérica compartida por `+`, `-`, `*` y `/`.
///
/// La división entre dos enteros conserva el tipo `integer`. Cuando aparece
/// al menos un `float`, el resultado es `float` y el operando entero se
/// amplía de forma segura antes de evaluar la operación.
const ARITHMETIC_RULES: [ArithmeticRule; 4] = [
    ArithmeticRule {
        left: Type::Int,
        right: Type::Int,
        result: Type::Int,
        left_coercion: Coercion::Exact,
        right_coercion: Coercion::Exact,
    },
    ArithmeticRule {
        left: Type::Int,
        right: Type::Float,
        result: Type::Float,
        left_coercion: Coercion::IntToFloat,
        right_coercion: Coercion::Exact,
    },
    ArithmeticRule {
        left: Type::Float,
        right: Type::Int,
        result: Type::Float,
        left_coercion: Coercion::Exact,
        right_coercion: Coercion::IntToFloat,
    },
    ArithmeticRule {
        left: Type::Float,
        right: Type::Float,
        result: Type::Float,
        left_coercion: Coercion::Exact,
        right_coercion: Coercion::Exact,
    },
];

/// Tabla completa de asignaciones permitidas. La única coerción implícita
/// es la ampliación `integer -> float`; el estrechamiento inverso se rechaza.
const ASSIGNMENT_RULES: [AssignmentRule; 6] = [
    AssignmentRule {
        expected: Type::Int,
        found: Type::Int,
        coercion: Coercion::Exact,
    },
    AssignmentRule {
        expected: Type::Float,
        found: Type::Int,
        coercion: Coercion::IntToFloat,
    },
    AssignmentRule {
        expected: Type::Float,
        found: Type::Float,
        coercion: Coercion::Exact,
    },
    AssignmentRule {
        expected: Type::Bool,
        found: Type::Bool,
        coercion: Coercion::Exact,
    },
    AssignmentRule {
        expected: Type::Str,
        found: Type::Str,
        coercion: Coercion::Exact,
    },
    AssignmentRule {
        expected: Type::Void,
        found: Type::Void,
        coercion: Coercion::Exact,
    },
];

/// Punto único de consulta para todas las reglas de compatibilidad.
///
/// Es una estructura sin estado: las reglas son constantes y deterministas.
/// Exponerla como valor permite inyectar la misma política en otras fases sin
/// reconstruir matrices ni duplicar condicionales.
#[derive(Debug, Clone, Copy, Default)]
pub struct CompatibilityTable;

pub const TYPE_COMPATIBILITY: CompatibilityTable = CompatibilityTable;

impl CompatibilityTable {
    /// Resuelve el tipo resultante y las coerciones de `+`, `-`, `*` y `/`.
    pub fn arithmetic(
        &self,
        operator: ArithmeticOperator,
        left: &Type,
        right: &Type,
    ) -> Result<ArithmeticResolution, TypeError> {
        // `Unknown` no es un tipo: es "esta fase todavía no sabe tipar esto".
        // Con un operando así no se puede AFIRMAR que la operación sea
        // inválida, y reportarla igual produce un diagnóstico inventado
        // encima de la causa real. El resultado también queda `Unknown` y se
        // propaga hacia arriba, que es la respuesta honesta.
        //
        // Misma política que ya aplican `flow::validate_condition` (deja
        // pasar `Unknown`) y `classes::resolve_expr_type` (devuelve `None`
        // en vez de adivinar).
        if matches!(left, Type::Unknown) || matches!(right, Type::Unknown) {
            return Ok(ArithmeticResolution {
                result: Type::Unknown,
                left_coercion: Coercion::Exact,
                right_coercion: Coercion::Exact,
            });
        }

        ARITHMETIC_RULES
            .iter()
            .find(|rule| &rule.left == left && &rule.right == right)
            .map(|rule| ArithmeticResolution {
                result: rule.result.clone(),
                left_coercion: rule.left_coercion,
                right_coercion: rule.right_coercion,
            })
            .ok_or_else(|| TypeError::InvalidArithmetic {
                operator,
                left: left.clone(),
                right: right.clone(),
            })
    }

    /// Comprueba una asignación contra el tipo declarado.
    pub fn assignment(&self, expected: &Type, found: &Type) -> Result<Coercion, TypeError> {
        // `Unknown` de CUALQUIERA de los dos lados se deja pasar, por el mismo
        // motivo que en `arithmetic`: significa "no lo sabemos todavía", no un
        // tipo incompatible. Antes solo se aceptaba `Unknown` contra `Unknown`,
        // y eso convertía cada hueco de tipado en un error falso — un `%type_of`
        // que apunta a un terminal sin `%type_token`, o un `const K = f();`
        // cuyo inicializador no se sabe tipar, hacían que toda declaración,
        // asignación, argumento o `return` de ese símbolo reportara S006/S014/
        // S016. `functions::validate_return` llegó a llevar un parche local
        // para tapar su parte del problema; la regla vive acá, que es donde el
        // resto de las fases la consultan.
        if matches!(expected, Type::Unknown) || matches!(found, Type::Unknown) {
            return Ok(Coercion::Exact);
        }

        // Los tipos compuestos y nominales son compatibles únicamente cuando
        // son exactamente iguales. Los primitivos viven explícitamente en la
        // tabla para que toda coerción siga centralizada y sea auditable.
        //
        // OJO al agregar un compuesto nuevo: si no se lo nombra acá, cae al
        // `find` de abajo, no encuentra fila —las reglas solo cubren
        // primitivos— y termina siendo incompatible HASTA CONSIGO MISMO.
        if matches!(
            expected,
            Type::Named(_) | Type::Array(_) | Type::Map(_, _) | Type::Set(_) | Type::Tuple(_)
        ) && expected == found
        {
            return Ok(Coercion::Exact);
        }

        ASSIGNMENT_RULES
            .iter()
            .find(|rule| &rule.expected == expected && &rule.found == found)
            .map(|rule| rule.coercion)
            .ok_or_else(|| TypeError::IncompatibleAssignment {
                expected: expected.clone(),
                found: found.clone(),
            })
    }
}

/// Atajo para consultar la tabla aritmética global.
pub fn resolve_arithmetic(
    operator: ArithmeticOperator,
    left: &Type,
    right: &Type,
) -> Result<ArithmeticResolution, TypeError> {
    TYPE_COMPATIBILITY.arithmetic(operator, left, right)
}

/// Atajo para consultar la tabla global de asignaciones.
pub fn resolve_assignment(expected: &Type, found: &Type) -> Result<Coercion, TypeError> {
    TYPE_COMPATIBILITY.assignment(expected, found)
}
