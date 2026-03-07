use thiserror::Error;

#[derive(Debug, Error)]
pub enum LexerGenError {
    #[error("formato de especificación inválido: {0}")]
    InvalidSpec(String),

    #[error("definición inválida: {0}")]
    InvalidDefinition(String),

    #[error("regla inválida: {0}")]
    InvalidRule(String),

    #[error("error interno: {0}")]
    Internal(String),
}