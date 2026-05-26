# Case 02 — Extended Arithmetic

Arithmetic with subtraction, division, and numeric literals.

## Files
- `lexer.yal` — tokens: ID, NUMBER, PLUS, MINUS, TIMES, DIV, LPAREN, RPAREN, WHITESPACE
- `grammar.yapar` — `E -> E + T | E - T | T`, `T -> T * F | T / F | F`, `F -> ( E ) | ID | NUMBER`
- `input.txt` — 18 expressions with identifiers, integers, decimals, scientific notation

## Expected results
- SLR(1) conflicts: none
- LALR conflicts: none
- LL(1): conflict (left-recursive)
- All 18 input lines: **ACCEPT**

## Run
```
python src/main.py --cli tests/cases/02_arithmetic_extended/lexer.yal tests/cases/02_arithmetic_extended/grammar.yapar tests/cases/02_arithmetic_extended/input.txt
```
