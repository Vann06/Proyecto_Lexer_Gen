// Análisis léxico: .yal → SpecIR → regex/AST → NFA → DFA → tabla → simulación/codegen.
pub mod spec;
pub mod regex;
pub mod automata;
pub mod table;
pub mod runtime;
pub mod codegen;
pub mod graph;
