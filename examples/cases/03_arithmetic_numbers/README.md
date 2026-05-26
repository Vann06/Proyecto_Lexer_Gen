# Case 03 — Numeric-Only Arithmetic

Same as case 01 but with NUMBER instead of ID.

## Files
- `lexer.yal` — tokens: NUMBER, PLUS, TIMES, LPAREN, RPAREN, WHITESPACE
- `grammar.yapar` — `E -> E + T | T`, `T -> T * F | F`, `F -> ( E ) | NUMBER`
- `input.txt` — 17 numeric expressions (integers, decimals, scientific)

## Expected results
- SLR(1) conflicts: none
- LALR conflicts: none
- All 17 input lines: **ACCEPT**

## Run
```
python src/main.py --cli tests/cases/03_arithmetic_numbers/lexer.yal tests/cases/03_arithmetic_numbers/grammar.yapar tests/cases/03_arithmetic_numbers/input.txt
```
