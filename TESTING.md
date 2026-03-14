# Guía de pruebas del generador de analizadores léxicos

Este documento describe cómo probar cada fase del pipeline (Fase 6, 11, 12 y 13) y el flujo completo. Se usa como referencia [ORGANIZACION.md](ORGANIZACION.md) y la guía de implementación (Compiler.md) para los contratos de entrada/salida.

---

## Requisitos

- **Rust** estable (`rustc` / `cargo`) instalado.
- **Graphviz** (opcional): para validar y renderizar archivos `.dot` (`dot -Tpng archivo.dot -o archivo.png`).
- Archivos de ejemplo: `examples/basic/lexer.yal`, `examples/basic/ejemplo_c.yal`.

> Nota: **no necesitas ninguna herramienta externa tipo YALex**.  
> Todo el pipeline (parser de regex, construcción de NFA/DFA, minimización, simulador y codegen) se ejecuta únicamente con `cargo run` / `cargo test` sobre este proyecto.

---

## Fase 6 — `src/graph/dot.rs` (exportar a DOT)

**Archivo:** `src/graph/dot.rs`  
**Parte del lexer:** Exportación/visualización (Graphviz DOT).

### Entrada

- **AST de regex** (en memoria): raíz `RegexAst` ya parseada (p. ej. desde `regex::parser::parse_regex`).
- **DFA** (en memoria): salida de la Fase 10 (DFA minimizado), con `states` y `start_state`.

### Cómo probar

1. **Con el pipeline completo**  
   Ejecutar:
   ```bash
   cargo run -- examples/basic/lexer.yal
   ```
   Se genera `graphs/dfa.dot`. Comprobar que el archivo existe.

2. **Validar el DOT**  
   Si tienes Graphviz instalado:
   ```bash
   dot -Tpng graphs/dfa.dot -o graphs/dfa.png
   ```
   No debe dar error; puedes abrir `graphs/dfa.png` para inspección visual.

3. **AST (opcional)**  
   Desde código o tests: construir un `RegexAst` (p. ej. `Concat(Literal('a'), Star(Literal('b')))`) y llamar `graph::dot::write_ast_dot("graphs/ast.dot", &ast)`. Verificar que `graphs/ast.dot` contiene `digraph AST` y nodos con labels correctos.

### Salida esperada

- Archivo `.dot` con:
  - `digraph DFA { ... }` o `digraph AST { ... }`
  - Nodos y aristas con `label`
- Sin errores de escritura; Graphviz puede interpretar el archivo sin errores.

---

## Fase 11 — `src/table/transition_table.rs` (tabla de transición)

**Archivo:** `src/table/transition_table.rs`  
**Parte del lexer:** Construcción de la tabla de transición (δ).

### Entrada

- **DFA minimizado** (salida de Fase 10): `Dfa` con `states: HashMap<usize, DfaState>`, `start_state`, y cada `DfaState` con `transitions: HashMap<char, usize>` y `accept_action: Option<(usize, String)>`.

### Cómo probar

1. **Test unitario con DFA dummy**  
   En `src/table/transition_table.rs` hay (o puedes añadir) un test que construye un DFA con 2 estados: estado 0 (inicio) con transiciones para `'0'..'9'` al estado 1; estado 1 de aceptación con acción `"NUM"`. Llamar `build(&dfa)` y comprobar:
   - `tt.start == 0`, `tt.n_states == 2`
   - `tt.accept[0].is_none()`, `tt.accept[1] == Some("NUM")`
   - `tt.next(0, '5') != DEAD`, `tt.next(0, 'a') == DEAD`

2. **Pipeline completo**  
   Tras `cargo run -- examples/basic/lexer.yal`, la tabla se construye en memoria. Puedes añadir un `print_table(&table)` temporal en `main.rs` para inspeccionar filas/columnas en consola.

### Salida esperada

- `TransitionTable` con `delta`, `accept`, `start`, `n_states`, `alphabet` coherentes.
- `next(state, c)` devuelve el siguiente estado o `DEAD`; `is_accepting(state)` y `token_at(state)` correctos para estados de aceptación.

---

## Fase 12 — `src/runtime/simulator.rs` (simulador / maximal munch)

**Archivo:** `src/runtime/simulator.rs`  
**Parte del lexer:** Simulación del lexer (maximal munch).

### Entrada

- **TransitionTable** (salida de Fase 11).
- **Texto a analizar**: `&str` (p. ej. `"42 abc"` o contenido de un archivo).

### Cómo probar

1. **Test unitario con tabla dummy**  
   Construir una tabla (desde un DFA dummy con reglas para dígitos y letras) y un `Simulator::new(&table, "42 abc")`. Llamar `tokenize()` y comprobar:
   - Número de tokens y que el primer token tiene `lexeme == "42"` y `kind` coherente con la acción.
   - Segundo token con `lexeme == "abc"`.
   - Sin errores en la lista de errores para esa entrada.

2. **Pipeline completo (sin usar lexer generado)**  
   Ejecutar el pipeline con una especificación `.yal` de ejemplo:
   ```bash
   cargo run -- examples/basic/lexer.yal
   ```
   Al final de `main.rs` se hace una prueba del **simulador en memoria** con una cadena fija (por defecto `"42 abc"`):
   - Se construye el DFA minimizado.
   - Se construye la `TransitionTable`.
   - Se instancia `Simulator::new(&table, "42 abc")` y se llama a `tokenize()`.
   Revisar la salida en consola: se listan los tokens (kind, lexeme, line, col) y el número de errores.
   - Debes ver un token numérico para `"42"` y un identificador para `"abc"`, **sin errores léxicos**.

3. **Carácter inválido**  
   Probar con una entrada que contenga un carácter no reconocido por ninguna regla; debe aparecer un `LexResult::Error` y el mensaje correspondiente en la lista de errores.

### Salida esperada

- `(Vec<Token>, Vec<String>)`: tokens con `kind`, `lexeme`, `line`, `col` correctos; lista de errores para caracteres no reconocidos.
- Maximal munch: el prefijo más largo que acepta alguna regla se consume en un solo token.

---

## Fase 13 — `src/codegen/rust_codegen.rs` (generar `lexer.rs`)

**Archivo:** `src/codegen/rust_codegen.rs`  
**Parte del lexer:** Generación de código (emitir `generated/lexer.rs`).

### Entrada

- **TransitionTable** (salida de Fase 11).
- **Reglas expandidas** (`Vec<ExpandedRule>`).
- **Header/Trailer** (opcionales): bloques de código del `.yal` para insertar al inicio y al final del archivo generado.

### Cómo probar

1. **Pipeline completo**  
   Ejecutar:
   ```bash
   cargo run -- examples/basic/lexer.yal
   ```
   Comprobar que existe `generated/lexer.rs` y que contiene:
   - `const DEAD`, `N_STATES`, `static DELTA`, `static ACCEPT`
   - `struct Token`, `fn next_token`, `fn tokenize`
   - Si el `.yal` tiene header/trailer, deben aparecer en el archivo.

2. **Compilación del generado**  
   El archivo generado está pensado para ser usado como módulo o incluido en un crate. Para comprobar que es Rust válido puedes:
   - Incluirlo en un test de integración que lo compile como parte del crate, o
   - Crear un crate mínimo que lo importe y llame a `tokenize("...")` y verificar que la secuencia de tokens coincide con la del simulador en memoria (Fase 12) para la misma entrada.

### Salida esperada

- Archivo `generated/lexer.rs` que:
  - Compila sin errores de sintaxis.
  - Expone `Token`, `next_token`, `tokenize` con la misma semántica que el simulador (maximal munch, mismas posiciones line/col).

---

## Pipeline completo

Pasos recomendados para una prueba de extremo a extremo:

1. **Generar (todo el pipeline interno)**  
   ```bash
   cargo run -- examples/basic/lexer.yal
   ```

2. **Comprobar artefactos del pipeline (opcional)**  
   - `graphs/dfa.dot` existe y, con Graphviz, `dot -Tpng graphs/dfa.dot -o graphs/dfa.png` no falla.
   - `generated/lexer.rs` existe y contiene las tablas y funciones descritas arriba (no es obligatorio usar este archivo para las pruebas básicas; basta con el simulador en memoria).

3. **Probar simulador en memoria (recomendado)**  
   La salida en consola muestra una prueba con una cadena fija (por defecto `"42 abc"`); revisar que los tokens y el número de errores son los esperados.

4. **Correr todos los tests automáticos**  
   ```bash
   cargo test
   ```
   Esto ejecuta los tests unitarios de:
   - Construcción de `TransitionTable` dummy (Fase 11).
   - Simulador `Simulator` con casos simples como `"42"` y `"42x"` (Fase 12).

4. **Opcional: usar el lexer generado**  
   Integrar `generated/lexer.rs` en un programa o test que llame a `tokenize(input)` y comparar con la salida del simulador para la misma `input`.

---

## Referencias

- [ORGANIZACION.md](ORGANIZACION.md): fases del proyecto, entradas/salidas y ejemplos visuales por fase.
- Guía de implementación (Compiler.md): detalles de algoritmos y estructuras del Dragon Book.
