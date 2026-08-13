// Crate root shared by every binary and integration test. Before this file existed,
// each binary/test remounted the module tree with `#[path = "..."] mod X;`, so the
// same code compiled once per consumer and `crate::` paths only resolved by accident
// (whichever binary happened to declare the module under that exact name at its root).
pub mod analizador_sintactico;
pub mod api;
pub mod automata;
pub mod codegen;
pub mod error;
pub mod graph;
pub mod regex;
pub mod runtime;
pub mod spec;
pub mod table;
