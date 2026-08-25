//! Validación semántica de funciones y procedimientos.
//!
//! Este módulo se apoya en la tabla de símbolos y en la política de tipos ya
//! existentes. No conoce nombres de producciones ni tokens de una gramática:
//! quien recorre el árbol le entrega firmas, tipos de argumentos y retornos.

use thiserror::Error;

use super::scopes::ScopeKind;
use super::symbols::{SemanticError, Signature, SymbolKind, SymbolTable};
use super::types::{resolve_assignment, Coercion, Type};

/// Error específico de la validación de funciones.
///
/// Los errores que ya pertenecen a la tabla de símbolos (por ejemplo una
/// redeclaración) se conservan sin duplicar su lógica mediante `Symbol`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FunctionError {
    #[error(transparent)]
    Symbol(#[from] SemanticError),

    #[error("'{name}' no es una función o procedimiento invocable")]
    NotCallable {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("la función '{name}' no tiene una firma disponible")]
    MissingSignature {
        name: String,
        line: usize,
        col: usize,
    },

    #[error("'{callee}' espera {expected} argumento(s), se encontraron {found}")]
    ArityMismatch {
        callee: String,
        expected: usize,
        found: usize,
        line: usize,
        col: usize,
    },

    #[error("argumento {index} de '{callee}': se esperaba {expected}, se encontró {found}")]
    ArgumentTypeMismatch {
        callee: String,
        index: usize,
        expected: Type,
        found: Type,
        line: usize,
        col: usize,
    },

    #[error("'return' solo puede usarse dentro de una función o procedimiento")]
    ReturnOutsideFunction { line: usize, col: usize },

    #[error("la función '{function}' debe retornar un valor de tipo {expected}")]
    MissingReturnValue {
        function: String,
        expected: Type,
        line: usize,
        col: usize,
    },

    #[error("el procedimiento '{function}' no puede retornar un valor de tipo {found}")]
    UnexpectedReturnValue {
        function: String,
        found: Type,
        line: usize,
        col: usize,
    },

    #[error("retorno incompatible en '{function}': se esperaba {expected}, se encontró {found}")]
    ReturnTypeMismatch {
        function: String,
        expected: Type,
        found: Type,
        line: usize,
        col: usize,
    },
}

/// Declara una función en el ámbito actual y guarda su firma inmediatamente.
///
/// Registrar la firma antes de recorrer el cuerpo permite validar llamadas
/// recursivas: cuando el cuerpo invoque a `name`, la tabla ya contiene la
/// aridad, los tipos posicionales y el retorno. La detección de redeclaración
/// sigue a cargo de `SymbolTable::declare`, por lo que mantiene exactamente
/// las mismas reglas de ámbitos que el resto del analizador.
pub fn declare_function(
    table: &mut SymbolTable,
    name: &str,
    params: Vec<Type>,
    returns: Type,
    line: usize,
    col: usize,
) -> Result<(), FunctionError> {
    table.declare(name, SymbolKind::Function, line, col)?;

    let function = table
        .lookup_mut(name)
        .expect("la función acaba de declararse en el ámbito actual");
    function.ty = Some(returns.clone());
    function.signature = Some(Signature { params, returns });
    Ok(())
}

/// Abre el ámbito de una función ya declarada.
///
/// Es un atajo pequeño para que el orden correcto sea explícito en los
/// recorridos: primero `declare_function`, después `enter_function_scope`, y
/// finalmente las declaraciones del cuerpo. Ese orden es el que habilita la
/// recursión sin introducir un caso especial en `lookup`.
pub fn enter_function_scope(table: &mut SymbolTable, name: &str) {
    table.enter_scope_named(ScopeKind::Function, name);
}

/// Valida una llamada por nombre contra la firma guardada en la tabla.
///
/// Cada posición es `Option<Type>` porque no todas las expresiones pueden
/// tiparse durante esta fase. Un tipo desconocido sigue contando para la
/// aridad, pero no produce un diagnóstico especulativo de tipo.
pub fn validate_call(
    table: &SymbolTable,
    callee: &str,
    arguments: &[Option<Type>],
    line: usize,
    col: usize,
) -> Vec<FunctionError> {
    let symbol = match table.lookup_or_err(callee, line, col) {
        Ok(symbol) => symbol,
        Err(error) => return vec![error.into()],
    };

    if !matches!(symbol.kind, SymbolKind::Function | SymbolKind::Other(_)) {
        return vec![FunctionError::NotCallable {
            name: callee.to_string(),
            line,
            col,
        }];
    }

    let signature = match &symbol.signature {
        Some(signature) => signature,
        None => {
            return vec![FunctionError::MissingSignature {
                name: callee.to_string(),
                line,
                col,
            }]
        }
    };

    validate_arguments(callee, signature, arguments, line, col)
}

/// Un desajuste entre unos argumentos y la firma que se invoca, SIN
/// comprometerse todavía con una variante concreta de error ni con una
/// posición: la misma comprobación sirve para un constructor
/// (`new Clase(...)`), para una llamada normal (`f(...)`, `obj.m(...)`) y
/// para cualquier otra forma invocable que una gramática defina, pero cada
/// una reporta con su propio mensaje y ubica el error a su manera. Cada
/// llamador mapea estos casos a lo que corresponda.
///
/// `index` es 1-based, para poder usarse tal cual en el mensaje al usuario;
/// quien necesite la posición del argumento la saca de su propia lista con
/// `index - 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgProblem {
    Arity { expected: usize, found: usize },
    ArgType { index: usize, expected: Type, found: Type },
}

/// Núcleo neutral de la comprobación de argumentos: aridad exacta, y tipo de
/// cada argumento contra el parámetro correspondiente vía `resolve_assignment`
/// — la misma tabla de coerciones que ya usa `SymbolTable::assign`.
///
/// Cada posición es `Option<Type>` porque no todas las expresiones pueden
/// tiparse en esta fase. Un tipo desconocido (`None`) sigue contando para la
/// aridad, pero no produce un diagnóstico especulativo de tipo.
///
/// Si la aridad ya está mal devuelve SOLO ese problema, sin comparar tipos
/// posicionalmente: los pares parámetro/argumento ya están desalineados y
/// cualquier diferencia de tipo sería ruido derivado del error real.
///
/// Esta es la ÚNICA implementación de la regla en el proyecto —
/// `validate_arguments`/`validate_call` acá y `classes::validate_call`/
/// `classes::validate_instantiation` son todos envoltorios sobre ella.
pub fn check_arguments(signature: &Signature, arguments: &[Option<Type>]) -> Vec<ArgProblem> {
    if signature.params.len() != arguments.len() {
        return vec![ArgProblem::Arity {
            expected: signature.params.len(),
            found: arguments.len(),
        }];
    }

    signature
        .params
        .iter()
        .zip(arguments)
        .enumerate()
        .filter_map(|(index, (expected, found))| {
            let found = found.as_ref()?;
            resolve_assignment(expected, found)
                .is_err()
                .then(|| ArgProblem::ArgType {
                    index: index + 1,
                    expected: expected.clone(),
                    found: found.clone(),
                })
        })
        .collect()
}

/// Valida argumentos ya resueltos contra una firma concreta.
///
/// Es útil también para métodos u otros símbolos invocables que ya fueron
/// resueltos por otra capa. Envoltorio sobre `check_arguments` que ubica
/// todos los problemas en la misma posición (la de la invocación).
pub fn validate_arguments(
    callee: &str,
    signature: &Signature,
    arguments: &[Option<Type>],
    line: usize,
    col: usize,
) -> Vec<FunctionError> {
    check_arguments(signature, arguments)
        .into_iter()
        .map(|problem| match problem {
            ArgProblem::Arity { expected, found } => FunctionError::ArityMismatch {
                callee: callee.to_string(),
                expected,
                found,
                line,
                col,
            },
            ArgProblem::ArgType { index, expected, found } => FunctionError::ArgumentTypeMismatch {
                callee: callee.to_string(),
                index,
                expected,
                found,
                line,
                col,
            },
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveFunction {
    name: String,
    returns: Type,
}

/// Pila de funciones activas usada para validar `return`.
///
/// Una pila, en lugar de un único valor, mantiene correcto el contexto al
/// analizar lenguajes que permiten funciones anidadas. Al salir de la interna
/// se restaura automáticamente la firma de la externa.
#[derive(Debug, Default)]
pub struct FunctionContext {
    active: Vec<ActiveFunction>,
}

impl FunctionContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter(&mut self, name: impl Into<String>, signature: &Signature) {
        self.enter_returning(name, signature.returns.clone());
    }

    /// Igual que `enter`, pero cuando solo se conoce el tipo de retorno.
    ///
    /// Es el caso del recorrido del árbol: al abrir el scope de una función
    /// su tipo declarado ya está resuelto, pero la firma completa todavía no
    /// —los parámetros se van declarando al recorrer los hijos—, y esperar a
    /// tenerla llegaría tarde para validar los `return` del cuerpo.
    pub fn enter_returning(&mut self, name: impl Into<String>, returns: Type) {
        self.active.push(ActiveFunction {
            name: name.into(),
            returns,
        });
    }

    pub fn exit(&mut self) -> bool {
        self.active.pop().is_some()
    }

    pub fn depth(&self) -> usize {
        self.active.len()
    }

    /// Comprueba un `return` contra la función activa más interna.
    ///
    /// `found == None` representa `return;`; `Some(T)` representa
    /// `return expr;`. Devuelve la coerción necesaria cuando el valor es
    /// compatible (por ejemplo `integer -> float`) y `None` para un retorno
    /// vacío válido de un procedimiento `void`.
    pub fn validate_return(
        &self,
        found: Option<&Type>,
        line: usize,
        col: usize,
    ) -> Result<Option<Coercion>, FunctionError> {
        let active = self
            .active
            .last()
            .ok_or(FunctionError::ReturnOutsideFunction { line, col })?;

        match (&active.returns, found) {
            // El tipo de retorno declarado no se pudo resolver (por ejemplo
            // un `%type_token` cuyo lado derecho cae fuera del vocabulario
            // fijo del enum `Type`). Sin un tipo esperado real no se puede
            // comprobar nada sin inventar diagnósticos: silencio deliberado.
            // Cuidado: `resolve_assignment` NO es permisivo con `Unknown`
            // —solo acepta `Unknown` contra `Unknown`—, así que sin este
            // brazo cada `return` de esa función daría un error falso.
            (Type::Unknown, _) => Ok(None),
            (Type::Void, None) => Ok(None),
            (Type::Void, Some(found)) => Err(FunctionError::UnexpectedReturnValue {
                function: active.name.clone(),
                found: found.clone(),
                line,
                col,
            }),
            (expected, None) => Err(FunctionError::MissingReturnValue {
                function: active.name.clone(),
                expected: expected.clone(),
                line,
                col,
            }),
            (expected, Some(found)) => {
                resolve_assignment(expected, found).map(Some).map_err(|_| {
                    FunctionError::ReturnTypeMismatch {
                        function: active.name.clone(),
                        expected: expected.clone(),
                        found: found.clone(),
                        line,
                        col,
                    }
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_stores_complete_signature_before_body() {
        let mut table = SymbolTable::new();
        declare_function(
            &mut table,
            "sumar",
            vec![Type::Int, Type::Int],
            Type::Int,
            1,
            1,
        )
        .unwrap();

        let function = table.lookup("sumar").unwrap();
        assert_eq!(function.ty, Some(Type::Int));
        assert_eq!(
            function.signature,
            Some(Signature {
                params: vec![Type::Int, Type::Int],
                returns: Type::Int,
            })
        );
    }

    #[test]
    fn an_unresolved_declared_return_type_is_never_checked() {
        // El tipo declarado cayo en `Unknown` (p.ej. un %type_token con un
        // nombre fuera del vocabulario fijo de `Type`). `resolve_assignment`
        // trata `Unknown` como incompatible con todo salvo consigo mismo, asi
        // que sin la guarda cada `return` daria un error inventado.
        let mut context = FunctionContext::new();
        context.enter_returning("misteriosa", Type::Unknown);

        assert!(context.validate_return(Some(&Type::Int), 1, 1).is_ok());
        assert!(context.validate_return(Some(&Type::Str), 2, 1).is_ok());
        assert!(context.validate_return(None, 3, 1).is_ok(), "un `return;` tampoco se chequea");
    }

    #[test]
    fn a_return_outside_any_function_is_an_error() {
        let context = FunctionContext::new();
        assert!(matches!(
            context.validate_return(Some(&Type::Int), 4, 2),
            Err(FunctionError::ReturnOutsideFunction { line: 4, col: 2 })
        ));
    }

    #[test]
    fn wrong_arity_suppresses_positional_type_problems() {
        // Con las posiciones desalineadas, cualquier diferencia de tipo es
        // ruido derivado del error real: se reporta solo la aridad.
        let signature = Signature { params: vec![Type::Int], returns: Type::Void };
        let problems = check_arguments(&signature, &[Some(Type::Str), Some(Type::Str)]);
        assert_eq!(problems, vec![ArgProblem::Arity { expected: 1, found: 2 }]);
    }

    #[test]
    fn an_untypable_argument_counts_for_arity_but_not_for_types() {
        let signature = Signature { params: vec![Type::Int, Type::Int], returns: Type::Void };
        let problems = check_arguments(&signature, &[Some(Type::Int), None]);
        assert!(problems.is_empty(), "un argumento sin tipo resuelto no se compara: {problems:?}");
    }

    #[test]
    fn return_context_restores_outer_function_after_nested_one() {
        let outer = Signature {
            params: vec![],
            returns: Type::Int,
        };
        let inner = Signature {
            params: vec![],
            returns: Type::Str,
        };
        let mut context = FunctionContext::new();
        context.enter("externa", &outer);
        context.enter("interna", &inner);

        assert!(context.validate_return(Some(&Type::Str), 1, 1).is_ok());
        assert!(context.exit());
        assert!(context.validate_return(Some(&Type::Int), 2, 1).is_ok());
        assert_eq!(context.depth(), 1);
    }
}
