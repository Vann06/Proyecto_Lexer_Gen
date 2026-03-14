
// Entender la sintaxis de las expresiones regulares
// convertir regex a una AST 

use crate::error::LexerGenError;
use crate::regex::ast::RegexAst;

pub fn parse_regex(input: &str) -> Result<RegexAst, LexerGenError> {
    let chars: Vec<char> = input.chars().collect();
    let mut parser = Parser { chars, pos: 0 };
    let ast = parser.parse_union()?;

    if parser.pos < parser.chars.len() {
        return Err(LexerGenError::InvalidSpec(format!(
            "Símbolos sobrantes en regex cerca de posición {}",
            parser.pos
        )));
    }

    Ok(ast)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn parse_union(&mut self) -> Result<RegexAst, LexerGenError> {
        let mut left = self.parse_concat()?;

        while self.peek() == Some('|') {
            self.consume();
            let right = self.parse_concat()?;
            left = RegexAst::Union(Box::new(left), Box::new(right));
        }

        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<RegexAst, LexerGenError> {
        let mut nodes = Vec::new();

        while let Some(c) = self.peek() {
            if c == ')' || c == '|' {
                break;
            }

            nodes.push(self.parse_postfix()?);
        }

        if nodes.is_empty() {
            return Ok(RegexAst::Empty);
        }

        let mut result = nodes.remove(0);
        for node in nodes {
            result = RegexAst::Concat(Box::new(result), Box::new(node));
        }

        Ok(result)
    }

    fn parse_postfix(&mut self) -> Result<RegexAst, LexerGenError> {
        let mut node = self.parse_primary()?;

        loop {
            match self.peek() {
                Some('*') => {
                    self.consume();
                    node = RegexAst::Star(Box::new(node));
                }
                Some('+') => {
                    self.consume();
                    node = RegexAst::Plus(Box::new(node));
                }
                Some('?') => {
                    self.consume();
                    node = RegexAst::Optional(Box::new(node));
                }
                _ => break,
            }
        }

        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<RegexAst, LexerGenError> {
        match self.peek() {
            Some('(') => {
                self.consume();
                let expr = self.parse_union()?;
                if self.peek() != Some(')') {
                    return Err(LexerGenError::InvalidSpec(
                        "Paréntesis no cerrado en regex".to_string(),
                    ));
                }
                self.consume();
                Ok(RegexAst::Group(Box::new(expr)))
            }

            Some('[') => {
                // Inicio de clase de caracteres: [ ... ]
                self.consume();
                let mut class_content = String::new();
                while let Some(c) = self.peek() {
                    if c == ']' {
                        break;
                    }
                    class_content.push(c);
                    self.consume();
                }

                if self.peek() != Some(']') {
                    return Err(LexerGenError::InvalidSpec(
                        "Corchete no se cerró en la clase de caracteres".to_string(),
                    ));
                }
                self.consume();

                // EXPANSIÓN DE LA CLASE DE CARACTERES
                let chars_in_class: Vec<char> = class_content.chars().collect();
                let mut expanded_nodes = Vec::new();

                // Lógica básica para expandir rangos como "0-9" o "a-z"
                if chars_in_class.len() == 3 && chars_in_class[1] == '-' {
                    let start = chars_in_class[0] as u32;
                    let end = chars_in_class[2] as u32;

                    for code in start..=end {
                        if let Some(ch) = std::char::from_u32(code) {
                            expanded_nodes.push(RegexAst::Literal(ch));
                        }
                    }
                } else {
                    // Si no es un rango, solo es una lista de caracteres ej. [abc]
                    for ch in chars_in_class {
                        expanded_nodes.push(RegexAst::Literal(ch));
                    }
                }

                if expanded_nodes.is_empty() {
                    return Ok(RegexAst::Empty);
                }

                // Convertir el vector de literales en un árbol de Uniones (a | b | c ...)
                let mut result = expanded_nodes.remove(0);
                for node in expanded_nodes {
                    result = RegexAst::Union(Box::new(result), Box::new(node));
                }

                Ok(result)
            }

            Some('"') => {
                self.consume();
                let mut nodes = Vec::new();
                while let Some(c) = self.peek() {
                    if c == '"' {
                        break;
                    }
                    nodes.push(RegexAst::Literal(c));
                    self.consume();
                }

                if self.peek() != Some('"') {
                    return Err(LexerGenError::InvalidSpec(
                        String::from("Comillas dobles no cerradas"),
                    ));
                }
                self.consume(); 

                if nodes.is_empty() {
                    return Ok(RegexAst::Empty);
                }

                let mut result = nodes.remove(0);
                for node in nodes {
                    result = RegexAst::Concat(Box::new(result), Box::new(node));
                }
                
                Ok(result)
            }

            Some('\\') => {
                // Nuevo: Soporte de secuencias de escape (ej. \s, \+, \n)
                self.consume(); // consumimos la barra invertida
                if let Some(c) = self.peek() {
                    self.consume();
                    // Aquí devolverías un literal escapado
                    Ok(RegexAst::Literal(c))
                } else {
                    Err(LexerGenError::InvalidSpec(
                        "Barra invertida al final de la regex".to_string(),
                    ))
                }
            }

            Some(c) => {
                self.consume();
                Ok(RegexAst::Literal(c))
            }
            None => Err(LexerGenError::InvalidSpec(
                "Regex incompleta".to_string(),
            )),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn consume(&mut self) {
        self.pos += 1;
    }
}