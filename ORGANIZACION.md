
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

**Ubicación:** `src/semantico/` — **implementada y conectada al pipeline HTTP.**

Tabla de símbolos, alcance y chequeo de tipos, sobre el `ParseNode` real que
construyen `LRParser::parse_tree`/`parse_recovering_with_pos`
(`src/sintactico/runtime/parser_lr.rs`) y `LL1Parser::parse_tree`
(`src/sintactico/runtime/ll1.rs`) — ambos anotan cada hoja con `line`/`col`
(`ParseNode`/`ParseToken` en `src/sintactico/runtime/parse_tree.rs`), así que
cada diagnóstico sale ubicado.

**Submódulos** (ver [`ARQUITECTURA.md`](ARQUITECTURA.md) para el detalle
completo de esta fase — mapa de módulos, catálogo de directivas y de
diagnósticos, estructuras de datos y límites conocidos):

| submódulo | responsabilidad |
|---|---|
| `scopes` / `symbols` | tabla de símbolos con entornos anidados, shadowing, `dump()`, foto de cada ámbito al cerrarse (`ScopeCollector`) |
| `types` | enum de tipos, tabla de compatibilidad y coerciones |
| `visitor` | el recorrido genérico sobre `ParseNode` (`Visitor`/`walk`) |
| `spec` | la config declarativa por gramática (`SemanticSpec`) |
| `analyzer` | el walker genérico; no menciona ninguna producción concreta |
| `errors` | `Diagnostic` + códigos `S001`–`S034` y `W001` + `ErrorCollector` |
| `classes` | miembros con `.` (con herencia), `this`, constructor, literal de struct |
| `functions` | firmas, argumentos (`check_arguments`) y `return` |
| `closures` | captura de variables libres del entorno de definición |
| `flow` | condiciones booleanas y contexto de `break`/`continue` |
| `operators` | expresiones binarias/unarias: lógicas, comparaciones y sentido semántico del operando |
| `collections` | las cuatro colecciones: arreglo, conjunto, mapa y tupla |
| `duplicates` | declaraciones repetidas y símbolos declarados pero nunca leídos |
| `deadcode` | instrucciones inalcanzables tras `return`/`break`/`continue` |

**Agnosticismo a la gramática.** Nada de esto está atado a Compiscript: toda
la especificidad llega por directivas en el `.yalp` — `%ident`, `%declare`,
`%scope`, `%type_of`, `%type_token`, `%init_of`, `%immutable`, `%assign`,
`%arith`, `%this`, `%member_access`, `%new`, `%call`, `%arg_list_symbol`,
`%constructor`, `%return`, `%struct_literal`, `%field_list_symbol`,
`%field_init`, `%condition`, `%loop`, `%break`, `%continue`, `%logic`,
`%compare`, `%unary`, `%array_type`, `%array_literal`, `%index`, `%switch`,
`%case`, `%foreach`, `%stmt_list`, `%map_type`, `%map_literal`, `%map_entry`,
`%map_list_symbol`, `%set_type`, `%set_literal`, `%tuple_type`,
`%tuple_literal`, `%warn_unused`. Se parsean en un único lugar
(`Grammar::parse_tokens_section`) y se traducen a `SemanticSpec` en
`SemanticSpec::from_grammar`. La prueba empírica vive en
dos gramáticas de prueba independientes, que producen exactamente los mismos
códigos de diagnóstico que Compiscript:

* `examples/grammar/objetos_es.yalp` — todos los NOMBRES distintos
  (`tests/gramatica_agnostica_tests.rs`).
* `examples/grammar/pascalito.yalp` — además la FORMA distinta: sin llaves
  (bloques `is ... end`), asignación con `:=`, literal de registro con
  corchetes, comentarios con `--` (`tests/pascalito_tests.rs`).

**Límite conocido de las directivas:** el lado derecho de `%type_token`
pertenece a un vocabulario FIJO (`integer`, `float`, `string`, `bool`,
`void`) porque nombra una variante del enum `Type`. Escribir otra cosa
compila y parsea igual, pero el tipo cae en `Unknown` y los chequeos se
desactivan en silencio — ver `examples/grammar/objetos_es.README.md`.

**Bloqueos históricos, ya resueltos:** el pipeline HTTP sí construye el árbol
y sí corre el análisis (`api::pipeline`, gated a `mode != "ll1"` y a que el
`.yalp` traiga `%ident`), y `errors::Diagnostic` es el tipo de diagnóstico
compartido que faltaba. La **deuda de shift-reduce duplicado ya está saldada**: el bucle vive
una sola vez en `sintactico::runtime::driver`, y los cuatro consumidores
—traza, árbol, recuperación en modo pánico y traza JSON del IDE— son
observadores (`ParseObserver`), mismo patrón que `semantico::visitor` una capa
más abajo. (La doc afirmaba "5 variantes"; al contarlas resultaron ser 4 — el
quinto `let top` era el desapilado del modo pánico, no un motor.) También se
unificó el pipeline `.yal`→tabla, que estaba triplicado, en
`lexico::pipeline::build_all`, y la construcción de la tabla LR, que se hacía
DOS veces por petición del IDE. Las **acciones semánticas por
producción al estilo yacc** siguen sin implementarse — las directivas
declarativas cubrieron el caso de uso sin necesitarlas.

**Estructuras definidas por el usuario.** `struct Nombre { campo: tipo; ... }`
declara un tipo registro; se usa como anotación de tipo, se construye con un
literal de campos nombrados (`Punto { x: 1, y: 2 }`) que valida campo
inexistente, faltante, repetido y mal tipado, y sus campos se acceden con `.`
ya tipados. Reusa la maquinaria de clases; lo único propio es el literal.

**Operadores (`operators/`).** Lógicas (`&& || !`) sobre `bool`, comparaciones
(`== != < <= > >=`) con compatibilidad de operandos —el orden exige numéricos,
la igualdad delega en la tabla de `types`—, y el sentido semántico del
operando: una función o una clase NOMBRADA A SECAS no es un valor (`f * 2` con
`f` función es `S031`). Sin esa última regla el caso pasa desapercibido, porque
el tipo de la hoja `f` es el tipo de RETORNO de `f`. Ver
`src/semantico/operators/README.md`.

**Colecciones (`collections/`).** Cuatro tipos compuestos: arreglo, conjunto,
mapa y tupla. Literales homogéneos (`S032`), índice entero para un arreglo
(`S033`), indexar algo no indexable —un conjunto, entre otros— (`S034`), clave
de mapa con el tipo declarado (`S037`) e índice literal de tupla dentro de rango
(`S038`). Iterar recorre los elementos de un arreglo o conjunto y las claves de
un mapa; una tupla no es iterable (`S036`). Todo se declara con directivas
(`%array_*`, `%map_*`, `%set_*`, `%tuple_*`, `%index`), y como Compiscript solo
tiene arreglos, las otras tres se prueban con `workspace/colecciones.yalp`.

**Duplicados y no usados (`duplicates/`).** Redeclaración en el mismo ámbito
para variables y parámetros (`S001`, conservando la primera declaración) y la
advertencia `W001` para lo declarado pero nunca leído, opt-in con
`%warn_unused`. La batería de 15 casos vive en `workspace/duplicates_casos.txt`
y se ejecuta igual desde el IDE que desde `tests/duplicates_tests.rs`.

**Control de flujo completo.** `if`/`while`/`do-while`/`for`/`foreach`/
`switch`/`try-catch`, con condición booleana obligatoria donde corresponde
(`S025`), `break`/`continue` contra su contexto (`S026`/`S027`) y compatibilidad
de cada `case` con el discriminante del `switch` (`S035`). El `switch` NO exige
un discriminante booleano —se selecciona sobre enteros o cadenas— y admite
`break` pero no `continue`. La variable de un `for`, la de un `foreach` y la de
un `catch` viven en su propio ámbito; la del `foreach` se tipa con el tipo de
elemento del iterable, y iterar algo que no es una colección es `S036`.

**Código muerto (`deadcode/`).** Toda instrucción que siga a un
`return`/`break`/`continue` dentro de la misma secuencia es inalcanzable: se
reporta **W002** una sola vez, sobre la primera, y el recorrido deja de
analizar el resto del bloque para no producir diagnósticos derivados de código
que nunca corre. La detección es conservadora a propósito — ver los límites
conocidos en [`ARQUITECTURA.md`](ARQUITECTURA.md).

**Lo que falta:** detectar "función con tipo declarado que nunca retorna", que
necesita análisis de alcanzabilidad completo. Del `Compiscript.g4` oficial solo
queda sin traducir el operador ternario.

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
