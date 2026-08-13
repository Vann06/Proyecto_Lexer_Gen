# Guía del Pipeline — Lexer-Generator

Documento para agentes o front-ends que quieran consumir o invocar el sistema.

> **El IDE real (`frontend/IDE/`) no usa nada de esto.** Habla con el servidor
> HTTP (`src/bin/api.rs`) descrito en [GUIA_USO.md](GUIA_USO.md) —
> `POST /api/pipeline` hace exactamente lo que describe este documento, pero
> devuelve JSON en vez de que el caller tenga que invocar un binario y
> parsear su stdout/stderr. La sección "Flujo completo recomendado para un
> front-end" de más abajo es el diseño anterior al servidor HTTP; se deja
> como referencia de los binarios de CLI (siguen siendo útiles para debug
> manual), no como la integración recomendada.

---

## Visión general

El proyecto es un **generador de lexers y parsers** escrito en Rust. A partir de tres archivos de entrada el sistema:

1. Compila un **lexer** desde una especificación `.yal` (estilo OCaml YALex).
2. Construye un **parser** (LALR(1), SLR(1) o LL(1)) desde una gramática `.yalp` (estilo YACC/Bison).
3. Tokeniza un **archivo fuente** con el lexer generado.
4. Parsea los tokens con el parser y produce un **árbol de derivación**.
5. Escribe el árbol en ASCII (stdout) y en DOT (Graphviz) en `output/`.

---

## Archivos de entrada

| Rol | Extensión | Descripción |
|---|---|---|
| Especificación léxica | `.yal` | Define tokens con expresiones regulares |
| Gramática | `.yalp` | Define la gramática con directivas de tokens y producciones |
| Código fuente | cualquiera | El texto a analizar (`.c`, `.py`, `.txt`, etc.) |

### Formato `.yal` (lexer)

```
(* comentario *)
let nombre = patron

rule tokens =
    patron1    { "NOMBRE_TOKEN" }
  | patron2    { "NOMBRE_TOKEN" }
  | patron3    {}            (* ignorado — no pasa al parser *)
```

**Patrones soportados:**
- Rango de caracteres: `['a'-'z']`
- Alternativa: `a | b`
- Concatenación implícita: `'a''b'` ó `ab` (si `ab` no es macro definida)
- Cuantificadores: `*` (cero o más), `+` (uno o más), `?` (opcional)
- Carácter literal: `'x'` (comillas simples)
- Macros: `let nombre = ...` y luego `nombre` en otra definición

**Archivos de ejemplo disponibles:**

| Archivo | Para qué |
|---|---|
| `examples/lexer/hardtest_sim.yal` | Subconjunto de C (int, char, void, if, while, return...) |
| `examples/lexer/hardtest.yal` | Subconjunto de Python |
| `examples/lexer/python.yal` | Python simplificado |
| `examples/lexer/expr.yal` | Expresiones aritméticas simples |
| `examples/lexer/ejemplo_c.yal` | C básico alternativo |

### Formato `.yalp` (gramática)

```yacc
%token TOKEN1 TOKEN2 TOKEN3

%left  PLUS MINUS          (* precedencia baja, asociativo izquierda *)
%right STAR SLASH          (* precedencia alta, asociativo derecha *)
%nonassoc LT GT            (* no asociativo *)

%%

simbolo_inicial:
    produccion_A
  | produccion_B
;

produccion_A:
    TOKEN1 simbolo_inicial TOKEN2
  | /* vacio */
;
```

**Directivas:**
- `%token` — declara terminales (requerido para todos los tokens del lexer)
- `%left` / `%right` / `%nonassoc` — resuelven conflictos S/R por precedencia
- `%%` — separa declaraciones de producciones
- `/* vacio */` — producción épsilon

**Archivos de ejemplo disponibles:**

| Archivo | Descripción |
|---|---|
| `examples/grammar/hardtest_sim.yalp` | C: funciones, if, while, expresiones estratificadas |
| `examples/grammar/hardtest.yalp` | Python: def, if, while, for, class |
| `examples/grammar/expr_left_recursive.yalp` | Expresiones con recursión izquierda (LALR) |
| `examples/grammar/expr_ll1.yalp` | Expresiones sin recursión izquierda (LL1) |
| `examples/grammar/expr_ambiguous_prec.yalp` | Expresiones con directivas de precedencia |
| `examples/grammar/lalr_cc.yalp` | Gramática clásica de conflictos LALR |
| `examples/grammar/ejemplo_c.yalp` | C básico alternativo |

### Archivos fuente de ejemplo

| Archivo | Lexer que usar | Gramática que usar |
|---|---|---|
| `examples/source/test_c.c` | `hardtest_sim.yal` | `hardtest_sim.yalp` |
| `examples/source/hardtest.py` | `hardtest.yal` | `hardtest.yalp` |
| `examples/source/expr.txt` | `expr.yal` | `expr_left_recursive.yalp` |

---

## Binarios disponibles

### `test_pipeline` — **Pipeline completo (el principal)**

```bash
cargo run --bin test_pipeline -- <gramatica.yalp> <lexer.yal> <fuente> [--lalr|--slr|--ll1]
```

**Modo por defecto:** `--lalr`

**Ejemplo funcional:**
```bash
cargo run --bin test_pipeline -- \
  examples/grammar/hardtest_sim.yalp \
  examples/lexer/hardtest_sim.yal \
  examples/source/test_c.c \
  --lalr
```

**Salida en stdout:**
```
=== PIPELINE LEXER → PARSER → ÁRBOL ===
  gramática : examples/grammar/hardtest_sim.yalp
  lexer     : examples/lexer/hardtest_sim.yal
  fuente    : examples/source/test_c.c
  modo      : LALR

✓ Lexer construido (97 estados).
✓ Lexer produjo 383 tokens raw.
✓ Tras filtrar ignorables: 227 tokens al parser.
  tokens: ["INT", "ID", "LPAREN", ...]

⚠ 1 conflicto(s) en la tabla:
   SHIFT-REDUCE en estado I128 con 'ELSE': shift→I130 vs reduce (...). Se conserva SHIFT.

--- ÁRBOL DE DERIVACIÓN ---
programa
└── lista_externas
    ├── lista_externas
    │   └── declaracion_externa
    │       └── definicion_funcion
    │           ├── tipo
    │           │   └── INT (int)
    │           ├── ID (suma)
    ...

✓ DOT escrito en output/parse_tree_lalr.dot (genera PNG con: dot -Tpng output/parse_tree_lalr.dot -o tree.png)
```

**Archivos generados:**

| Modo | Archivo DOT |
|---|---|
| `--lalr` | `output/parse_tree_lalr.dot` |
| `--slr` | `output/parse_tree_slr.dot` |
| `--ll1` | `output/parse_tree_ll1.dot` |

**Código de salida:**
- `0` — parseo exitoso
- `2` — error sintáctico (la gramática rechazó la cadena)
- `1` — error de argumentos / archivo no encontrado

---

### `test_lalr` — Parser LALR interactivo

```bash
cargo run --bin test_lalr
```

Pide la ruta del `.yalp` por stdin, imprime:
- Producciones de la gramática
- Todos los estados LR(1) canónicos con sus items `[A → α•β, lookahead]`
- Mapa de fusión LR(1) → LALR (qué estados se fusionaron)
- Todos los estados LALR resultantes
- La tabla ACTION/GOTO completa
- Resumen de conflictos

Luego entra en un loop interactivo donde se ingresan secuencias de tokens a mano:
```
> INT ID LPAREN INT ID COMMA INT ID RPAREN LBRACE ...
✓ Cadena VÁLIDA.
--- ÁRBOL DE DERIVACIÓN ---
...
(DOT escrito en output/parse_tree_lalr.dot)
```

**Cuándo usar:** Para inspeccionar la tabla y los estados sin necesitar un archivo fuente.

---

### `test_ll1` — Parser LL(1) interactivo

```bash
cargo run --bin test_ll1
```

Pide la ruta del `.yalp`, aplica transformaciones automáticas (eliminación de recursión izquierda + factorización izquierda), imprime:
- Log de transformaciones aplicadas
- Gramática después de limpieza
- FIRST y FOLLOW de cada no-terminal
- Tabla LL(1) (`M[A, a] → producción`)

Luego loop interactivo igual que `test_lalr`.

**Limitación:** Solo funciona con gramáticas que no tengan conflictos después de las transformaciones. No todas las gramáticas son LL(1).

---

## Flujo interno del pipeline

```
.yal  ──► parse_yalex() ──► SpecIR
                              │
                     expand_definitions()
                              │
                     parse_regex() (por cada regla)
                              │
                     build_nfa_from_ast()   ← Thompson
                              │
                     build_dfa_from_nfa()   ← Subconjuntos
                              │
                     minimize_dfa()         ← Hopcroft
                              │
                     build() → TransitionTable
                              │
.fuente ─────────────────► Simulator::next_token()
                              │
                        Vec<Token> (raw)
                              │
                        filtrar ignorables
                              │
                        Vec<ParseToken> { kind, lexeme }
                              │
.yalp ──► Grammar::parse_for_lr() ──► Grammar
                              │
              ┌──────────────┼──────────────┐
              ▼              ▼              ▼
          LALR(1)         SLR(1)         LL(1)
              │              │              │
     LR1Automaton     LR0Automaton   calculate_first
     merge_by_core    calculate_follow  calculate_follow
     LRTable::        LRTable::      LL1Parser::build()
     build_from_lalr  build_from_slr
              │              │              │
              └──────────────┴──────────────┘
                              │
                        LRParser / LL1Parser
                              │
                        parse_tree(tokens)
                              │
                        ParseNode (árbol)
                              │
                   ┌──────────┴──────────┐
                   ▼                     ▼
              print_ascii()           to_dot()
              (stdout)          output/parse_tree_*.dot
```

---

## Estructura de datos clave

### `Token` (salida del lexer)

```rust
pub struct Token {
    pub kind:   String,   // nombre del token (ej: "ID", "INT", "PLUS")
    pub action: String,   // acción raw del .yal (ej: "return ID")
    pub lexeme: String,   // texto coincidido (ej: "suma", "int", "+")
    pub line:   usize,    // línea en el archivo fuente (1-based)
    pub col:    usize,    // columna (1-based)
}
```

### `ParseToken` (entrada al parser)

```rust
pub struct ParseToken {
    pub kind:   String,   // token normalizado en UPPERCASE (coincide con %token del .yalp)
    pub lexeme: String,   // texto original para mostrar en las hojas del árbol
}
```

### `ParseNode` (árbol de derivación)

```rust
pub struct ParseNode {
    pub symbol:   String,           // nombre del NT o terminal
    pub lexeme:   Option<String>,   // Some("texto") solo en hojas (terminales)
    pub children: Vec<ParseNode>,   // vacío en hojas
}
```

**Hojas** (terminales): `children` vacío, `lexeme = Some("texto")`.
**Nodos internos** (no-terminales): `children` con sus expansiones, `lexeme = None`.
**Épsilon**: `symbol = "ε"`, `lexeme = None`, `children` vacío.

---

## Generación del PNG del árbol

El archivo DOT generado en `output/` puede convertirse a imagen con Graphviz:

```bash
# PNG
dot -Tpng output/parse_tree_lalr.dot -o tree.png

# SVG (recomendado para web — vectorial)
dot -Tsvg output/parse_tree_lalr.dot -o tree.svg

# PDF
dot -Tpdf output/parse_tree_lalr.dot -o tree.pdf
```

**Instalar Graphviz si no está disponible:**
```bash
sudo pacman -S graphviz        # Arch/Manjaro
sudo apt install graphviz      # Debian/Ubuntu
brew install graphviz          # macOS
```

**Verificar disponibilidad:**
```bash
which dot && dot -V
```

---

## Flujo completo recomendado para un front-end

```bash
# 1. Ejecutar el pipeline
cargo run --bin test_pipeline -- \
  examples/grammar/hardtest_sim.yalp \
  examples/lexer/hardtest_sim.yal \
  examples/source/test_c.c \
  --lalr

# 2. Convertir el árbol DOT a imagen
dot -Tpng output/parse_tree_lalr.dot -o output/parse_tree_lalr.png

# 3. Mostrar la imagen al usuario
```

El front-end puede capturar:
- **stdout** — árbol ASCII + resumen de tokens + conflictos
- **stderr** — errores léxicos y sintácticos
- **código de salida** — `0` OK, `2` error sintáctico
- **`output/parse_tree_lalr.dot`** — para renderizar con Graphviz o cualquier librería DOT
- **`output/parse_tree_lalr.png`** — imagen lista para mostrar

---

## Tabla de conflictos y estados

El binario `test_lalr` imprime todo esto de forma detallada. Un front-end puede ejecutarlo y capturar stdout:

```bash
echo "examples/grammar/hardtest_sim.yalp" | cargo run --bin test_lalr 2>&1
```

La salida incluye:

1. **Producciones** — `A → α | β | ...` numeradas
2. **Estados LR(1)** — cada estado con sus items `[A → α•β, lookahead]`
3. **Mapa de fusión** — `LALR I5 ← LR1 [12, 34]`
4. **Estados LALR** — mismo formato, con lookaheads fusionados
5. **Tabla ACTION/GOTO** — filas = estados, columnas = símbolos
   - `Sx` = shift al estado x
   - `Ry` = reduce por producción y
   - `Gx` = goto al estado x
   - `ACC` = aceptar
6. **Conflictos** — `SHIFT-REDUCE en estado Ix con 'TOKEN': shift→Iy vs reduce (A → α). Se conserva SHIFT.`

---

## Errores comunes

| Error | Causa | Solución |
|---|---|---|
| `Error al leer fuente: stream did not contain valid UTF-8` | El archivo fuente tiene chars no-ASCII (Unicode en comentarios) | Eliminar comentarios con caracteres especiales del fuente |
| `Error sintáctico: estado I0, token 'SLASH'` | El fuente tiene `/* */` que el lexer no conoce | Usar fuente sin comentarios de bloque, o agregar regla `/* */` al .yal |
| `LL(1) no construible` | La gramática tiene conflictos después de transformaciones | Usar `--lalr` en lugar de `--ll1` |
| `Símbolos sobrantes en regex` | Error de sintaxis en patrón del .yal | Revisar el patrón (comillas sin cerrar, corchetes, etc.) |
| `Error al parsear .yal` | Sintaxis incorrecta del archivo .yal | Ver formato de .yal en la sección de arriba |

---

## Pares lexer/gramática probados y funcionando

| `.yal` | `.yalp` | Fuente | Modo |
|---|---|---|---|
| `hardtest_sim.yal` | `hardtest_sim.yalp` | `test_c.c` | `--lalr` |
| `hardtest.yal` | `hardtest.yalp` | `hardtest.py` | `--lalr` |
| `expr.yal` | `expr_left_recursive.yalp` | `expr.txt` | `--lalr` o `--slr` |
| `expr.yal` | `expr_ll1.yalp` | `expr.txt` | `--ll1` |
