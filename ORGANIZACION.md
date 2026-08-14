
# Proyecto de Generador de Analizadores Léxicos a partir de especificaciones en YALex.

## Fases del proyecto y ubicación en la estructura

---

### Fase 0. Coordinación general

**Ubicación:** `src/main.rs`

### Qué hace

Es el punto de entrada del proyecto.

### Qué debería hacer

* leer argumentos de línea de comandos
* recibir la ruta del archivo `.yal`
* llamar las fases en orden
* manejar errores
* decidir dónde guardar resultados

### Entrada

* ruta del archivo `.yal`

### Salida

* coordinación del pipeline completo

---

### Fase 1. Leer y entender la especificación YALex

**Ubicación:** `src/lexico/spec/parser.rs`

### Qué hace

Lee el archivo `.yal` y separa sus partes.

### Qué debería hacer

* leer `header`
* leer definiciones `let`
* leer sección `rule`
* leer acciones asociadas
* capturar prioridad por orden
* guardar trailer o código auxiliar si existe

### Entrada

* texto completo del archivo `.yal`

### Salida

* una estructura interna tipo `SpecIR`

### Resultado esperado

El archivo ya no se ve como texto bruto, sino como datos organizados.

---

### Fase 2. Representar internamente la especificación

**Ubicación:** `src/lexico/spec/ast.rs`

### Qué hace

Define las estructuras de datos para guardar la especificación.

### Qué debería contener

* `SpecIR`
* `Definition`
* `Rule`
* prioridad de reglas
* acción asociada

### Entrada

* datos interpretados por el parser

### Salida

* representación interna limpia y usable por las siguientes fases


---

### Fase 3. Expandir definiciones y macros

**Ubicación:** `src/lexico/spec/expand.rs`

### Qué hace

Reemplaza referencias como `{DIGIT}` por su definición real.

### Qué debería hacer

* buscar definiciones declaradas con `let`
* sustituir referencias dentro de reglas
* detectar referencias faltantes
* detectar ciclos si una definición depende de otra indefinidamente

### Entrada

* `SpecIR`

### Salida

* reglas con regex ya expandidas

### Idea simple

Transforma expresiones abreviadas en expresiones completas.

---

### Fase 4. Convertir regex a árbol

**Ubicación:** `src/lexico/regex/parser.rs`

### Qué hace

Toma una expresión regular expandida y la convierte en una estructura de árbol.

### Qué debería hacer

* reconocer unión `|`
* reconocer concatenación
* reconocer `*`, `+`, `?`
* reconocer paréntesis
* reconocer clases de caracteres
* respetar precedencia

### Entrada

* regex expandida en texto

### Salida

* AST de regex

---

### Fase 5. Definir la estructura del AST de regex

**Ubicación:** `src/lexico/regex/ast.rs`

### Qué hace

Define los nodos que puede tener el árbol.

### Qué debería contener

* literal
* unión
* concatenación
* estrella
* plus
* opcional
* clase de caracteres
* vacío si se necesita

### Entrada

* no recibe datos directamente; define la forma del árbol

### Salida

* tipos y estructuras usadas por `regex/parser.rs`

### Idea simple

Es la plantilla de cómo se verá el árbol de expresiones regulares.

---

### Fase 6. Graficar el árbol

**Ubicación:** `src/lexico/graph/dot.rs`

### Qué hace

Convierte el AST a un formato graficable.

### Qué debería hacer

* recorrer el AST
* generar nodos y conexiones
* exportar un archivo `.dot`
* opcionalmente permitir luego generar `.png`

### Entrada

* AST de regex

### Salida

* archivo de grafo

---

### Fase 7. Construcción de AFN

**Ubicación:** `src/lexico/automata/nfa.rs`

### Qué hace

Convierte cada AST en un AFN usando Thompson.

### Qué debería hacer

* construir AFN para literal
* construir AFN para unión
* construir AFN para concatenación
* construir AFN para `*`, `+`, `?`
* marcar estados de aceptación por token
* manejar prioridad de reglas

### Entrada

* AST de cada regex

### Salida

* AFN por regla
* o AFN global si ya se combinan aquí


---

### Fase 8. Unir todos los AFN y convertir a AFD

**Ubicación:** `src/lexico/automata/subset.rs`

### Qué hace

Construye el AFD a partir del AFN usando el algoritmo de subconjuntos.

### Qué debería hacer

* calcular `epsilon-closure`
* calcular `move`
* construir estados del AFD
* definir estados de aceptación
* resolver prioridad si varios tokens coinciden

### Entrada

* AFN global

### Salida

* AFD


---

### Fase 9. Representar el AFD

**Ubicación:** `src/lexico/automata/dfa.rs`

### Qué hace

Guarda la estructura del AFD.

### Qué debería contener

* estados
* transiciones
* estado inicial
* estados de aceptación
* token aceptado por estado

### Entrada

* datos construidos por el algoritmo de subconjuntos

### Salida

* AFD bien estructurado


---

### Fase 10. Minimización del AFD

**Ubicación:** `src/lexico/automata/minimize.rs`

### Qué hace

Reduce el AFD sin cambiar el lenguaje reconocido.

### Qué debería hacer

* agrupar estados equivalentes
* producir un AFD más pequeño
* conservar aceptación y prioridad correctas

### Entrada

* AFD

### Salida

* AFD minimizado


---

### Fase 11. Construcción de tabla de transiciones

**Ubicación:** `src/lexico/table/transition_table.rs`

### Qué hace

Transforma el AFD en una tabla fácil de usar durante la simulación.

### Qué debería construir

* `delta[state][symbol]`
* `accept[state]`
* `start_state`

### Entrada

* AFD minimizado

### Salida

* tabla de transición

### Idea simple

En vez de recorrer estructuras complejas, el lexer luego solo consulta la tabla.

---

### Fase 12. Simulación del analizador léxico

**Ubicación:** `src/lexico/runtime/simulator.rs`

### Qué hace

Usa la tabla para analizar texto real.

### Qué debería hacer

* leer la entrada carácter por carácter
* moverse por la tabla
* recordar la última aceptación válida
* aplicar maximal munch
* romper empate por prioridad de regla
* emitir tokens
* reportar error cuando no haya coincidencia

### Entrada

* tabla de transición
* texto de entrada

### Salida

* secuencia de tokens
* errores léxicos


---

### Fase 13. Generación de código del lexer

**Ubicación:** `src/lexico/codegen/rust_codegen.rs`

### Qué hace

Genera el archivo fuente final del analizador léxico.

### Qué debería hacer

* escribir estructuras necesarias
* escribir la tabla de transición
* escribir la lógica de `next_token`
* insertar acciones de usuario
* guardar el archivo generado, por ejemplo `generated/src/lexer.rs`

### Entrada

* tabla de transición
* reglas
* acciones
* código auxiliar

### Salida

* archivo fuente del lexer generado


---

### Fase 14. Manejo de errores

**Ubicación:** `src/error.rs`

### Qué hace

Centraliza errores del proyecto.

### Qué debería manejar

* formato inválido del `.yal`
* definición inexistente
* regex mal formada
* transición inválida
* error de generación de archivo


---

## Fases futuras (no implementadas)

Todo lo de arriba (Fases 0–14) cubre léxico y sintáctico. El libro del
dragón sigue con tres fases más que este proyecto todavía no toca. Se
documentan aquí para que el roadmap quede escrito en algún lado — antes de
esta sección, ningún doc del repo mencionaba semántica, código intermedio
ni código objetivo.

**Restricción de diseño que condiciona las tres**: el generador tiene que
seguir siendo agnóstico a la gramática. Cada práctica entrega un
`.yal`/`.yalp`/`.txt` distintos — no hay un lenguaje fijo para el que
hardcodear reglas semánticas, así que las tres fases futuras tienen que
salir de lo que declara la gramática *dada*, dinámicamente, igual que hoy
el lexer no sabe de antemano qué tokens va a tokenizar.

---

### Fase 15. Análisis semántico

**Ubicación:** `src/semantico/` (esqueleto creado, sin lógica)

Tabla de símbolos, alcance y chequeo de tipos. Consume el `ParseNode` que ya
construyen `LRParser::parse_tree`/`parse_recovering_with_pos`
(`src/sintactico/runtime/parser_lr.rs`) y `LL1Parser::parse_tree`
(`src/sintactico/runtime/ll1.rs`) — ambos ya anotan cada hoja con
`line`/`col` (`ParseNode`/`ParseToken` en
`src/sintactico/runtime/parse_tree.rs`), así que los errores semánticos
("variable X no declarada en línea N") pueden ubicarse sin trabajo extra.

**Bloqueos a resolver antes de implementarla:**

* **El pipeline HTTP nunca construye ese árbol.** `api::pipeline::
  build_pipeline_response` descarta línea/columna de cada token al derivar
  `token_kinds`, y `api::sintactico::build_parse_response` usa
  `parse_with_trace_lr` — una reimplementación aparte del shift-reduce que
  solo emite un trace JSON para el stepper del IDE, nunca un `ParseNode`.
  Hoy el árbol solo lo consumen los binarios de CLI (`src/bin/test_*.rs`).
* **Deuda de shift-reduce duplicado.** Solo para LR ya hay 4 variantes del
  mismo driver en `parser_lr.rs` (`parse`, `parse_tree`, `parse_recovering`/
  `parse_recovering_with_pos`) más una 5ª en `api::sintactico::
  parse_with_trace_lr`. Conectar semántica al pipeline HTTP sin antes
  consolidarlas sumaría una 6ª. Riesgo de tocar esto: la variante JSON
  alimenta directamente la UI de "PASO" del frontend, así que la
  consolidación tiene que preservar ese contrato byte a byte.
* **Acciones semánticas dinámicas por producción (diseño, no implementado).**
  Para que `src/semantico/` sirva con cualquier gramática dada, el `.yalp`
  necesitará eventualmente sintaxis de acciones al estilo yacc, igual que
  `.yal` ya tiene `{ action_code }` por regla:
  ```
  E : E PLUS T  { $$ = $1 + $3 }
    | T          { $$ = $1 }
    ;
  ```
  Para keyear cada acción a su producción no hace falta un id nuevo en
  `Production` (cambiar `Production.bodies: Vec<Vec<Symbol>>` sería
  invasivo — se itera en `first.rs`, `follow.rs`, `lr0.rs`, `lr1.rs`,
  `ll1.rs`, `tablas.rs`, `api/sintactico.rs`): el orden de iteración que ya
  usa `grammar_to_prods` (`src/api/sintactico.rs`) para numerar
  producciones en la respuesta JSON es determinista y sirve como id
  implícito.
* **Sin tipo de diagnóstico compartido.** `src/error.rs` (`LexerGenError`)
  es exclusivo del lexer (4 variantes, todas `String`) y `sintactico` no lo
  usa — todo devuelve `Result<_, String>`. Diseñar ya una forma con *spans*
  compartida sería especular sin un caso de uso real que la valide.

---

### Fase 16. Código intermedio

**Ubicación:** `src/intermedio/` (esqueleto creado, sin lógica)

Código de tres direcciones (TAC) / cuádruplos, a partir de la salida de
`semantico`. Sin forma fija asumida más allá de la estructura genérica de
TAC — depende de qué construya la Fase 15 para la gramática dada.

---

### Fase 17. Código objetivo

**Ubicación:** `src/codigo_objetivo/` (esqueleto creado, sin lógica)

Generación de código ensamblador/objetivo a partir de `intermedio`. El
nombre evita chocar con dos módulos que también se llaman "codegen" pero
son otra cosa: `lexico::codegen::rust_codegen` (emite el *lexer* standalone,
Fase 13 ya implementada) y `api::codegen` (el handler HTTP de esa fase,
`/api/codegen`).

---

## 5. Flujo completo del proyecto

```text
archivo .yal
   ↓
lexico/spec/parser.rs
   ↓
lexico/spec/ast.rs
   ↓
lexico/spec/expand.rs
   ↓
lexico/regex/parser.rs + lexico/regex/ast.rs
   ↓
lexico/graph/dot.rs
   ↓
lexico/automata/nfa.rs
   ↓
lexico/automata/subset.rs + lexico/automata/dfa.rs
   ↓
lexico/automata/minimize.rs
   ↓
lexico/table/transition_table.rs
   ↓
lexico/runtime/simulator.rs
   ↓
lexico/codegen/rust_codegen.rs
   ↓
lexer generado
   ↓
texto de entrada
   ↓
tokens / errores léxicos
```
---
