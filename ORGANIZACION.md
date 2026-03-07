
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

**Ubicación:** `src/spec/parser.rs`

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

**Ubicación:** `src/spec/ast.rs`

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

**Ubicación:** `src/spec/expand.rs`

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

**Ubicación:** `src/regex/parser.rs`

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

**Ubicación:** `src/regex/ast.rs`

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

**Ubicación:** `src/graph/dot.rs`

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

**Ubicación:** `src/automata/nfa.rs`

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

**Ubicación:** `src/automata/subset.rs`

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

**Ubicación:** `src/automata/dfa.rs`

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

**Ubicación:** `src/automata/minimize.rs`

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

**Ubicación:** `src/table/transition_table.rs`

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

**Ubicación:** `src/runtime/simulator.rs`

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

**Ubicación:** `src/codegen/rust_codegen.rs`

### Qué hace

Genera el archivo fuente final del analizador léxico.

### Qué debería hacer

* escribir estructuras necesarias
* escribir la tabla de transición
* escribir la lógica de `next_token`
* insertar acciones de usuario
* guardar el archivo generado, por ejemplo `generated/lexer.rs`

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

## 5. Flujo completo del proyecto

```text
archivo .yal
   ↓
spec/parser.rs
   ↓
spec/ast.rs
   ↓
spec/expand.rs
   ↓
regex/parser.rs + regex/ast.rs
   ↓
graph/dot.rs
   ↓
automata/nfa.rs
   ↓
automata/subset.rs + automata/dfa.rs
   ↓
automata/minimize.rs
   ↓
table/transition_table.rs
   ↓
runtime/simulator.rs
   ↓
codegen/rust_codegen.rs
   ↓
lexer generado
   ↓
texto de entrada
   ↓
tokens / errores léxicos
```
---
