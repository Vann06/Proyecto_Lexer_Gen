// Crate root shared by every binary and integration test. Before this file existed,
// each binary/test remounted the module tree with `#[path = "..."] mod X;`, so the
// same code compiled once per consumer and `crate::` paths only resolved by accident
// (whichever binary happened to declare the module under that exact name at its root).
pub mod api;
pub mod codigo_objetivo;
pub mod error;
pub mod intermedio;
pub mod lexico;
pub mod semantico;
pub mod sintactico;
