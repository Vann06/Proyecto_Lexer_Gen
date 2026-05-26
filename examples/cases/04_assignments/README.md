# Case 04 — Assignments and Comparisons

Statement-level grammar with assignment (`:=`), comparison (`<`, `=`), and arithmetic.

## Files
- `lexer.yal` — tokens: ID, NUMBER, SEMICOLON, ASSIGNOP, LT, EQ, PLUS, MINUS, TIMES, DIV, LPAREN, RPAREN, WHITESPACE
- `grammar.yapar` — `program -> stmt_list`, `stmt -> ID := expr ; | comparison`, plus expression hierarchy
- `input.txt` — 23 statements (assignments and comparisons)

## Expected results
- SLR(1) conflicts: none
- LALR conflicts: none
- All 23 input lines: **ACCEPT**

## Run
```
python src/main.py --cli tests/cases/04_assignments/lexer.yal tests/cases/04_assignments/grammar.yapar tests/cases/04_assignments/input.txt
```
