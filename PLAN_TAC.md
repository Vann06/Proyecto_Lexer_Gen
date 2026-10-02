# Plan — Generación de código intermedio (TAC)

Plan de trabajo del grupo para la fase de **código intermedio** de Compiscript:
qué hay que construir, qué ya existe, cómo se reparte entre los tres
integrantes y cómo se verifica. Leer completo antes de empezar tu parte.

---

## 1. Qué pide el enunciado

A partir del árbol sintáctico y la tabla de símbolos del análisis semántico,
generar **código de tres direcciones (TAC)**:

- **Lenguaje intermedio propio**, con sintaxis consistente y documentada:
  asignación, operaciones binarias y unarias, copia, saltos condicionales e
  incondicionales, etiquetas, llamadas y acceso indexado.
- **Traducción** de expresiones (aritméticas, lógicas y de comparación,
  respetando precedencia), declaraciones y asignaciones, `if`/`else`,
  `while`, `do-while`, `for`, `switch`, `break`/`continue`, funciones
  (parámetros, `return`, recursión, funciones anidadas y closures), clases
  (`new`, constructor, atributos, métodos, `this`) y listas (creación e
  índices).
- **Temporales con reciclaje**: liberar un temporal en cuanto su valor se
  consume, para usar la menor cantidad posible.
- **Tabla de símbolos extendida**: direcciones u offsets, tamaños,
  etiquetas, y registros de activación (parámetros, locales, temporales,
  valor y dirección de retorno, enlaces de control y de acceso).
- **Salida**: el TAC, los errores (si hay errores no se genera TAC) y el
  estado de la tabla de símbolos extendida.
- **Entregables**: batería de pruebas (casos exitosos y fallidos),
  documentación de la arquitectura y de cómo ejecutar el compilador,
  documentación detallada del lenguaje intermedio con ejemplos y supuestos,
  IDE funcional, y **commits individuales por integrante**.

---

## 2. Lo que ya existe (no hay que hacerlo)

El análisis semántico ya deja servido casi todo lo que necesita el generador.
Todo esto está probado: 200 pruebas unitarias más 11 en
`tests/intermediate_handoff_tests.rs`.

| Pieza | Dónde |
|---|---|
| Árbol real (LALR/SLR) y análisis semántico conectado a la API | `src/api/pipeline.rs`, `semantico::analyzer::analyze` → `AnalysisResult` |
| Tipo de cada nodo de expresión y conversión `int→float` por nodo | `TypeAnnotations::get`, `TypeAnnotations::coercion` |
| A qué declaración apunta cada identificador y cómo se llega a ella: `Static`, `Local`, `NonLocal { hops }` (enlaces de acceso) o `Field` (por `this`) | `semantico::bindings`, `AnalysisResult::resolve` / `symbol_for` |
| Registros de activación (ver §4) | `semantico::storage`, `Symbol::{storage, storage_size, nesting_level}`, `LayoutReport` |
| Layout de clases (el hijo empieza donde termina el padre) y vtable | `storage::allocate_classes` |
| Forma de cada `if`/`while`/`do_while`/`for`/`foreach`/`switch`/`case`/`default`/`try`/`catch` (qué hijo es la condición, el cuerpo, el `else`…) | directiva `%flow` → `intermedio::spec::{IntermediateSpec, FlowShape::child}` |
| Reconocedores de cada forma de expresión, sin nombrar producciones | `classes::{find_arithmetic, find_member_access, resolve_member, resolve_callee, flatten_arg_list, find_struct_literal, constructor_signature}`, `operators::{find_logical, find_comparison, find_unary}`, `collections::{find_array_literal, flatten_array_elements, find_index_access, element_type}`, `deadcode::flatten_sequence`, `spec.flow.{breaks, continues}` |
| Errores con tipo y ubicación; IDE con pestañas PROBLEMAS, SÍMBOLOS, TIPOS y ÁRBOL | `semantico::errors`, `frontend/IDE-lite/` |

El contrato completo del traspaso está en `ARQUITECTURA.md` §8.

**Restricción de diseño (igual que en todo el proyecto):** el generador no
puede nombrar producciones de Compiscript. Todo lo específico de la gramática
sale de las directivas del `.yalp` (`%flow`, `%call`, `%arith`…).

---

## 3. Lo que falta (lo que hace este plan)

1. El generador de TAC (hoy `src/intermedio/` solo tiene `spec.rs`).
2. Temporales: asignación, reciclaje y su lugar en el marco.
3. Tipo de `null`: `let d = null;` no tiene tipo, deja incompleta el área
   estática de `rubrica.cps` y el generador se negaría a correr.
4. `%` (módulo) no se tipa: falta `ArithmeticOperator::Modulo` y `%arith MOD`.
5. `print` no tiene directiva: el generador no puede reconocerlo sin nombrar
   `print_stmt`.
6. Los objetos no reservan lugar para el puntero a la vtable.
7. `find_child_index` y `find_identifier_child` son privados de `analyzer`.
8. La tabla de símbolos extendida no se muestra en el IDE.
9. No hay pestaña TAC en el IDE ni comando de consola para compilar un `.cps`.
10. No hay batería de pruebas de TAC ni documentación del lenguaje intermedio.

**Decisiones de diseño ya tomadas:**
- `null` es un **tipo referencia** (`Type::Null`).
- Los métodos usan **despacho virtual con vtable**.
- Hay un **intérprete de TAC** para que las pruebas ejecuten el código
  generado y comparen lo que imprime.

**Riesgos conocidos:**
- En Windows, de vez en cuando falla el lanzamiento de un binario de prueba;
  vuelve a pasar al correrlo solo. El CI corre en Linux.
- En modo LL(1) no hay análisis semántico, así que tampoco hay TAC
  (documentarlo).
- El enunciado menciona ANTLR y el proyecto usa su propio generador:
  confirmarlo con el catedrático.

---

## 4. Diseño del lenguaje intermedio

Se documenta en detalle en `CODIGO_INTERMEDIO.md` (Fase 4). Este es el
acuerdo base; si alguien necesita cambiarlo, avisa al grupo antes.

### Cómo se escriben las variables — convención C (decidida)

Las **instrucciones** son las del capítulo 6 del libro (§6.2.1). Lo que hubo
que decidir es cómo se escribe cada **variable** del programa. Se eligió una
mezcla de "por nombre" (como el libro) y "por dirección" (como el
registro de activación):

| Qué es | Cómo se escribe | Ejemplo |
|---|---|---|
| Variable de la función actual (local o parámetro) o global | **Por su nombre** | `total = 0`, `x = 10` |
| Otra variable distinta con el mismo nombre (p. ej. una local que tapa a una global) | Nombre con sufijo | `x.1` |
| Variable que vive en **otra** función (una función anidada que usa una variable de la que la encierra) | **Por dirección**, siguiendo el enlace de acceso, con el nombre comentado | `t0 = fp[12]` → `t1 = t0[-12]  ; total` |
| Resultado intermedio | Temporal | `t0`, `t1`, … |
| Constante | Literal | `5`, `true`, `"hola"`, `null` |
| Etiqueta | `L0`, `L1`, … y la de cada función: `f`, `Clase.metodo` | |

Por qué: se lee casi como el programa original (lo que necesitan los
calificadores), y a la vez muestra de forma explícita el acceso a variables
del entorno de definición, que es lo que pide el enunciado para funciones
anidadas y closures.

Las direcciones no se pierden: cada operando las guarda por dentro
(`Operand::Var { name, base, offset }`), el intérprete las usa, y la tabla de
símbolos extendida las muestra todas (`x` → `G[0]`, `total` → `fp[-12]`…).

Las bases de dirección son `fp` (marco actual; `fp[12]` es el enlace de
acceso), `G` (área estática) y un temporal (`t3[4]`: un objeto, una lista o
el marco de otra función). El nombre único de cada variable lo asigna
`Generator::var_name` (en `gen/mod.rs`), y hay que usarlo siempre en lugar de
inventar nombres.

**Ejemplo:**

```ts
let x: integer = 10;
function contador(paso: integer): integer {
  let total: integer = 0;
  function sumar(): integer {
    total = total + paso;        // total y paso son de contador
    return total;
  }
  let x: integer = sumar();      // otra x, local
  return x + 1;
}
print(contador(5));
```

```
func main nivel 0 marco 8
    x = 10
    param 5
    t0 = call contador, 1
    print t0
endfunc

func contador nivel 1 marco 24
    total = 0
    link fp                     ; enlace de acceso para sumar: el marco de contador
    t0 = call sumar, 0
    x.1 = t0
    t0 = x.1 + 1
    return t0
endfunc

func sumar nivel 2 marco 24
    t0 = fp[12]
    t1 = t0[-12]                ; total
    t2 = t0[16]                 ; paso
    t1 = t1 + t2
    t0[-12] = t1                ; total
    t1 = t0[-12]                ; total
    return t1
endfunc
```

(Los tamaños de marco y el orden de los temporales son ilustrativos; los
exactos los da el generador.)

### Instrucciones (cuádruplo `op, arg1, arg2, resultado`)

| Grupo | Sintaxis |
|---|---|
| Copia, binaria, unaria | `x = y` · `x = y op z` con `op` ∈ `+ - * / %` (enteros), `+f -f *f /f` (flotantes), `concat` (strings), `== != < <= > >=` · `x = minus y` (`minusf` en flotantes) · `x = not y` · `x = (float) y` (conversión `integer → float`, §6.5.2) |
| Saltos | `goto L` · `if x goto L` · `ifFalse x goto L` · `if x relop y goto L` · `L:` |
| Funciones | `func f nivel N marco M` · `endfunc` · `param x` · `link x` (enlace de acceso del llamado) · `x = call f, n` · `x = callv t, n` (llamada indirecta por vtable) · `return x` |
| Memoria | `x = y[i]` · `x[i] = y` (desplazamiento en bytes) · `x = alloc n` · `x = &vt_Clase` |
| E/S y errores | `print x` · `check_bounds a, i` · `check_zero x` · `try_begin L` · `try_end` · `x = exception` |

### Registro de activación (MIPS, ya calculado por `storage`)

| Offset | Contenido | Bytes |
|---|---|---|
| `$fp+16…` | Parámetros en orden. En un método, `$fp+16` es `this` (el primer parámetro oculto, que pasa quien llama) y los del usuario siguen desde `$fp+20` | 4 c/u (redondeado a la palabra) |
| `$fp+12` | Enlace de acceso: el `$fp` de la función que encierra estáticamente a esta | 4 |
| `$fp+4` | Valor de retorno (cabe hasta un `float`) | 8 |
| `$fp+0` | Enlace de control: el `$fp` de quien llamó | 4 |
| `$fp-4` | Dirección de retorno (`$ra`) | 4 |
| `$fp-8` | Estado guardado | 4 |
| `$fp-12…` | Locales, hacia abajo; los de bloques anidados acumulan en el marco de su función | según su tipo |
| debajo de los locales | Temporales, en ranuras de 8 bytes (los agrega el generador) | 8 c/u |

Lo que está **arriba** de `$fp` (parámetros, enlace de acceso, valor de
retorno, enlace de control) lo arma quien llama. Lo que está **abajo** es el
marco propio de la función: su tamaño es 8 bytes fijos + locales +
temporales.

### Reciclaje de temporales
- Cada función tiene su `TempPool`.
- `newtemp()` toma el número libre más bajo.
- Cuando una instrucción consume un operando que es temporal, ese temporal
  se libera en ese momento.
- El máximo de temporales vivos a la vez define el área de temporales del
  marco: `marco_final = frame_size + máx_vivos × 8` (ranuras de 8 bytes
  para que quepa un `float`).
- Se reportan por función los temporales pedidos y el máximo vivo.

### Supuestos
- Listas en el heap: `[longitud][e0][e1]…`, cada elemento redondeado a la
  palabra (4 bytes).
- Objetos: `[puntero a vtable][campos…]`.
- Strings: referencias inmutables.
- Booleanos con cortocircuito: código de saltos en condiciones (§6.6 del
  libro) y valor `0/1` cuando se usa como expresión.
- `switch` con caída entre casos (como TypeScript); `break` salta al final.
- `foreach` recorre listas por índice.
- Compiscript no tiene `throw`: un error en tiempo de ejecución (índice fuera
  de rango, división por cero) salta al `try` más interno, o termina el
  programa si no hay ninguno.

---

## 5. Fases y reparto

| Integrante | Fases | Depende de |
|---|---|---|
| **A** | Este documento + **Fase 0** (cimientos) + **Fase 1** (expresiones y control de flujo), con sus casos de prueba y su sección de `CODIGO_INTERMEDIO.md` | — |
| **B** | **Fase 2** (funciones, clases y listas), con sus casos de prueba y su sección de la documentación | Fase 0 de A, y las expresiones de la Fase 1 (los argumentos, índices y campos son expresiones) |
| **C** | **Fase 3** (intérprete, tabla extendida, API, CLI e IDE), la infraestructura de `tests/tac_battery_tests.rs` y las secciones de arquitectura y uso | `tac.rs` de A (la VM ejecuta esas instrucciones) |

**Orden:**
1. A entrega la Fase 0 (`tac.rs`, `temps.rs` y el esqueleto de `gen`) y las
   expresiones básicas de la Fase 1. Con eso queda fijo el conjunto de
   instrucciones.
2. Desde ahí C empieza la VM, la API y el IDE, B empieza funciones, clases y
   listas, y A sigue con el control de flujo.
3. Cada quien trabaja **en sus propios archivos y en su propia rama**, con PR
   a `main`. Así cada integrante tiene sus commits, como pide el enunciado.

| Integrante | Archivos |
|---|---|
| A | `src/semantico/*` (ajustes de la Fase 0), `src/intermedio/{tac.rs, temps.rs, gen/mod.rs, gen/expr.rs, gen/stmt.rs}` |
| B | `src/intermedio/gen/{func.rs, objects.rs, lists.rs}` |
| C | `src/intermedio/{vm.rs, symtab.rs}`, `src/api/*`, `src/bin/cpsc.rs`, `frontend/IDE-lite/*`, `tests/tac_battery_tests.rs` |

### Fase 0 — Cimientos (A)

**Ajustes al análisis semántico:**
- `Type::Null`: ancho de puntero, asignable a `string`, listas, mapas,
  conjuntos y clases. `let d = null` infiere `Null`, y una variable `Null`
  acepta cualquier referencia.
- `ArithmeticOperator::Modulo` (solo `integer`) y `%arith MOD modulo` en
  `workspace/compiscript.yalp`.
- `storage::allocate_classes` reserva el offset 0 para el puntero a la
  vtable en clases (no en structs). Hay que actualizar los tamaños esperados
  en las pruebas (`Figura` pasa de 4 a 8 bytes, etc.).
- Exponer `find_child_index` y `find_identifier_child` como `pub(crate)`.
- Directiva `%print <producción> <índice>` en `Grammar` e
  `IntermediateSpec`, con `%print print_stmt 2` en el `.yalp`.

**Núcleo de `src/intermedio/`:**
- `tac.rs`: `Operand`, `Instr`, `TacFunction`, `TacProgram`, con `Display`
  (la sintaxis de §4) y `to_json`.
- `temps.rs`: `TempPool` (asignar, liberar, estadísticas).
- `gen/mod.rs`: `generate(tree, &AnalysisResult, &SemanticSpec,
  &IntermediateSpec) -> Result<TacProgram, Vec<Problem>>`.
  - Solo genera si no hay errores semánticos y `layout.is_complete()`.
  - Recorrido genérico: secuencias y bloques en orden; un nodo de un solo
    hijo pasa al hijo; una sentencia-expresión se evalúa y su resultado se
    descarta.

### Fase 1 — Expresiones y control de flujo (A) — `gen/expr.rs`, `gen/stmt.rs`
- **Expresiones:**
  - literales;
  - identificadores, con la dirección según su `access`: para `NonLocal`
    se siguen `hops` enlaces desde `fp[12]`;
  - aritmética, con `(float)` donde `coercion` lo marca;
  - `concat`;
  - comparaciones;
  - `&&`, `||` y `!` con cortocircuito;
  - agrupación (`%group`).
- **Declaraciones y asignaciones:** `var`/`let`/`const` con inicializador;
  asignación a variable.
- **Control de flujo con `%flow`:**
  - `if`/`else`, incluido `else if`;
  - `while`, `do_while`, `for` (con `init`/`update` opcionales), `foreach`;
  - `switch` con caída entre casos y `default`;
  - `break`/`continue` con una pila de etiquetas;
  - `try`/`catch`.
- **Casos de prueba:** precedencia y asociatividad, cortocircuito, cada bucle
  con `break`/`continue`, y reciclaje (`a + b*c - d` usa pocos temporales).

### Fase 2 — Funciones, clases y listas (B) — `gen/func.rs`, `gen/objects.rs`, `gen/lists.rs`
- **Funciones:**
  - `func`/`endfunc` con el marco final;
  - `return`, con su conversión si hace falta;
  - llamadas: `param` (con conversión), `link` (siguiendo `nc − ng + 1`
    enlaces, donde `nc` es el nivel del llamador y `ng` el del llamado) y
    `call`;
  - recursión; funciones anidadas y closures por enlace de acceso.
- **Clases:**
  - `new C(args)`: `alloc instance_size`, guardar la vtable en el offset 0,
    `param` del objeto y de los argumentos, `call C.constructor` (el
    constructor se busca subiendo por la herencia);
  - campos: `obj[offset]` (los accesos `Field` van por `this`);
  - métodos con despacho virtual: `t = obj[0]`, `t2 = t[slot × 4]`,
    `callv t2, n`;
  - las vtables se emiten como datos.
- **Listas:** literal (`alloc`, guardar la longitud y los elementos), acceso
  indexado con `check_bounds`, asignación indexada.
- **Casos de prueba:** recursión (factorial), closure
  (`workspace/ejemplo_closures.cps`), herencia con sobrescritura llamada
  desde una variable del tipo padre, listas anidadas.

### Fase 3 — Intérprete, tabla extendida, API, CLI e IDE (C)
- **`vm.rs`, intérprete de TAC:**
  - marcos en pila con `fp`, heap de bloques, `link` y `callv`;
  - `try` con pila de manejadores;
  - `print` escribe a un búfer de salida;
  - límite de pasos para cortar ciclos infinitos.
- **`symtab.rs`, tabla extendida:** cada símbolo con tipo, offset, tamaño,
  etiqueta (funciones y métodos) y nivel; cada marco con su desglose
  (parámetros, locales, temporales, total). Sale como JSON y como texto.
- **API** (`src/api/pipeline.rs`, `src/api/mod.rs`):
  - campos nuevos `tac` (texto), `tac_quads` (JSON), `symbols_ext`, `temps`;
  - `run_output` cuando la petición trae `run: true`;
  - los errores de generación van en `problems` con códigos `C###`.
- **CLI:** binario `cpsc`: `cargo run --bin cpsc -- programa.cps [--run]
  [--slr]`. Usa por defecto la gramática de `workspace/` e imprime los
  errores o el TAC y la tabla extendida.
- **IDE** (`frontend/IDE-lite/`):
  - pestaña **TAC** con resaltado y un botón **▶ EJECUTAR** que muestra la
    salida;
  - la pestaña **SÍMBOLOS** gana columnas de offset, tamaño, etiqueta y
    nivel, más la lista de marcos;
  - subir el `?v=` de los `<script>` en `index.html` al cambiar un `.jsx`.

### Fase 4 — Batería y documentación (los tres; cada quien su parte)
- **`workspace/tac/ok/*.cps`:** cada programa trae su salida esperada en un
  comentario `// salida: ...`. `tests/tac_battery_tests.rs` lo compila, lo
  ejecuta en la VM y compara la salida. Para los ejemplos de la
  documentación compara además el TAC contra un `.tac` esperado.
- **`workspace/tac/error/*.cps`:** casos con error léxico, sintáctico o
  semántico. Se exige el código de error esperado **y que no se genere TAC**.
- **Cobertura mínima:** un caso por cada punto del enunciado (§1). Además,
  `workspace/rubrica.cps` completo debe compilar y ejecutarse.
- **`CODIGO_INTERMEDIO.md`:** conjunto de instrucciones, formato del marco,
  reciclaje de temporales, supuestos y un ejemplo traducido por cada
  construcción.
- **`ARQUITECTURA.md`** (sección del generador), **`GUIA_USO.md`** (cómo
  compilar desde el IDE, la CLI y las pruebas) y **`README.md`**.

---

## 6. Estado

| Fase | Estado |
|---|---|
| 0 — Cimientos (A) | ✅ Lista, con la convención C para escribir variables (§4). `null` es `Type::Null`; `%` es `ArithmeticOperator::Modulo`; las clases reservan el offset 0 para la vtable; `%print`; `find_child_index`/`find_identifier_child` son `pub(crate)`; `intermedio::{tac, temps, gen}` con los archivos de cada fase creados y sus puntos de enganche (`try_gen_*`), que por ahora reportan `C900`. Pruebas en `tests/tac_fase0_tests.rs`. |
| 1 — Expresiones y control de flujo (A) | ✅ Lista. `gen/expr.rs` (literales, variables según su acceso, aritmética con `(float)`, `concat`, `check_zero`, comparaciones, `&&`/`\|\|`/`!` con cortocircuito, agrupación) y `gen/stmt.rs` (declaraciones con valor por defecto, asignación directa `x = a + b`, `print`, `if`/`else`, `while`, `do-while`, `for`, `foreach`, `switch` con caída entre casos, `break`/`continue`, `try`/`catch`). Pruebas con el TAC exacto en `tests/tac_fase1_tests.rs`. En `rubrica.cps` solo quedan pendientes (C900) construcciones de la Fase 2. |
| 2 — Funciones, clases y listas (B) | Pendiente — se completan `gen/func.rs`, `gen/objects.rs` y `gen/lists.rs`, ya creados con sus firmas |
| Tabla de símbolos extendida — datos (A) | ✅ Listos. Ranura de retorno de 8 bytes (parámetros desde `$fp+16`, enlace de acceso en `$fp+12`); `StorageInfo::area` (`G`/`fp`/`obj`); `Symbol::label` con la etiqueta calificada de cada función; offsets de los campos y foto del Global en `scopes`; `extend_layout` para que el generador devuelva las ranuras de temporales de cada marco. Pruebas en `tests/intermediate_handoff_tests.rs` y `tests/tac_fase1_tests.rs`. |
| 3 — Intérprete, tabla extendida, API, CLI, IDE (C) | Pendiente — puede empezar ya: `tac.rs` está fijo y la tabla tiene todos los datos (ver abajo) |
| 4 — Batería y documentación | Pendiente |

**Cómo engancharse al generador (B):** cada archivo de `gen/` agrega métodos
al mismo `Generator` en su propio bloque `impl`. El recorrido de
`gen/mod.rs` ya llama a:
- `gen_function`, por cada nodo con `%scope … function`;
- `gen_class`, por cada clase o struct;
- `try_gen_call`, `try_gen_object_expr` y `try_gen_list_expr`, desde las
  expresiones;
- `try_gen_return`, `try_gen_object_assign` y `try_gen_list_assign`, desde
  las sentencias.

Para emitir se usan `emit`, `new_temp`, `release` (llamarlo **después** de
emitir la instrucción que consume el operando), `new_label` y `unsupported`.

**Para B — etiquetas:** cada `TacFunction` se tiene que llamar con la
etiqueta del símbolo de su función (`Symbol::label`: `contador`,
`contador.sumar`, `Animal.hablar`), no con el nombre suelto. `extend_layout`
usa esa etiqueta para devolverle a la tabla los temporales de cada marco.

**Para C — tabla de símbolos extendida:** todos los datos ya están; falta
mostrarlos (volcado en texto y vista del IDE):
- Por símbolo, en `scopes` (incluye la foto del Global, la última): `area`
  (`G`/`fp`/`obj`), `offset`, `size`, `nesting_level`. La etiqueta de cada
  función está en `Symbol::label` (`table`/`members`).
- Por marco, en `LayoutReport`: tamaño (8 fijos + locales), dueño
  (etiqueta y nivel) y, después de llamar a
  `intermedio::gen::extend_layout(&mut analysis.layout, &program)`, sus
  ranuras de temporales (`temp_slots`, y `main_temp_slots` para `main`).
  `storage::dump` ya muestra el desglose.
- Por clase: tamaño, vtable y offset de cada campo (`LayoutReport::classes`).
- El formato acordado está en §4 ("Registro de activación") y en el
  ejemplo de abajo ("Cómo debe verse la tabla de símbolos extendida").

**Lo que dejó la Fase 1 y conviene reusar (B):**

| Utilidad | Dónde | Para qué |
|---|---|---|
| `gen_expr(node)` | `gen/mod.rs` | El valor de cualquier expresión (argumentos, índices, la base de `obj.campo`) |
| `gen_widened(node)` / `widen(node, v)` | `gen/expr.rs` | Lo mismo, aplicando el `(float)` que marcó el análisis (argumentos y `return` ya vienen marcados) |
| `gen_cond(node, si_verdad, si_falso)` | `gen/expr.rs` | Una condición como saltos |
| `var_operand(&SymbolRef)` | `gen/mod.rs` | El operando de una variable según su acceso: por nombre, por enlaces de acceso (`NonLocal`) o por `this` (`Field`) |
| `follow_access_links(hops)` | `gen/mod.rs` | `t = fp[12]`, `t = t[12]`…; sirve también para el `link` de una llamada |
| `this_operand()` | `gen/mod.rs` | `this` dentro de un método (`fp[16]`). Ojo: un campo usado desde una función **anidada** dentro de un método necesita seguir enlaces hasta el marco del método; hoy `var_operand` asume que se usa directo en el método |
| `assign_to(&SymbolRef, valor)` | `gen/stmt.rs` | Asignar a una variable con su conversión y la asignación directa |
| `list_slot(tipo_elemento)` y `LIST_HEADER` | `gen/mod.rs` | El formato de lista en el heap, que ya usa `foreach`: `[longitud][e0][e1]…`, elementos redondeados a la palabra |
| `self.try_depth` | `gen/mod.rs` | Un `return` dentro de un `try` tiene que emitir un `try_end` por cada `try` abierto antes de salir (como hace `break`) |
| `self.jumps` | `gen/mod.rs` | Al traducir el cuerpo de una función hay que guardarlo y vaciarlo: un `break` no puede saltar fuera de la función |

---

## Cómo debe verse la tabla de símbolos extendida

Ejemplo de referencia para la Fase 3 (C). Los offsets y tamaños son los que
calcula hoy el análisis para este programa (está en
`tests/intermediate_handoff_tests.rs`, constante `TABLA`); la cantidad de
temporales es ilustrativa: la da el generador.

```ts
let x: integer = 10;
let nombre: string = "a";
function contador(paso: integer, b: boolean): integer {
  let total: integer = 0;
  function sumar(): integer { total = total + paso; return total; }
  if (b) { let extra: integer = 1; total = extra; }
  return sumar();
}
class Animal { var edad: integer = 0;     function hablar(): string { return "..."; } }
class Perro : Animal { var raza: string = ""; function hablar(): string { return "guau"; } }
```

**1. Símbolos**

| Nombre | Clase | Tipo | Ámbito | Área | Offset | Tamaño | Etiqueta | Nivel |
|---|---|---|---|---|---|---|---|---|
| `x` | variable | integer | Global | `G` | `+0` | 4 | — | — |
| `nombre` | variable | string | Global | `G` | `+4` | 4 | — | — |
| `contador` | función | integer | Global | — | — | marco 16 + temporales | `contador` | 1 |
| `paso` | parámetro | integer | contador | `fp` | `+16` | 4 | — | — |
| `b` | parámetro | boolean | contador | `fp` | `+20` | 1 (ranura de 4) | — | — |
| `total` | variable | integer | bloque de contador | `fp` | `-16` | 4 | — | — |
| `extra` | variable | integer | bloque del `if` | `fp` | `-12` | 4 | — | — |
| `sumar` | función | integer | bloque de contador | — | — | marco 8 + temporales | `contador.sumar` | 2 |
| `Animal` | clase | — | Global | heap | — | instancia 8 | `vt_Animal` | — |
| `edad` | campo | integer | Animal | `obj` | `+4` | 4 | — | — |
| `hablar` | método | string | Animal | — | — | marco 8 | `Animal.hablar` | 1 |
| `this` | parámetro | Animal | Animal.hablar | `fp` | `+16` | 4 | — | — |
| `Perro` | clase (hereda de Animal) | — | Global | heap | — | instancia 12 | `vt_Perro` | — |
| `raza` | campo | string | Perro | `obj` | `+8` | 4 | — | — |
| `hablar` | método | string | Perro | — | — | marco 8 | `Perro.hablar` | 1 |

(Los locales de un bloque interno reciben offset antes que los del bloque de
afuera, porque cada bloque se ubica al cerrarse: `extra` → `-12`, `total` →
`-16`. No se pisan.)

**2. Área estática (`G`) — 8 bytes:** `G+0 x` (4), `G+4 nombre` (4).

**3. Un registro de activación por función** (ejemplo: `contador`)

```
        ┌──────────────────────────────┐   ← lo arma quien llama
fp+20   │ b               parámetro    │  4
fp+16   │ paso            parámetro    │  4
fp+12   │ enlace de acceso             │  4
fp+4    │ valor de retorno             │  8
fp+0    │ enlace de control            │  4
        ├──────────────────────────────┤   ← marco propio: 8 fijos + locales + temporales
fp-4    │ dirección de retorno ($ra)   │  4
fp-8    │ estado guardado              │  4
fp-12   │ extra           local (if)   │  4
fp-16   │ total           local        │  4
fp-24   │ t0              temporal     │  8
        └──────────────────────────────┘
```

Los temporales van debajo de los locales, en ranuras de 8 bytes: `tk` en
`fp - (marco_sin_temporales + 8·(k+1))`.

**4. Objetos y vtables**

```
Animal (8 bytes)                 Perro (12 bytes)
+0  puntero a vt_Animal          +0  puntero a vt_Perro
+4  edad     integer             +4  edad     integer   (heredado)
                                 +8  raza     string

vt_Animal: [0] Animal.hablar     vt_Perro:  [0] Perro.hablar   (sobrescribe la misma posición)
```

---

## 7. Verificación final

- `cargo test --no-fail-fast` en local y en el CI (debug y release),
  incluida `tac_battery_tests`.
- `cargo run --bin cpsc -- workspace/rubrica.cps --run` imprime la salida
  esperada del programa.
- `cargo run --bin cpsc -- workspace/tac/error/<caso>.cps` muestra el error
  y no muestra TAC.
- En el IDE: cargar `rubrica.cps` → **▶ ANALIZAR** → pestaña **TAC** →
  **▶ EJECUTAR**, y revisar que **SÍMBOLOS** muestre offsets y marcos.
