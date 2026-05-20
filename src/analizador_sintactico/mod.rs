pub mod grammar;
pub mod first;
pub mod follow;
pub mod lr0;
pub mod lr1;
pub mod lalr;
pub mod tables;
pub mod parser_lr;
pub mod ll1;
pub mod parse_tree;
// pub mod slr;  // FUTURO: SLR(1) reusa LRTable y LRParser