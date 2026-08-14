// Ejecutan las tablas construidas por `gramatica`/`automatas`/`tablas`: el
// parser LR dirigido por tabla, el parser LL(1) recursivo por tabla, y el
// árbol de derivación que ambos producen.
pub mod parser_lr;
pub mod ll1;
pub mod parse_tree;
