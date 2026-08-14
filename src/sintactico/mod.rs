// Análisis sintáctico: .yalp → Grammar → FIRST/FOLLOW → LR(0)/LR(1)/LALR/LL(1)
// → tabla ACTION/GOTO → parseo.
pub mod gramatica;
pub mod automatas;
pub mod tablas;
pub mod runtime;
