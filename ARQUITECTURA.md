# Arquitectura de la fase de análisis semántico

Cómo está construido `src/semantico/`: qué hace cada submódulo, por qué el
analizador no conoce ninguna gramática concreta, qué diagnóstico produce cada
regla y cómo llega todo eso al IDE.

Para el resto del proyecto (léxico, sintáctico, generación de código, fases y
estructura de carpetas) ver [`ORGANIZACION.md`](ORGANIZACION.md); para el
pipeline completo de punta a punta, [`PIPELINE_GUIDE.md`](PIPELINE_GUIDE.md);
para los endpoints HTTP, [`API_REFERENCE.md`](API_REFERENCE.md).

---

## 0. Dónde está cada entregable

Mapa de arranque: qué pide el enunciado, dónde vive y con qué comando se
ejecuta. Todo lo demás de este archivo desarrolla la segunda fila.

| Entregable | Dónde | Cómo se ejecuta |
|---|---|---|
| **Analizador léxico + sintáctico** | `src/lexico/`, `src/sintactico/`; la gramática de Compiscript en `workspace/compiscript.yal` + `workspace/compiscript.yalp` | El IDE, o por CLI: `cargo run --bin test_pipeline -- workspace/compiscript.yalp workspace/compiscript.yal workspace/rubrica.cps --lalr` (ojo al **orden**: `.yalp` antes que `.yal`) |
| **Análisis semántico** | `src/semantico/` (15 submódulos) | Se activa solo, dentro del pipeline, cuando el `.yalp` trae `%ident` y el modo no es LL(1) |
| **Árbol sintáctico con representación visual** | `to_dot_annotated` en `src/sintactico/runtime/parse_tree.rs` | Pestaña **ÁRBOL** del IDE (render con viz.js, botón ↓ PNG) |
| **Tabla de símbolos por entorno** | `SymbolTable::dump()` y `ScopeCollector::to_json()` | Pestaña **SÍMBOLOS** del IDE — ver §6 |
| **Batería de pruebas** | `workspace/casos_semanticos.txt` y los `.cps` de `workspace/` | `cargo test`, o el panel **TEST CASES** del IDE — ver más abajo |
| **IDE funcional** | `frontend/IDE/` | `docker compose up --build` → <http://localhost:4000> — ver [`GUIA_USO.md`](GUIA_USO.md) |
| **Documentación** | Este archivo (fase semántica), [`ORGANIZACION.md`](ORGANIZACION.md) (fases y carpetas), [`PIPELINE_GUIDE.md`](PIPELINE_GUIDE.md) (pipeline de punta a punta), [`API_REFERENCE.md`](API_REFERENCE.md) (índice de funciones), [`GUIA_USO.md`](GUIA_USO.md) (ejecución) | — |

### La batería de pruebas

El enunciado pide **un caso exitoso y uno fallido por cada regla semántica**.
Eso es `workspace/casos_semanticos.txt`: **45 programas completos, uno por
línea**, cada uno un bloque `{ ... }` independiente. Su tabla de referencia
—qué comprueba cada caso y qué código debe salir— está en
[`workspace/casos_semanticos.README.md`](workspace/casos_semanticos.README.md),
que además documenta los códigos sin caso y por qué son inalcanzables.

```powershell
cargo test --test bateria_semantica_tests   # los 45 casos, en LALR(1) y SLR(1)
```

Ese test no se conforma con "no explotó": exige el diagnóstico **exacto** de
cada línea, ni uno de más ni uno de menos, y corre los 45 dos veces (una por
modo). Es la comprobación reproducible de que cada regla está viva.

**Desde el IDE**, para verlos de a uno: levantar el backend, cargar
`workspace/compiscript.yal`, `workspace/compiscript.yalp` y
`workspace/casos_semanticos.txt`, pulsar **RUN** y luego **PARSEAR**, y navegar
el panel **TEST CASES** de la izquierda. Cada línea es un caso clicable y la
pestaña **PROBLEMAS** muestra su diagnóstico. Un `.cps` multilínea, en cambio,
se compila entero: el panel por línea solo aplica a las baterías `.txt`.

#### Las otras baterías

| Archivo | Casos | Qué cubre | Test |
|---|---|---|---|
| `workspace/casos_semanticos.txt` | 45 | Una regla por caso, con su éxito y su fallo | `bateria_semantica_tests.rs` |
| `workspace/duplicates_casos.txt` | 15 | Declaraciones repetidas y símbolos nunca leídos (`S001`/`W001`) | `duplicates_tests.rs` |
| `workspace/colecciones_casos.txt` | 22 | Las cuatro colecciones, sobre `colecciones.yalp` — otra gramática | `colecciones_tests.rs` |

`duplicates_casos.txt` necesita la directiva `%warn_unused`, que
`compiscript.yalp` **no** trae: el test se la antepone en memoria
(`duplicates_tests.rs:35`). Para reproducirlo en el IDE hay que agregar esa
línea al `.yalp` a mano.

#### Programas `.cps` completos

Para demostrar el analizador sobre código de verdad y no sobre casos de una
línea, `workspace/` trae 16 archivos `.cps`. Los pares ok/error son los que más
sirven en una defensa:

| Éxito (cero diagnósticos) | Fallo (códigos esperados) | Tema |
|---|---|---|
| `rubrica.cps` | — | Las características del enunciado, todas juntas |
| `clases_ok.cps` | `clases_errores.cps` | Clases, herencia, constructores |
| `flujo_ok.cps` | `flujo_errores.cps` | Control de flujo completo |
| `ejemplo_closures.cps` | — | Funciones anidadas y capturas |
| `caso1_valido_multidim.cps` | `caso2_heterogeneo.cps`, `caso3_indice_no_entero.cps`, `caso4_no_indexable.cps` | Listas e índices |
| — | `codigo_muerto.cps`, `codigo_muerto_corte.cps` | Código inalcanzable |

#### La suite completa

```powershell
cargo test                    # todo: 170 unitarios + 145 de integración
cargo test --lib semantico    # solo los unitarios de esta fase
```

Detalle de entorno en Windows: un test de `codegen_tests.rs` compila y ejecuta
un binario, y el Control de aplicaciones puede bloquearlo con el error 4551
(`una directiva de Control de aplicaciones bloqueó este archivo`). Si pasa,
borrar el `.exe` de `target/debug/deps/` fuerza un re-enlace con otro hash y
vuelve a correr. Es del entorno, no del código.

El desglose de qué cubre cada archivo de test está en la §9.

---

## 1. Mapa de módulos

`src/semantico/` son 15 submódulos con una responsabilidad cada uno. Los tres
primeros son la infraestructura; el resto son familias de reglas.

| Módulo | Responsabilidad |
|---|---|
| `scopes/` | La **mecánica** de la pila de entornos: `ScopeStack` con `enter`/`exit`, `ScopeKind::{Global,Function,Class,Struct,Block}`. Sin política: no sabe qué está permitido. Incluye `ScopeCollector`, que fotografía cada ámbito al cerrarse. |
| `symbols/` | La **política** encima de esa pila: `SymbolTable` con `declare`/`lookup` de adentro hacia afuera, shadowing, redeclaración, tipado de declaraciones y asignaciones, y `dump()`. |
| `types/` | El `enum Type` y la **tabla de compatibilidad**: reglas aritméticas (`+ - * /` sobre `integer`/`float`) y de asignación, con la única coerción implícita `integer → float`. Punto único de toda pregunta "¿estos dos tipos son compatibles?". |
| `visitor/` | El recorrido genérico sobre `ParseNode`: trait `Visitor` (`enter`/`exit`) y `walk`. Ninguna fase reimplementa su propia recursión. |
| `spec/` | El `SemanticSpec`: la configuración declarativa que traduce las directivas de un `.yalp` concreto a reglas que el walker entiende. Es el **único** lugar con conocimiento de una gramática particular, y ese conocimiento viene del archivo, no del código. |
| `analyzer/` | El walker: `impl Visitor for Analyzer`. Declara, abre y cierra ámbitos, y llama a las familias de reglas en el momento correcto del recorrido. No menciona ningún nombre de producción. |
| `errors/` | `Diagnostic` (código, mensaje, línea, columna, severidad, `ErrorKind`) y `ErrorCollector`, que acumula sin detenerse en el primer error. |
| `classes/` | Resolución de miembros con `.` **subiendo la cadena de herencia**, tipo estático de una subexpresión simple, `this`, validación de `new Clase(args)` contra el constructor —que también se busca por la cadena de herencia— y literales de struct. |
| `functions/` | Comprobación de argumentos contra una firma (`check_arguments`, la única implementación de esa regla — `classes` la reusa para constructores y métodos) y validación de `return` contra el tipo declarado vía `FunctionContext`. |
| `closures/` | Acumula qué función anidada captura qué variables libres de su entorno de definición. Modela el resultado; la detección vive en el `Analyzer` usando `lookup_with_scope`. |
| `operators/` | Lo que la tabla aritmética no cubre: lógicos (`&& \|\| !`), comparaciones (`== != < <= > >=`), unarios, y el "sentido semántico" de un operando (una función o una clase nombradas a secas no son valores). |
| `flow/` | Condiciones booleanas obligatorias y la pila de contexto bucle/función que hace que `break`/`continue` solo valgan dentro de un bucle. |
| `collections/` | Las cuatro colecciones: arreglo, conjunto, mapa y tupla. Homogeneidad de los literales, validación del subíndice según el tipo de la base, y lo multidimensional gratis por anidamiento. |
| `duplicates/` | Declaraciones repetidas (variables **y** parámetros) y símbolos declarados pero nunca leídos. |
| `deadcode/` | Instrucciones inalcanzables tras una sentencia terminal, y el corte de evaluación del resto del bloque. |

---

## 2. La restricción de diseño: agnóstico a la gramática

Este proyecto es un **generador**, no el compilador de un lenguaje fijo. Recibe
cualquier `.yal`/`.yalp` y tiene que analizarlo. Por eso `analyzer::walk` nunca
escribe `if node.symbol == "class_decl"`: toda la especificidad entra por
directivas del propio `.yalp`, que `SemanticSpec::from_grammar` traduce.

La prueba de que funciona: las mismas reglas corren sobre **tres gramáticas
distintas de verdad** — Compiscript (`workspace/compiscript.yalp`), Pascalito
(`tests/pascalito_tests.rs`) y una alterna con nombres de producción totalmente
diferentes (`tests/gramatica_agnostica_tests.rs`), y las tres producen los
mismos códigos de diagnóstico.

### Catálogo de directivas

Todas van **antes** del `%%`. Las reconoce `Grammar::parse_tokens_section`
(`src/sintactico/gramatica/grammar.rs`) y las traduce
`SemanticSpec::from_grammar` (`src/semantico/spec/mod.rs`). Sin `%ident` no hay
análisis semántico en absoluto: la gramática sigue compilando y parseando igual.

**Ámbito y declaraciones**

| Directiva | Forma | Qué configura |
|---|---|---|
| `%ident` | `%ident ID` | Qué token es un identificador. Toda hoja con ese símbolo que no fue consumida como nombre de una declaración se trata como un **uso** y se busca con `lookup`. |
| `%declare` | `%declare <producción> <kind>` | Que esa producción declara un símbolo. `kind`: `variable`, `parameter`, `function`, `class`, `struct`. |
| `%scope` | `%scope <producción> <kind>` | Que esa producción abre un ámbito mientras se recorren sus hijos. `kind`: `global`, `function`, `class`, `struct`, `block`. |

**Tipos**

| Directiva | Forma | Qué configura |
|---|---|---|
| `%type_of` | `%type_of var_decl tipo` | Cuál hijo de esa producción es el nodo de tipo (por símbolo, no por índice). |
| `%type_token` | `%type_token INT_T integer` | Qué `Type` representa cada terminal de tipo o literal. |
| `%init_of` | `%init_of var_decl expr` | Cuál hijo es el inicializador, para validarlo contra el tipo declarado (o inferir el tipo si no se declaró). |
| `%immutable` | `%immutable const_decl` | Que esa declaración es inmutable: exige inicializador y rechaza asignaciones posteriores. |
| `%assign` | `%assign assign_stmt 0 2` | Producción de asignación, con el índice del destino y el del valor. |
| `%arith` | `%arith PLUS add` | Qué token es cada operador aritmético. |

**Operadores** (por token, reconocidos por **forma** del nodo — tres hijos con
el operador en el medio, o dos con el operador adelante — para no tener que
enumerar `or_expr`/`and_expr`/`equality_expr`/…)

| Directiva | Forma |
|---|---|
| `%logic` | `%logic AND and`, `%logic OR or` |
| `%compare` | `%compare EQ eq`, `%compare LT lt`, … |
| `%unary` | `%unary NOT not`, `%unary MINUS negate` |

**Clases y objetos**

| Directiva | Forma | Qué configura |
|---|---|---|
| `%this` | `%this THIS` | Token del receptor. Se autodeclara al entrar a un método (una función anidada directamente en un ámbito de clase). |
| `%member_access` | `%member_access primary DOT` | Producción y token del acceso a miembro. |
| `%new` | `%new atom NEW 1 3` | Producción, token, índice del nombre de clase e índice de la lista de argumentos. |
| `%call` | `%call primary LPAREN 0 2` | Producción, token, índice del invocado e índice de los argumentos. |
| `%arg_list_symbol` | `%arg_list_symbol args` | Símbolo de la lista de argumentos, para aplanarla. |
| `%constructor` | `%constructor constructor` | Nombre convencional del método que actúa como constructor (estilo JS/TS, igual que `Compiscript.g4`). La firma se busca **subiendo la cadena de herencia**, con el propio ganando sobre el heredado; si se agota la cadena, la clase tiene un constructor implícito de aridad 0. |

**Funciones y control de flujo**

| Directiva | Forma | Qué configura |
|---|---|---|
| `%return` | `%return return_stmt expr` | Producción de retorno y el hijo que lleva el valor, **por símbolo**: `return_stmt: RETURN expr \| RETURN` tiene dos alternativas de distinto largo, y esa ausencia es justo la señal de "retorno sin valor". |
| `%condition` | `%condition if_stmt expr`, `%condition for_stmt 4` | Qué nodo lleva la condición que debe ser booleana. Por símbolo o por índice: en un `for` la condición es el **segundo** hijo `expr`, y buscar por símbolo devolvería el inicializador. |
| `%loop` / `%break` / `%continue` | `%loop while_stmt` | Qué produce un contexto de bucle y cuáles son los saltos. |
| `%switch` | `%switch switch_stmt 2` | Producción del `switch` y su discriminante. **No** se exige booleano: se valida que cada `%case` sea compatible con él. Abre además un contexto que admite `break` pero no `continue`. |
| `%case` | `%case switch_case 1` | Producción de una rama `case` y el valor con el que compara. La rama `default` se declara como producción aparte porque no lleva valor. |
| `%foreach` | `%foreach foreach_stmt 2 4` | Producción del bucle, índice de la variable de iteración e índice del iterable. La variable se declara **dentro** del ámbito del bucle, con el tipo de elemento del iterable. |

**Structs y listas**

| Directiva | Forma |
|---|---|
| `%struct_literal` | `%struct_literal atom LBRACE 0 2` |
| `%field_list_symbol` / `%field_init` | `%field_init field_init 0 2` |
| `%array_type` | `%array_type LBRACKET` |
| `%array_literal` | `%array_literal atom LBRACKET 1` |
| `%index` | `%index primary LBRACKET 0 2` |
| `%map_type` | `%map_type MAPA 2 4` |
| `%map_literal` / `%map_entry` / `%map_list_symbol` | `%map_entry entrada 0 2` |
| `%set_type` / `%set_literal` | `%set_type CONJ 2` |
| `%tuple_type` / `%tuple_literal` | `%tuple_type TUPLA 2 lista_tipos` |

**Advertencias**

| Directiva | Forma | Qué configura |
|---|---|---|
| `%stmt_list` | `%stmt_list stmt_list` | Qué producción es una secuencia de sentencias. Sobre ella se detecta el código inalcanzable (**W002**). Las sentencias terminales no se declaran aparte: son las de `%return`/`%break`/`%continue`. |
| `%warn_unused` | `%warn_unused` (sin argumentos) | Activa W001 para variables y parámetros nunca leídos. Sin ella, el comportamiento es el de antes de esa regla. |

### Por qué no se usó ANTLR

El enunciado pide implementar el analizador sintáctico *"utilizando ANTLR (u
otra herramienta similar)"*. Este proyecto tomó la segunda opción: en vez de
**usar** un generador de analizadores, **construyó uno**. La gramática oficial
`Compiscript.g4` (175 líneas, en la raíz del repo) sigue siendo la fuente de
verdad; lo que se hizo fue traducir su subconjunto a `workspace/compiscript.yalp`
y alimentarlo a un generador propio.

**Qué se construyó en lugar de instalar ANTLR.** El pipeline completo, en tres
capas que suman unas 14.600 líneas de Rust:

- **`src/lexico/` (~2.600 líneas)** — especificación YALex → árbol de regex →
  AFN por Thompson → AFD por construcción de subconjuntos → minimización →
  tabla de transiciones → simulador con *maximal munch* y desempate por
  prioridad de regla.
- **`src/sintactico/` (~3.500 líneas)** — cálculo de FIRST/FOLLOW → autómata
  LR(1) → fusión LALR por núcleo → tablas ACTION/GOTO con detección de
  conflictos → driver shift-reduce que construye el árbol, con recuperación en
  modo pánico. También LL(1) con eliminación de recursión izquierda y
  factorización.
- **`src/semantico/` (~8.600 líneas)** — lo que documenta el resto de este
  archivo.

**Qué se pierde.** ANTLR es más expresivo: permite escribir código arbitrario
por regla, tiene ALL(*) —que acepta gramáticas que un LALR(1) rechaza—, y trae
generación de código para múltiples lenguajes destino. Nada de eso está acá.
Una gramática que necesite más de un token de anticipación no compila en este
generador, y hay que reescribirla (fue justo el trabajo de traducir el
`switch` y el `for` de la `.g4` a una forma LALR sin conflictos).

**Qué se gana, y por qué era lo pertinente para esta fase.** ANTLR 4 genera
una clase base con **un método por regla de la gramática** —`enterClassDeclaration`,
`exitFunctionDeclaration`…— así que el código semántico queda lleno de nombres
de producciones concretas: es específico de *esa* gramática, y cambiarla obliga
a cambiar el código. El analizador de este proyecto se negó a eso: `analyzer.rs`
no menciona ni una sola producción, y toda la especificidad vive en las 46
directivas declarativas que reconoce el `.yalp`.

La diferencia es medible, no retórica: **las mismas reglas semánticas corren
sin cambios sobre cuatro gramáticas distintas** —Compiscript, Pascalito
(`examples/grammar/pascalito.yalp`, con otra forma sintáctica: bloques
`is…end`, asignación `:=`, comentarios `--`), objetos_es (todos los nombres de
producciones y tokens en español) y la de colecciones— y las cuatro producen
los mismos códigos de diagnóstico. Con Listeners de ANTLR eso habría exigido
cuatro implementaciones.

**Lo que sí se tomó de ANTLR: su arquitectura.** ANTLR 3 era un esquema de
traducción clásico, con acciones incrustadas dentro de la gramática. ANTLR 4
abandonó eso a propósito y pasó a *parsear primero, recorrer después* con
Listeners. Este proyecto hace exactamente lo mismo: el parser LALR construye el
`ParseNode` completo y recién entonces el `Visitor` lo recorre con `enter`/`exit`
—que son, uno a uno, el `enterX`/`exitX` de un Listener de ANTLR—. La diferencia
no está en el diseño del recorrido, sino en que acá la política por producción
es una tabla de datos y no código generado.

---

## 3. Estructuras de datos

### `Symbol` (`symbols/mod.rs`)

```rust
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,          // Variable | Parameter | Function | Class | Struct | Other
    pub line: usize, pub col: usize,
    pub ty: Option<Type>,
    pub mutable: bool,
    pub initialized: bool,
    pub used: bool,                // lo marca una LECTURA real, no una asignación
    pub signature: Option<Signature>,
    pub storage: Option<StorageInfo>,
    pub members: Option<Vec<Symbol>>,
    pub parent: Option<String>,    // clase padre, solo para SymbolKind::Class
}
```

Quién llena qué, y cuándo:

- `name`/`kind`/`line`/`col` — al declarar, es lo único que se sabe en ese momento.
- `ty` — al declarar, si la producción trae `%type_of`; si no, se infiere del inicializador (`%init_of`).
- `signature` — al declarar una función, **antes** de recorrer su cuerpo: por eso una función recursiva se ve a sí misma.
- `members` — solo, cuando `analyzer::walk` **cierra** el ámbito que ese símbolo abrió. Así los campos y métodos de una clase quedan consultables sin volver a recorrer el árbol. No se aplana transitivamente: un local de un bloque anidado dentro de una función no cuelga de la función.
- `parent` — al declarar una clase con `: Padre`. Es la cadena por la que sube `classes::resolve_member`.
- `storage` — **nadie**: es la fase de asignación de almacenamiento, no implementada (ver §7).

### `Type` (`types/mod.rs`)

`Int | Float | Bool | Str | Void | Named(String) | Array(Box<Type>) | Unknown`

`Unknown` no es "sin tipo": es "todavía no lo sabemos", y las reglas lo tratan
como comodín silencioso — nunca se reporta un error derivado de un tipo que no
se pudo resolver, para no inventar diagnósticos en cascada.

`Named` da compatibilidad **nominal** (dos clases distintas nunca son
compatibles aunque tengan los mismos campos). `Array` anidado es lo que hace
que `integer[][]` funcione sin código especial.

### Ámbitos (`scopes/mod.rs`)

`ScopeStack` arranca con el `Global` en la posición 0 y ese nunca se desapila.
`lookup` recorre de adentro hacia afuera y gana el más cercano (shadowing);
`declare` solo mira el ámbito **actual**, así que declarar el mismo nombre en un
ámbito anidado es válido.

---

**Concatenación de textos.** `+` sobre dos `string` da `string`; el resto de
las combinaciones con texto sigue siendo un error (`S015`), incluida
`integer + string`. La regla NO vive en la matriz aritmética: esa matriz la
comparten `+`, `-`, `*` y `/` y su búsqueda ignora el operador, así que una
fila `string, string` allí habría hecho legales también `"a" - "b"` y
`"a" * "b"`. Está como corto-circuito al principio de
`CompatibilityTable::arithmetic`, y `tests/type_system_tests.rs` fija las dos
mitades: que `+` concatene y que los otros tres sigan rechazando.

---

## 4. El recorrido

`visitor::walk` hace pre-order en `enter` y post-order en `exit`. El `Analyzer`
implementa la política:

- **`enter`** — si el nodo es una hoja identificador, es un uso: `lookup_or_err`.
  Si coincide con una `%declare`, declara (con tipo si hay `%type_of`). Si
  coincide con un `%scope`, abre el ámbito. Devuelve un `Flow` que puede excluir
  de la recursión a los hijos ya consumidos (el nombre declarado no debe
  procesarse otra vez como uso).
- **`exit`** — cierra el ámbito que este mismo nodo abrió y cuelga sus símbolos
  como `members` del símbolo que lo abrió. También registra la foto del ámbito
  en el `ScopeCollector`.

Cada nodo visitado empuja un `Frame` que dice si abrió ámbito y si declaró un
nombre, para que `exit` sepa exactamente qué deshacer sin volver a inspeccionar
el árbol.

La regla que sostiene todo: **`enter` declara y abre, `exit` cierra y cuelga.**

---

## 5. Catálogo de diagnósticos

40 códigos: `S001`–`S038` más `W001` y `W002`. Todos los `S###` son errores
(`level: "err"`); los `W###` son advertencias (`level: "warn"`). Fuente:
`src/semantico/errors/mod.rs`.

| Código | `ErrorKind` | Significado | Módulo |
|---|---|---|---|
| S001 | Ambito | Nombre ya declarado en este mismo ámbito | `duplicates` / `symbols` |
| S002 | Ambito | Variable no declarada | `symbols` |
| S003 | Ambito | Intento de cerrar el ámbito global | `scopes` |
| S004 | Tipos | Constante sin inicializador | `symbols` |
| S005 | Tipos | Asignación a una constante | `symbols` |
| S006 | Tipos | Asignación incompatible con el tipo declarado | `symbols` |
| S007 | Clases | Clase desconocida | `classes` |
| S008 | Clases | Clase padre inexistente | `classes` |
| S009 | Clases | `this` fuera del ámbito de una clase | `classes` |
| S010 | Clases | Miembro inexistente (ni propio ni heredado) | `classes` |
| S011 | Clases | Aridad incorrecta en el constructor | `classes` |
| S012 | Clases | Tipo de argumento incorrecto en el constructor | `classes` |
| S013 | Funciones | Aridad incorrecta en una llamada | `functions` |
| S014 | Funciones | Tipo de argumento incorrecto en una llamada | `functions` |
| S015 | Tipos | Operandos inválidos para un operador aritmético | `types` |
| S016 | Funciones | `return` con tipo distinto al declarado | `functions` |
| S017 | Funciones | `return` sin valor en una función tipada | `functions` |
| S018 | Funciones | `return` con valor en un procedimiento | `functions` |
| S019 | Funciones | `return` fuera de una función | `functions` |
| S020 | Funciones | Se invoca algo que no es una función | `classes` / `functions` |
| S021 | Funciones | Falta la firma del invocado | `functions` |
| S022 | Tipos | Campo de struct con tipo incorrecto | `classes` |
| S023 | Tipos | Falta un campo en el literal de struct | `classes` |
| S024 | Tipos | Campo repetido en el literal de struct | `classes` |
| S025 | ControlFlujo | Condición que no es booleana | `flow` |
| S026 | ControlFlujo | `break` fuera de un bucle | `flow` |
| S027 | ControlFlujo | `continue` fuera de un bucle | `flow` |
| S028 | ControlFlujo | Operando no booleano en un operador lógico | `operators` |
| S029 | ControlFlujo | Comparación entre tipos incompatibles | `operators` |
| S030 | ControlFlujo | Operando inválido para un operador unario | `operators` |
| S031 | ControlFlujo | Un nombre de función o clase no es un valor | `operators` |
| S032 | Listas | Elementos heterogéneos en un literal de lista | `collections` |
| S033 | Listas | Índice que no es entero | `collections` |
| S034 | Listas | Se indexa algo que no es un arreglo | `collections` |
| S035 | ControlFlujo | El valor de un `case` no es compatible con el discriminante del `switch` | `flow` |
| S036 | Listas | Se itera con `foreach` sobre algo que no es una colección | `collections` |
| S037 | Listas | Clave de mapa con un tipo incompatible con el declarado | `collections` |
| S038 | Listas | Índice literal fuera del rango de una tupla | `collections` |
| **W001** | Ambito | Variable o parámetro declarado pero nunca leído | `duplicates` |
| **W002** | ControlFlujo | Instrucción inalcanzable tras un `return`/`break`/`continue` | `deadcode` |

Todos llevan línea y columna reales, heredadas de las hojas del `ParseNode` que
el parser ya anota. El `ErrorCollector` **no se detiene en el primero**: reporta
todo lo que puede en una sola pasada, igual que el modo pánico del parser.

---

## 6. Del backend al IDE

`api::build_pipeline_response_named` corre el análisis sobre el árbol real y
llena `ParseResponse`:

| Campo | Contenido | Panel del IDE |
|---|---|---|
| `problems` | Diagnósticos léxicos, sintácticos y semánticos con `{level, code, msg, loc, line, col}` | **PROBLEMAS**, y el gutter del editor marca las líneas en rojo (`err`) y amarillo (`warn`) |
| `parse_tree_dot` | El árbol de derivación real exportado a DOT, **anotado** con el tipo de cada expresión cuando hubo análisis | **ÁRBOL** |
| `symbol_table` | `SymbolTable::dump()`: el **estado final**, o sea el global con los miembros de funciones y clases anidados | **SÍMBOLOS** (mitad superior) |
| `scopes` | `ScopeCollector::to_json()`: una foto de **cada ámbito al cerrarse**, en orden de cierre | **SÍMBOLOS** (mitad inferior, "ÁMBITOS CERRADOS") |
| `closures` | Qué función anidada captura qué variables libres | **CLOSURES** |
| `types` | `TypeAnnotations::to_json()`: el tipo inferido de cada nodo de expresión, con el `id` del nodo en el DOT | **TIPOS** |
| `token_map` | Los tokens con su lexema y posición | **TOKENS** |

Las dos mitades del panel de símbolos **no son redundantes**, y esa es la razón
de que existan las dos: al terminar el recorrido la tabla solo conserva el
global, así que un `let` dentro de un `if` vive en un ámbito de bloque que se
desapiló y **no aparece en `symbol_table` por ningún lado**. `scopes` es lo
único que lo muestra. Comprobación mínima:

```
let g: integer = 1;
if (g > 0) { let dentro: integer = 2; print(dentro); }
```

`dentro` sale en un snapshot de `kind: "Block"` y en ninguna otra parte.

El `id` de cada fila de `types` es el mismo identificador (`n0`, `n1`, …) que
lleva ese nodo dentro de `parse_tree_dot`, así que una fila de la tabla y un
nodo del árbol dibujado se pueden correlacionar. Por eso el DOT se genera
**después** del análisis y no antes: necesita las anotaciones ya calculadas.

El análisis semántico corre solo en **LALR(1)/SLR(1)**, no en LL(1): la
transformación LL(1) elimina recursión izquierda y factoriza, lo que renombra
producciones y dejaría al `SemanticSpec` sin encontrarlas. En ese modo el árbol
sale sin anotar y `types` llega vacío.

---

## 7. Límites conocidos

Documentados a propósito, no olvidados:

- **`S021` no tiene productor real.** Todo símbolo invocable recibe su firma en
  `enter`, antes de recorrer su cuerpo —es lo que permite validar una llamada
  recursiva—, así que un símbolo sin firma nunca es una función: ese caso lo
  reporta `S020`. `S021` queda como red de seguridad de
  `functions::validate_call`, probada en sus propios tests unitarios.
- **Compiscript no tiene mapas, conjuntos ni tuplas.** El analizador sí los
  soporta —tipos, literales, indexado, iteración y compatibilidad— y está
  probado de punta a punta con `workspace/colecciones.yalp`, pero para verlos en
  un `.cps` habría que agregarles sintaxis a la gramática de Compiscript.
- **La detección de código muerto es conservadora**: una sentencia cuenta
  como terminal solo si el `return`/`break`/`continue` está en su subárbol sin
  cruzar otra secuencia de sentencias. Eso evita el falso positivo de
  `if (c) { return 1; } print(2);` —donde el `print` SÍ se alcanza— al precio
  de no detectar `{ return 1; } print(2);`, donde un bloque suelto siempre
  retorna. Saberlo exigiría propagar la terminalidad hacia arriba y decidir
  sobre las ramas de un `if`, que ya es análisis de alcanzabilidad completo.
- **Dos formas del `Compiscript.g4` oficial no están en el subconjunto
  ejecutable.** La primera es el **operador ternario** (`conditionalExpr`,
  `Compiscript.g4:85-87`). La segunda es la **asignación como expresión**
  (`AssignExpr` y `PropertyAssignExpr`, `Compiscript.g4:79-83`): acá
  `expr: or_expr` (`workspace/compiscript.yalp:504-506`) arranca en el `or`, y
  la asignación existe solo como sentencia (`assign_stmt`), así que
  `while ((x = next()) > 0)` no parsea. El control de flujo, en cambio, ya está
  completo: `if`/`while`/`do-while`/`for`/`foreach`/`switch`/`try-catch`.
- **Todo cuerpo de control de flujo exige llaves — igual que la `.g4`.** Esto
  NO es una desviación: `Compiscript.g4:51-55` define `ifStatement`,
  `whileStatement`, `forStatement` y `foreachStatement` con `block`, y
  `block: '{' statement* '}'` (`Compiscript.g4:30`). La gramática oficial
  también obliga a las llaves; acá `if_stmt`/`while_stmt`/`for_stmt`/
  `foreach_stmt` piden `bloque` y `bloque` es `LBRACE ... RBRACE`
  (`workspace/compiscript.yalp:286-295`), que es exactamente lo mismo. Los que
  no siguen la gramática oficial son dos ejemplos **en prosa** del enunciado
  (el de `break`/`continue` y el de recursión), escritos como
  `if (n < 60) continue;`; en `workspace/rubrica.cps` van con llaves. Admitir
  la sentencia suelta, además, reintroduciría el *dangling else*, una
  ambigüedad LALR real.
- **La gramática permite varios `default` en un `switch`** y en cualquier
  posición, mientras que la `.g4` admite a lo sumo uno y al final. Se resolvió
  así para no anidar epsilons que generan conflictos LALR; restringirlo sería
  una regla semántica, no gramatical.
- **La condición de un `for` es obligatoria**, mientras que en la `.g4` es
  `expression?`. Mismo motivo, y mismo criterio que el `for` de
  `examples/grammar/miniprog.yalp`.
- **`Symbol.storage` (offset y tamaño) nunca se llena.** Es la fase de
  asignación de almacenamiento del capítulo 7 del libro del dragón — ver la
  sección 8, que detalla qué le falta a la generación de código intermedio.
- **Los tipos de expresiones compuestas no siempre se resuelven.** `resolve_expr_type`
  cubre identificadores, `this`, literales, accesos a miembro, indexaciones,
  operadores, **llamadas a función** (por el retorno declarado del invocado) e
  **instanciaciones** (`new C(...)` tipa como `C`); una expresión más enredada
  devuelve `None`, y las reglas que dependen de ella se callan en vez de
  adivinar. Que las llamadas tipen no es cosmético: sin eso,
  `let x: T = f();`, `return f(x)` y `if (f())` quedaban sin verificar, porque
  todas se rinden ante un tipo desconocido. La instanciación exige, además, que
  la clase **exista** —la misma condición que usa `validate_instantiation`—: si
  no se declaró, el `S007` ya se reporta y tipar como `Named(inexistente)` haría
  que la asignación fallara con un `S006` **derivado** de aquel. Es el mismo
  criterio que hace neutro a un tipo desconocido: un hueco no debe cascadear en
  diagnósticos falsos.
- **`members` no se aplana transitivamente**: un local declarado dos niveles
  adentro de una función no aparece colgado de ella. Aplanarlo hacia arriba
  arriesgaría filtrar la visibilidad de ese nombre más allá de su bloque.

---

## 8. Qué recibe la fase de generación de código intermedio

Esta fase no está escrita. Lo que sigue es el contrato de traspaso: qué le deja
servido el análisis semántico y qué le va a faltar.

El punto de partida es una corrección de expectativa. La entrada principal de
esa fase **no es la tabla de símbolos, es el árbol**. La tabla no sabe que
existe `a = b + c * d`; sabe que hay una `a`, una `b`, una `c` y una `d`, de qué
tipo son y dónde se declararon. La estructura —qué se opera con qué, en qué
orden, qué cuelga de qué `if`— solo está en el árbol. El capítulo 6 del libro
del dragón plantea la traducción como una SDD sobre ese árbol, donde
`E -> E1 + E2` sintetiza `E.addr` y `E.code` concatenando el código de sus
hijos; la tabla entra en esa misma regla como servicio de consulta.

### Lo que ya recibe

| Qué | Dónde | Estado |
|---|---|---|
| El árbol de derivación | `sintactico::runtime::parse_tree::ParseNode` | Vive todo el pipeline; `analyze` lo toma por `&`, así que sobrevive intacto al análisis |
| El tipo de cada nodo de expresión | `types::TypeAnnotations` (campo `types` de `AnalysisResult`) | Completo para toda expresión que `resolve_expr_type` sepa tipar |
| Tipos, firmas y herencia | `symbols::Symbol` (`ty`, `signature`, `parent`, `members`) | Completo |
| Los ámbitos, incluidos los anónimos | `scopes::ScopeCollector` | Completo, con la salvedad de abajo |

Las anotaciones de tipo son la pieza nueva: antes el tipo de cada expresión se
calculaba durante el recorrido y se descartaba. Sin ellas, la regla de
`E -> E1 + E2` no tendría con qué decidir si hace falta una ampliación ni qué
instrucción emitir. Es el *árbol de análisis anotado* del libro, con una
diferencia deliberada: los atributos viven en un mapa lateral y no dentro del
`ParseNode` —igual que el `ParseTreeProperty` de ANTLR— para no meter un tipo
semántico en una estructura de la capa sintáctica. Ver
`types::annotations` para la invariante de las claves.

### Los dos huecos conocidos

**1. `Symbol.storage` nunca se llena.** El campo existe
(`StorageInfo { offset, size_bytes }`, `symbols/mod.rs`), pero los ocho sitios
que construyen un `Symbol` —cuatro en `symbols`, cuatro en `classes`— escriben
`storage: None`, y nada lo completa después. Es la asignación de almacenamiento
del capítulo 7 del libro: una pasada que recorra cada ámbito acumulando
desplazamientos según el `width` de cada tipo. Hasta que exista, el código
intermedio puede nombrar variables pero no ubicarlas en un marco de activación.

**2. Los ámbitos anónimos no están en la tabla final.** Al terminar el
recorrido la tabla viva solo conserva el Global; lo declarado dentro de una
función o una clase sobrevive anidado en `Symbol.members`, pero lo de un bloque
anónimo se descarta. Está en `ScopeCollector`, sí, pero como **lista plana
ordenada por cierre**, con un `depth` y sin enlace al ámbito padre. Para saber
qué locales caen en el marco de qué función hay que reconstruir esa relación a
partir del `depth`.

### La restricción de LL(1)

En modo LL(1) no hay salida semántica —ni tabla, ni ámbitos, ni anotaciones— y
por lo tanto tampoco habría código intermedio. No es un descuido:
`Grammar::parse_for_ll1_from_str` elimina recursión izquierda y factoriza, lo
que **renombra las producciones**; un `SemanticSpec` escrito contra los nombres
originales del `.yalp` dejaría de encontrarlas y emitiría diagnósticos falsos.
Preferimos no analizar antes que analizar mal. Ver `api::pipeline`.

---

## 9. Pruebas

170 tests unitarios (dentro de `src/`) y 145 de integración (en `tests/`). Dónde
están las baterías de casos, cómo se corren y cómo se navegan desde el IDE está
en la **§0**; acá va el desglose de qué cubre cada archivo.

Los de integración corren por el **pipeline real** (`.yal` + `.yalp` + fuente),
no con árboles armados a mano:

| Archivo | Qué cubre |
|---|---|
| `compiscript_tests.rs` (12) | Cero conflictos LALR, `ejemplo.cps` end-to-end, closures, redeclaración de funciones, y el módulo `arrays` con los casos de listas |
| `compiscript_clases_tests.rs` (2) | `clases_ok.cps` sin diagnósticos y `clases_errores.cps` con los códigos esperados |
| `control_flow_tests.rs` (4) | Condiciones, `break`/`continue`, y que las reglas no dependen de los nombres de Compiscript |
| `operator_tests.rs` (7) | Lógicos, comparaciones, unarios, y que un nombre de función o clase no es un valor |
| `duplicates_tests.rs` (5) | Los 15 casos de `workspace/duplicates_casos.txt`, el mismo archivo que se ejecuta desde el IDE |
| `colecciones_tests.rs` (2) | Las cuatro colecciones sobre `workspace/colecciones.yalp`, una gramática que NO es Compiscript — la prueba de que el soporte es configuración y no código por lenguaje |
| `bateria_semantica_tests.rs` (2) | Los 44 casos de `workspace/casos_semanticos.txt` —uno exitoso y uno fallido por regla—, en LALR y SLR, exigiendo el diagnóstico exacto de cada uno |
| `deadcode_tests.rs` (2) | Código inalcanzable y el corte de evaluación, sobre `codigo_muerto*.cps` |
| `contract_tests.rs` (7) | Los contratos entre fases: que el lexer entrega posiciones reales, que el árbol tiene la forma de la gramática, que las firmas y las clases sobreviven al pipeline |
| `pascalito_tests.rs` (5) · `gramatica_agnostica_tests.rs` (3) | Que todo lo anterior vale para otras gramáticas |
| `api_pipeline_tests.rs` (7) | La forma de la respuesta HTTP, incluido `scopes` |
| `semantic_analysis_tests.rs` (4) · `type_system_tests.rs` (13) | El walker y la tabla de tipos aislados |
| `rubrica_compiscript_tests.rs` (14) | El enunciado de punta a punta: `rubrica.cps` sin un solo diagnóstico, el constructor heredado, la concatenación de textos, y que las llamadas y `new` tienen tipo sin encadenar errores derivados |
| `type_annotations_tests.rs` (6) | El mapa lateral de tipos por nodo y su correlación con los `id` del DOT |

Casos de ejemplo en `workspace/`: `ejemplo.cps`, `clases_ok.cps`,
`clases_errores.cps`, `ejemplo_closures.cps`, `arreglos*.cps`, `caso1..4*.cps` y
`duplicates_casos.txt`.
