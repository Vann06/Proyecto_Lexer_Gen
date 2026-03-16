use thiserror::Error;

// LexerGenError: enum para representar los diferentes tipos de
// errores que pueden ocurrir durante el proceso de generación del lexer
#[derive(Debug, Error)]
pub enum LexerGenError {
    #[error("formato de especificación inválido: {0}")]
    InvalidSpec(String),

    #[error("definición inválida: {0}")]
    InvalidDefinition(String),

    #[allow(dead_code)]
    #[error("regla inválida: {0}")]
    InvalidRule(String),

    #[allow(dead_code)]
    #[error("error interno: {0}")]
    Internal(String),
}