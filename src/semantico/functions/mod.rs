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

/// Valida argumentos ya resueltos contra una firma concreta.
///
/// Es útil también para métodos u otros símbolos invocables que ya fueron
/// resueltos por otra capa. Si falla la aridad se reporta solo ese problema:
/// comparar tipos con posiciones desalineadas produciría errores derivados.
pub fn validate_arguments(
    callee: &str,
    signature: &Signature,
    arguments: &[Option<Type>],
    line: usize,
    col: usize,
) -> Vec<FunctionError> {
    if signature.params.len() != arguments.len() {
        return vec![FunctionError::ArityMismatch {
            callee: callee.to_string(),
            expected: signature.params.len(),
            found: arguments.len(),
            line,
            col,
        }];
    }

    signature
        .params
        .iter()
        .zip(arguments)
        .enumerate()
        .filter_map(|(index, (expected, found))| {
            let found = found.as_ref()?;
            resolve_assignment(expected, found).is_err().then(|| {
                FunctionError::ArgumentTypeMismatch {
                    callee: callee.to_string(),
                    index: index + 1,
                    expected: expected.clone(),
                    found: found.clone(),
                    line,
                    col,
                }
            })
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
        self.active.push(ActiveFunction {
            name: name.into(),
            returns: signature.returns.clone(),
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
