# Case 05 — Classes, Functions, Lists

Most complex case: class declarations, function declarations with parameters,
return statements, function calls, list literals, recursion.

## Files
- `lexer.yal` — tokens: ID, NUMBER, STRING, CLASS, DEF, RETURN, ASSIGN, plus operators and punctuation
- `grammar.yapar` — `program -> decl_list`, `class_decl`, `func_decl`, `func_call`, `list_expr`
- `input.txt` — 5 inputs exercising assignments, function defs, classes, function calls, list literals

## Expected results
- LR(0): 67 states (large grammar)
- SLR(1) conflicts: none
- LALR conflicts: none
- LL(1): many conflicts (left-recursive)
- All 5 input lines: **ACCEPT**

## Parse trees
After running, check `output/tree_01.png` through `output/tree_05.png` for visual confirmation:
1. `x = 1 ;` — simple assignment
2. `def foo ( x , y ) { ... }` — function with params and body
3. `class MyClass { def bar ( ) { ... } }` — class with empty-param method (uses ε production)
4. `result = foo ( 1 , 2 ) ;` — function call
5. `items = [ 1 , 2 , 3 ] ;` — list literal

## Run
```
python src/main.py --cli tests/cases/05_classes_functions/lexer.yal tests/cases/05_classes_functions/grammar.yapar tests/cases/05_classes_functions/input.txt
```
