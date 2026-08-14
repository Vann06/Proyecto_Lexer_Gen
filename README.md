# Lexer Generator

Generador de analizadores léxicos (YALex) y sintácticos LR/LL (YAPar), con una
API HTTP y un IDE web (`frontend/IDE/`) para compilar gramáticas y ver la
traza de parseo paso a paso. Ver [GUIA_USO.md](GUIA_USO.md) para levantar el
IDE + API, o seguir leyendo para el generador de lexers standalone (CLI).
Cubre léxico y sintáctico (Fases 0–14 del libro del dragón); semántica,
código intermedio y código objetivo están planeados pero no implementados —
ver [ORGANIZACION.md](ORGANIZACION.md#fases-futuras-no-implementadas).

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

## 2. Estructura general

```text
lexer-generator/
├── Cargo.toml
├── .gitignore
├── README.md
├── Dockerfile.api
├── docker-compose.yml
├── frontend/IDE/         # IDE React (servido por nginx en el contenedor)
├── examples/
│   ├── lexer/            # .yal de ejemplo (ejemplo_c.yal, hardtest.yal, ...)
│   ├── grammar/          # .yalp de ejemplo
│   ├── source/           # fuentes de prueba para tokenizar/parsear
│   └── cases/            # casos de extremo a extremo (lexer+grammar+input+README por caso)
├── workspace/            # archivos que sirve /api/workspace (montados como volumen en Docker)
├── generated/            # salida de `cargo run` — crate standalone con el lexer generado
├── tests/                # tests de integración (cargo test)
└── src/
    ├── lib.rs                # raíz del crate — declara lexico, sintactico, api, error
    ├── main.rs                # CLI del generador de lexers standalone
    ├── error.rs
    ├── lexico/                # ── ANÁLISIS LÉXICO ──
    │   ├── spec/              #   parseo de .yal (header, definiciones, reglas, trailer)
    │   ├── regex/              #   parseo de expresiones regulares a AST
    │   ├── automata/            #   Thompson (NFA), subset construction (DFA), minimización
    │   ├── table/                #   tabla de transiciones del DFA
    │   ├── runtime/               #   simulador del lexer (maximal munch) + síntesis INDENT/DEDENT
    │   ├── codegen/                #   emite el lexer.rs standalone
    │   └── graph/                   #   exporta AST/DFA a Graphviz DOT
    ├── sintactico/            # ── ANÁLISIS SINTÁCTICO ──
    │   ├── gramatica/          #   gramática YAPar (.yalp → Grammar), FIRST/FOLLOW
    │   ├── automatas/           #   LR(0)/LR(1)/LALR
    │   ├── tablas.rs             #   tabla ACTION/GOTO, conflictos
    │   └── runtime/               #   parser LR dirigido por tabla, LL(1), árbol de derivación
    ├── api/                   # ── CAPA HTTP COMPARTIDA ── (lógica del servidor y los tests)
    └── bin/
        ├── api.rs                 # servidor HTTP (Axum) — expone api:: al IDE
        ├── test_lalr.rs            # REPL de consola para LALR(1)/LR(1)
        ├── test_ll1.rs              # REPL de consola para LL(1)
        └── test_pipeline.rs          # pipeline completo por CLI: .yal + .yalp + fuente → árbol
```

---


## Ejecución (CLI del generador de lexers)

Pruebas con varios .txt de entrada en `examples/` y generación de lexers en `generated/`.
Input txt propuestos en el proyecto (bajo `examples/source/`):
* `input_test.txt`, `input_test2.txt`: texto de prueba con tokens válidos e inválidos.
* `input_errors.txt`: texto de prueba con errores léxicos.

```bash
cargo run -- examples/lexer/ejemplo_c.yal examples/source/input_test2.txt
```

El código generado queda en `generated/src/lexer.rs` (crate standalone,
`cargo run` dentro de `generated/` lo compila y ejecuta sobre el `.txt`).