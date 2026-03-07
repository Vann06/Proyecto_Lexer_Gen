# Lexer Generator

Proyecto de Generador de Analizadores Léxicos a partir de especificaciones en YALex.

## Objetivo
Leer un archivo `.yal`, procesar sus definiciones y reglas, construir internamente los autómatas necesarios y generar un analizador léxico funcional.

## Flujo del proyecto
1. Parseo de especificación YALex
2. Expansión de definiciones (`let`)
3. Parseo de expresiones regulares a AST
4. Construcción de AFN (Thompson)
5. Conversión AFN -> AFD
6. Minimización de AFD
7. Generación de tabla de transiciones
8. Simulación del lexer
9. Generación de código fuente del analizador

---

## 2. Estructura general sugerida

```text
lexer-generator/
├── Cargo.toml
├── .gitignore
├── README.md
├── examples/
│   └── basic/
│       ├── lexer.yal
│       └── input.txt
├── generated/
│   └── .gitkeep
├── graphs/
│   └── .gitkeep
├── tests/
│   └── smoke_test.rs
└── src/
    ├── main.rs
    ├── error.rs
    ├── spec/
    │   ├── mod.rs
    │   ├── ast.rs
    │   ├── parser.rs
    │   └── expand.rs
    ├── regex/
    │   ├── mod.rs
    │   ├── ast.rs
    │   └── parser.rs
    ├── automata/
    │   ├── mod.rs
    │   ├── nfa.rs
    │   ├── dfa.rs
    │   ├── subset.rs
    │   └── minimize.rs
    ├── table/
    │   ├── mod.rs
    │   └── transition_table.rs
    ├── runtime/
    │   ├── mod.rs
    │   └── simulator.rs
    ├── codegen/
    │   ├── mod.rs
    │   └── rust_codegen.rs
    └── graph/
        ├── mod.rs
        └── dot.rs
```

---


## Ejecución
```bash
cargo run 