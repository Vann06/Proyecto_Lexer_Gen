
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