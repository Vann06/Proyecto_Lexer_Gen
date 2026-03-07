
// Definicion de piezas del arbol
// Fase Regex -> AST 

#[derive(Debug, Clone)]
pub enum RegexAst {
    Literal(char),
    Concat(Box<RegexAst>, Box<RegexAst>),
    Union(Box<RegexAst>, Box<RegexAst>),
    Star(Box<RegexAst>),
    Plus(Box<RegexAst>),
    Optional(Box<RegexAst>),
    CharClass(String),
    Empty,
}