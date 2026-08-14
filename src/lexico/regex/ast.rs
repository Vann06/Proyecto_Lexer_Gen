
// Definicion de piezas del arbol
// Fase Regex -> AST

// RegexAst: representa la estructura de un regex como un árbol
// Cada nodo refleja una operación o símbolo del lenguaje regular
#[derive(Debug, Clone)]
pub enum RegexAst {
    /// Carácter literal, p.ej. 'a'
    Literal(char),
    /// Concatenación: A seguido de B
    Concat(Box<RegexAst>, Box<RegexAst>),
    /// Unión (alternancia): A o B
    Union(Box<RegexAst>, Box<RegexAst>),
    /// Clausura de Kleene: A*
    Star(Box<RegexAst>),
    /// Una o más repeticiones: A+
    Plus(Box<RegexAst>),
    /// Cero o una ocurrencia: A?
    Optional(Box<RegexAst>),
    /// Grupo entre paréntesis: (A) — preserva precedencia explícita
    Group(Box<RegexAst>),
    /// Clase de caracteres, p.ej. [a-z]
    CharClass(String),
    /// Expresión vacía (épsilon)
    Empty,
}
