
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

impl RegexAst {
    /// Imprime el AST como árbol indentado para visualización en consola.
    /// `indent` controla el nivel de sangría (multiplicado por 2 espacios).
    #[allow(dead_code)]
    pub fn pretty_print(&self, indent: usize) -> String {
        let pad = "  ".repeat(indent);
        match self {
            RegexAst::Literal(c) => format!("{}Literal('{}')", pad, c),
            RegexAst::CharClass(s) => format!("{}CharClass(\"{}\")", pad, s),
            RegexAst::Empty => format!("{}Empty", pad),
            RegexAst::Concat(l, r) => format!(
                "{}Concat(\n{},\n{}",
                pad,
                l.pretty_print(indent + 1),
                r.pretty_print(indent + 1),
            ) + &format!("\n{})", pad),
            RegexAst::Union(l, r) => format!(
                "{}Union(\n{},\n{}",
                pad,
                l.pretty_print(indent + 1),
                r.pretty_print(indent + 1),
            ) + &format!("\n{})", pad),
            RegexAst::Star(inner) => {
                format!("{}Star(\n{}\n{})", pad, inner.pretty_print(indent + 1), pad)
            }
            RegexAst::Plus(inner) => {
                format!("{}Plus(\n{}\n{})", pad, inner.pretty_print(indent + 1), pad)
            }
            RegexAst::Optional(inner) => {
                format!("{}Optional(\n{}\n{})", pad, inner.pretty_print(indent + 1), pad)
            }
            RegexAst::Group(inner) => {
                format!("{}Group(\n{}\n{})", pad, inner.pretty_print(indent + 1), pad)
            }
        }
    }
}