# Case 01 — Arithmetic with Identifiers

Classic LR grammar for arithmetic expressions over identifiers.

## Files
- `lexer.yal` — tokens: ID, PLUS, TIMES, LPAREN, RPAREN, WHITESPACE
- `grammar.yapar` — productions for `E -> E + T | T`, `T -> T * F | F`, `F -> ( E ) | ID`
- `input.txt` — 12 valid arithmetic expressions

## Expected results
- LR(0): 12 states
- SLR(1) conflicts: none
- LALR conflicts: none
- LL(1): conflict (left-recursive grammar)
- All 12 input lines: **ACCEPT** under SLR(1) and LALR

## Run
```
python src/main.py --cli tests/cases/01_arithmetic_id/lexer.yal tests/cases/01_arithmetic_id/grammar.yapar tests/cases/01_arithmetic_id/input.txt
```
