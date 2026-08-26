//! Reglas semánticas de control de flujo.
//!
//! Este módulo no conoce palabras reservadas ni nombres de producciones. El
//! `.yalp` decide qué nodo contiene una condición, cuál abre un bucle y cuáles
//! representan `break`/`continue`; el analyzer solo entrega aquí el tipo y la
//! posición resueltos.

use thiserror::Error;

use super::types::Type;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FlowError {
    #[error("la condición debe ser de tipo bool, se encontró {found}")]
    ConditionNotBoolean {
        found: Type,
        line: usize,
        col: usize,
    },

    #[error("'break' solo puede usarse dentro de un bucle")]
    BreakOutsideLoop { line: usize, col: usize },

    #[error("'continue' solo puede usarse dentro de un bucle")]
    ContinueOutsideLoop { line: usize, col: usize },
}

/// Comprueba el tipo ya resuelto de una condición.
///
/// `None` y `Type::Unknown` no generan un diagnóstico: significan que otra
/// fase todavía no sabe tipar la expresión. Rechazarla aquí inventaría un
/// error derivado y duplicaría el diagnóstico de la causa real.
pub fn validate_condition(found: Option<&Type>, line: usize, col: usize) -> Result<(), FlowError> {
    match found {
        None | Some(Type::Unknown) | Some(Type::Bool) => Ok(()),
        Some(found) => Err(FlowError::ConditionNotBoolean {
            found: found.clone(),
            line,
            col,
        }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextKind {
    Function,
    Loop,
}

/// Pila de contextos que delimita saltos de control.
///
/// La frontera de función es importante: una función declarada dentro de un
/// bucle no puede ejecutar `break` para salir del bucle de la función externa.
/// Solo cuenta un `Loop` encontrado antes que el `Function` más cercano.
#[derive(Debug, Default)]
pub struct FlowContext {
    stack: Vec<ContextKind>,
}

impl FlowContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter_function(&mut self) {
        self.stack.push(ContextKind::Function);
    }

    pub fn exit_function(&mut self) -> bool {
        self.exit(ContextKind::Function)
    }

    pub fn enter_loop(&mut self) {
        self.stack.push(ContextKind::Loop);
    }

    pub fn exit_loop(&mut self) -> bool {
        self.exit(ContextKind::Loop)
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    pub fn validate_break(&self, line: usize, col: usize) -> Result<(), FlowError> {
        if self.has_loop_in_current_function() {
            Ok(())
        } else {
            Err(FlowError::BreakOutsideLoop { line, col })
        }
    }

    pub fn validate_continue(&self, line: usize, col: usize) -> Result<(), FlowError> {
        if self.has_loop_in_current_function() {
            Ok(())
        } else {
            Err(FlowError::ContinueOutsideLoop { line, col })
        }
    }

    fn has_loop_in_current_function(&self) -> bool {
        for context in self.stack.iter().rev() {
            match context {
                ContextKind::Loop => return true,
                ContextKind::Function => return false,
            }
        }
        false
    }

    fn exit(&mut self, expected: ContextKind) -> bool {
        if self.stack.last() == Some(&expected) {
            self.stack.pop();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_require_bool_but_ignore_unresolved_types() {
        assert!(validate_condition(Some(&Type::Bool), 1, 1).is_ok());
        assert!(validate_condition(Some(&Type::Unknown), 1, 1).is_ok());
        assert!(validate_condition(None, 1, 1).is_ok());
        assert_eq!(
            validate_condition(Some(&Type::Int), 4, 7),
            Err(FlowError::ConditionNotBoolean {
                found: Type::Int,
                line: 4,
                col: 7,
            })
        );
    }

    #[test]
    fn break_and_continue_require_a_loop() {
        let mut context = FlowContext::new();
        assert!(matches!(
            context.validate_break(1, 1),
            Err(FlowError::BreakOutsideLoop { .. })
        ));
        assert!(matches!(
            context.validate_continue(2, 1),
            Err(FlowError::ContinueOutsideLoop { .. })
        ));

        context.enter_loop();
        assert!(context.validate_break(3, 1).is_ok());
        assert!(context.validate_continue(4, 1).is_ok());
        assert!(context.exit_loop());
        assert_eq!(context.depth(), 0);
    }

    #[test]
    fn a_nested_function_cannot_jump_to_an_outer_loop() {
        let mut context = FlowContext::new();
        context.enter_loop();
        context.enter_function();

        assert!(matches!(
            context.validate_break(1, 1),
            Err(FlowError::BreakOutsideLoop { .. })
        ));
        assert!(matches!(
            context.validate_continue(1, 1),
            Err(FlowError::ContinueOutsideLoop { .. })
        ));

        context.enter_loop();
        assert!(context.validate_break(2, 1).is_ok());
        assert!(context.validate_continue(2, 1).is_ok());
        assert!(context.exit_loop());
        assert!(context.exit_function());
        assert!(context.exit_loop());
    }
}
