# Case 06 — Rejection Examples (Negative Tests)

Intentionally malformed inputs to verify parser correctly rejects them.

## Files
- `lexer.yal` / `grammar.yapar` — same arithmetic grammar as case 01
- `input.txt` — 8 invalid expressions

## Expected results
All 8 inputs should be **REJECT** under SLR(1) and LALR, with descriptive error messages:

| Input | Reason |
|-------|--------|
| `A + + B` | Two consecutive `+` |
| `A B` | Two consecutive IDs without operator |
| `+ A` | Leading `+` |
| `(A + B` | Missing closing paren |
| `A + B)` | Extra closing paren |
| `A * * B` | Two consecutive `*` |
| `((A + B)` | Missing one closing paren |
| `A + (B *)` | Operator before `)` |

## Run
```
python src/main.py --cli tests/cases/06_rejection_examples/lexer.yal tests/cases/06_rejection_examples/grammar.yapar tests/cases/06_rejection_examples/input.txt
```

In the GUI, rejected lines are highlighted red in the input editor with wavy underline.
