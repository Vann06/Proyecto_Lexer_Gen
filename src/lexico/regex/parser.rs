
// Entender la sintaxis de las expresiones regulares
// convertir regex a una AST 

use crate::error::LexerGenError;
use crate::lexico::regex::ast::RegexAst;

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
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                self.consume();
            } else {
                break;
            }
        }
    }

    fn parse_union(&mut self) -> Result<RegexAst, LexerGenError> {
        self.skip_ws();
        let mut left = self.parse_concat()?;

        loop {
            self.skip_ws();
            if self.peek() == Some('|') {
                self.consume();
                self.skip_ws();
                let right = self.parse_concat()?;
                left = RegexAst::Union(Box::new(left), Box::new(right));
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<RegexAst, LexerGenError> {
        let mut nodes = Vec::new();

        loop {
            self.skip_ws();
            match self.peek() {
                None | Some(')') | Some('|') => break,
                _ => nodes.push(self.parse_postfix()?),
            }
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
                    
                    if c == '\\' {
                        self.consume();
                        if let Some(escaped) = self.peek() {
                            match escaped {
                                'n' => class_content.push('\n'),
                                't' => class_content.push('\t'),
                                'r' => class_content.push('\r'),
                                's' => {
                                    class_content.push(' ');
                                    class_content.push('\t');
                                    class_content.push('\n');
                                    class_content.push('\r');
                                }
                                other => class_content.push(other),
                            }
                            self.consume();
                        }
                    } else {
                        class_content.push(c);
                        self.consume();
                    }
                }

                if self.peek() != Some(']') {
                    return Err(LexerGenError::InvalidSpec(
                        "Corchete no se cerró en la clase de caracteres".to_string(),
                    ));
                }
                self.consume();

                // DEVOLVEMOS EL NODO CHARCLASS PARA QUE EL NFA LO PROCESE (REGLA 9)
                Ok(RegexAst::CharClass(class_content))
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

            Some('\'') => {
                self.consume();
                let mut nodes = Vec::new();
                while let Some(c) = self.peek() {
                    if c == '\'' {
                        break;
                    }
                    if c == '\\' {
                        // Escapes DENTRO de un literal '...' (p. ej. '\n'): antes esto
                        // no se decodificaba aquí — el bucle empujaba los DOS
                        // caracteres literales '\' y 'n' en vez de un salto de línea
                        // real, así que `let newline = '\n'` (la forma en que YALex
                        // normalmente escribe un literal de salto de línea, y la que
                        // usan los propios .yal de ejemplo del repo) nunca matcheaba
                        // un '\n' de verdad. Misma tabla de escapes que el caso suelto
                        // `Some('\\')` de más abajo, vía `Self::decode_escape`.
                        self.consume(); // la barra invertida
                        match self.peek() {
                            Some(escaped) => {
                                nodes.push(Self::decode_escape(escaped));
                                self.consume();
                            }
                            None => {
                                return Err(LexerGenError::InvalidSpec(
                                    "Barra invertida al final de la regex".to_string(),
                                ));
                            }
                        }
                        continue;
                    }
                    nodes.push(RegexAst::Literal(c));
                    self.consume();
                }

                if self.peek() != Some('\'') {
                    return Err(LexerGenError::InvalidSpec(
                        String::from("Comillas simples no cerradas"),
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
                // Soporte de secuencias de escape sueltas (ej. \s, \+, \n)
                self.consume(); // consumimos la barra invertida
                if let Some(c) = self.peek() {
                    self.consume();
                    Ok(Self::decode_escape(c))
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

    /// Decodifica una secuencia de escape `\X` (la `X` ya consumida por el
    /// llamador) a su AST. Compartido entre el escape suelto (fuera de
    /// comillas, p. ej. `\+`) y el escape dentro de un literal `'...'`
    /// (p. ej. `'\n'`) — antes solo el primer caso decodificaba, así que
    /// `let newline = '\n'` nunca matcheaba un salto de línea real.
    fn decode_escape(c: char) -> RegexAst {
        match c {
            'n' => RegexAst::Literal('\n'),
            't' => RegexAst::Literal('\t'),
            'r' => RegexAst::Literal('\r'),
            's' => {
                // Expandimos \s a espacio, \t, \n y \r
                let space = RegexAst::Union(Box::new(RegexAst::Literal(' ')), Box::new(RegexAst::Literal('\t')));
                let return_chars = RegexAst::Union(Box::new(RegexAst::Literal('\n')), Box::new(RegexAst::Literal('\r')));
                RegexAst::Union(Box::new(space), Box::new(return_chars))
            }
            _ => RegexAst::Literal(c),
        }
    }
}