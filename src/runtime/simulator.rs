// Fase 12: Simulación del lexer sobre texto real.
// Maximal munch + desempate por prioridad (ya fijada en la tabla).

use crate::table::transition_table::{TransitionTable, DEAD};

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: String,
    pub lexeme: String,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum LexResult {
    Token(Token),
    Error { lexeme: char, line: usize, col: usize },
    EOF,
}

pub struct Simulator<'a> {
    table: &'a TransitionTable,
    input: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Simulator<'a> {
    pub fn new(table: &'a TransitionTable, input: &str) -> Self {
        Simulator {
            table,
            input: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    /// Retorna el siguiente token (maximal munch).
    pub fn next_token(&mut self) -> LexResult {
        if self.pos >= self.input.len() {
            return LexResult::EOF;
        }

        let mut state = self.table.start as i32;
        let start_pos = self.pos;
        let start_line = self.line;
        let start_col = self.col;

        let mut last_accept_pos: Option<usize> = None;
        let mut last_accept_token: Option<String> = None;

        while self.pos < self.input.len() {
            let c = self.input[self.pos];
            if (c as usize) >= 128 {
                break;
            }
            let next = self.table.next(state as usize, c);
            if next == DEAD {
                break;
            }
            state = next;
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }

            if self.table.is_accepting(state as usize) {
                last_accept_pos = Some(self.pos);
                last_accept_token = self.table.token_at(state as usize).map(String::from);
            }
        }

        if let Some(accept_pos) = last_accept_pos {
            self.pos = accept_pos;
            let lexeme: String = self.input[start_pos..accept_pos].iter().collect();
            LexResult::Token(Token {
                kind: last_accept_token.unwrap_or_default(),
                lexeme,
                line: start_line,
                col: start_col,
            })
        } else {
            let bad = self.input[start_pos];
            self.pos = start_pos + 1;
            if bad == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
            LexResult::Error {
                lexeme: bad,
                line: start_line,
                col: start_col,
            }
        }
    }

    /// Tokeniza todo el input.
    pub fn tokenize(&mut self) -> (Vec<Token>, Vec<String>) {
        let mut tokens = Vec::new();
        let mut errors = Vec::new();
        loop {
            match self.next_token() {                
                // Consumimos pero no devolvemos tokens de Whitespace
                LexResult::Token(t) if t.kind.contains("Whitespace") => {
                    // simplemente los ignoramos
                }
                LexResult::Token(t) => tokens.push(t),
                LexResult::Error { lexeme, line, col } => {
                    errors.push(format!("Error línea {}:{} — carácter '{}'", line, col, lexeme));
                }
                LexResult::EOF => break,
            }
        }
        (tokens, errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::transition_table;

    #[test]
    fn test_simulator_maximal_munch() {
        // Tabla dummy: estado 0 -> con dígito va a 1; estado 1 acepta "NUM", con dígito sigue en 1
        let dfa = transition_table::make_dummy_dfa_num();
        let tt = transition_table::build(&dfa);
        let mut sim = Simulator::new(&tt, "42");
        let (tokens, errors) = sim.tokenize();
        assert_eq!(errors.len(), 0);
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].lexeme, "42");
        assert_eq!(tokens[0].kind, "NUM");
        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].col, 1);
    }

    #[test]
    fn test_simulator_error_on_invalid_char() {
        let dfa = transition_table::make_dummy_dfa_num();
        let tt = transition_table::build(&dfa);
        let mut sim = Simulator::new(&tt, "42x");
        let (tokens, errors) = sim.tokenize();
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].lexeme, "42");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains('x'));
    }
}
