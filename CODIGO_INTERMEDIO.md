# Código intermedio (TAC) de Compiscript

Documentación del lenguaje intermedio que genera SYNTRA para Compiscript:
qué instrucciones existen, cómo se escriben, cómo se traduce cada
construcción del lenguaje y qué se supuso. Las instrucciones están definidas
en `src/intermedio/tac.rs` (su `Display` produce exactamente la sintaxis de
este documento) y todos los ejemplos son la salida real del generador,
fijada por los tests (`tests/tac_fase1_tests.rs`, `tests/tac_fase2_tests.rs`).

Contenido:

1. [Cómo usar el compilador](#1-cómo-usar-el-compilador)
2. [Arquitectura](#2-arquitectura)
3. [El conjunto de instrucciones](#3-el-conjunto-de-instrucciones)
4. [Variables, direcciones y registro de activación](#4-variables-direcciones-y-registro-de-activación)
5. [Temporales: asignación y reciclaje](#5-temporales-asignación-y-reciclaje)
6. [Expresiones y asignaciones](#6-expresiones-y-asignaciones)
7. [Control de flujo](#7-control-de-flujo)
8. [Funciones, clases y listas](#8-funciones-clases-y-listas)
9. [Tabla de símbolos extendida](#9-tabla-de-símbolos-extendida)
10. [Errores](#10-errores)
11. [Supuestos generales](#11-supuestos-generales)

## 1. Cómo usar el compilador

**Línea de comandos** (`src/bin/cpsc.rs`). Usa la gramática de
`workspace/compiscript.yal` + `workspace/compiscript.yalp`:

```bash
cargo run --bin cpsc -- workspace/tac/ok/functions.cps          # TAC + tabla de símbolos extendida
cargo run --bin cpsc -- workspace/tac/ok/functions.cps --run    # además ejecuta el TAC
cargo run --bin cpsc -- workspace/tac/error/semantic_errors.cps # solo errores, sin TAC
```

Imprime dos secciones, `TAC` y `TABLA DE SÍMBOLOS EXTENDIDA` (y `SALIDA`
con `--run`). Si hay errores los imprime por `stderr` con su código y
ubicación y termina con código de salida distinto de 0. `--slr` cambia el
parser LALR por SLR.

**IDE** (`frontend/IDE-lite/`):

```bash
docker compose up
```

Abrir `http://localhost:4000`, cargar un `.cps` (por ejemplo
`workspace/rubrica.cps`) → **▶ ANALIZAR**. El TAC aparece en la pestaña
**TAC** (con **▶ EJECUTAR** para correrlo), la tabla extendida en
**SÍMBOLOS** y los errores en **PROBLEMAS** con su fase (`L` léxico,
`P` sintáctico, `S` semántico, `C` generación, `R` ejecución), línea y
columna. Sin Docker: `cargo run --bin api` y servir `frontend/IDE-lite` con
`python3 -m http.server`.

**API**: `POST /api/pipeline` con `{yal_content, yalp_content, input,
mode, source_name, run}`; la respuesta trae `tac` (texto), `tac_quads`
(cuádruplos en JSON), la tabla extendida y, con `run: true`, la salida del
programa.

**Batería de pruebas**:

```bash
cargo test --test tac_battery_tests
```

Recorre `workspace/tac/ok/*.cps` (cada caso declara su salida esperada con
`// salida: …`; se genera el TAC, se ejecuta y se compara) y
`workspace/tac/error/*.cps` (cada caso declara `// error: S0xx`; se exige
ese error y que **no** se genere TAC). Para agregar un caso basta con
crear un `.cps` nuevo en una de las dos carpetas. `cargo test` corre todo
el proyecto, incluidas las pruebas de TAC exacto
(`tac_fase0/1/2_tests.rs`) y de la API (`api_fase3_tests.rs`).

## 2. Arquitectura

```
.cps ─► lexer (DFA de compiscript.yal) ─► parser LALR (compiscript.yalp) ─► ParseNode
                                                                              │
          ┌───────────────────────────────────────────────────────────────────┘
          ▼
   semantico::analyze ── tabla de símbolos + tipos + storage (offsets, marcos)
          │  ¿errores? ── sí ─► reporte (S0xx), no se genera TAC (C001)
          ▼  no
   intermedio::gen::generate ── recorre el mismo árbol ─► TacProgram
          │
          ├─► extend_layout: devuelve a la tabla las ranuras de temporales
          ├─► intermedio::symtab: tabla de símbolos extendida (texto / JSON)
          └─► intermedio::vm: ejecuta el TAC (opcional, --run / ▶ EJECUTAR)
```

| Módulo | Qué hace |
|---|---|
| `src/intermedio/tac.rs` | El lenguaje: `Instr`, `Operand`, `TacFunction`, `TacProgram`, y su sintaxis textual y JSON. |
| `src/intermedio/temps.rs` | `TempPool`: asignación y reciclaje de temporales (§5). |
| `src/intermedio/spec.rs` | `IntermediateSpec`: lee las directivas `%flow` y `%print` del `.yalp`. |
| `src/intermedio/gen/mod.rs` | `Generator`: el recorrido, acceso a variables (`var_operand`, `follow_access_links`), etiquetas, `generate` y `extend_layout`. |
| `src/intermedio/gen/expr.rs` | Expresiones y condiciones con cortocircuito. |
| `src/intermedio/gen/stmt.rs` | Declaraciones, asignaciones, `print` y control de flujo. |
| `src/intermedio/gen/func.rs` | Funciones, llamadas, `return`. |
| `src/intermedio/gen/objects.rs` | `new`, campos, métodos y vtables. |
| `src/intermedio/gen/lists.rs` | Literales de lista y acceso indexado. |
| `src/intermedio/symtab.rs` | Tabla de símbolos extendida. |
| `src/intermedio/vm.rs` | Intérprete del TAC (valida que el código generado es correcto). |
| `src/api/pipeline.rs`, `src/bin/cpsc.rs` | Conectan todo para el IDE y la línea de comandos. |

**Traducción dirigida por la sintaxis, pero sin nombres de producción.** El
generador, igual que el analizador semántico, no conoce los nombres de las
reglas de la gramática. Lo que necesita saber lo leen de directivas del
`.yalp`: las del análisis (`%declare`, `%arith`, `%call`, …) y dos propias:

```
%flow while while_stmt cond=2 body=4
%print print_stmt 2
```

`%flow` dice qué producción es un `if`/`while`/`do_while`/`for`/`foreach`/
`switch`/`try` y en qué hijo está cada parte (`cond`, `then`, `else`,
`body`, `init`, `update`…). Detalle en `ARQUITECTURA.md` (tabla de
directivas) y §8 (contrato con el análisis).

**Por qué no ANTLR.** El enunciado menciona ANTLR para el paso léxico/
sintáctico. Este proyecto usa el lexer y el parser LALR que genera él mismo
(fases anteriores del curso) a partir de `compiscript.yal`/`.yalp`, que es
la gramática de `Compiscript.g4` reescrita. El árbol que recibe el
generador es el árbol de derivación real de ese parser. Ver
`ARQUITECTURA.md` ("por qué no se usó ANTLR").

## 3. El conjunto de instrucciones

Cada instrucción es un cuádruplo `(op, arg1, arg2, resultado)` con a lo
sumo tres direcciones (§6.2 del libro del dragón). Un operando es una
variable, un temporal (`t0`, `t1`…), una constante (`5`, `2.5`, `true`,
`"hola"`, `null`) o una dirección de memoria `base[desplazamiento]` (§4).

| Grupo | Sintaxis | Significado |
|---|---|---|
| Copia | `x = y` | |
| Binaria | `x = y op z` | `op` ∈ `+ - * / %` (enteros), `+f -f *f /f` (flotantes), `concat` (strings), `== != < <= > >=` (resultado booleano). No hay `and`/`or`: `&&` y `\|\|` se traducen con saltos (§6) |
| Unaria | `x = minus y` · `x = minusf y` · `x = not y` · `x = (float) y` | Negación entera/flotante, negación lógica, conversión `integer → float` (§6.5.2) |
| Etiqueta | `L3:` | |
| Salto incondicional | `goto L3` | |
| Saltos condicionales | `if x goto L` · `ifFalse x goto L` · `if x relop y goto L` | El tercero compara sin pasar por un temporal |
| Funciones | `func f nivel N marco M` … `endfunc` | Delimitan una función; `N` es su nivel de anidamiento, `M` el tamaño de su marco (con temporales) |
| Llamadas | `param x` · `link x` · `x = call f, n` · `call f, n` · `x = callv t, n` | Argumento, enlace de acceso, llamada directa (con o sin resultado), llamada indirecta por vtable |
| Retorno | `return x` · `return` | |
| Acceso indexado | `x = y[i]` · `x[i] = y` | Lectura / escritura en `y + i` bytes |
| Heap | `x = alloc n` · `x = &vt_C` | Reserva `n` bytes; dirección de un dato (vtable) |
| Datos | `data vt_C: f, g, …` | Tabla de métodos de una clase |
| E/S | `print x` | |
| Verificaciones | `check_bounds a, i` · `check_zero x` | Error de ejecución si `i` está fuera de la lista `a` o si `x` es 0 |
| Excepciones | `try_begin L` · `try_end` · `x = exception` | Apila / desapila un manejador; mensaje del error capturado |

Un comentario empieza con `;` y nunca cambia el significado: el generador
lo usa para poner el nombre de una variable que se accede por dirección
(`t0[-12]  ; total`). La primera línea del programa es `; área estática: N
bytes`; luego los `data` de las vtables, `main` (el código de nivel de
programa, nivel 0) y las funciones en orden del fuente.

## 4. Variables, direcciones y registro de activación

**Cómo se escribe cada variable** (convención elegida para que el TAC se
lea como el programa y, a la vez, se vea el registro de activación):

| Qué es | Cómo se escribe | Ejemplo |
|---|---|---|
| Variable global, local o parámetro de la función actual | Por su nombre | `total = 0` |
| Otra variable con el mismo nombre (una local que tapa a una global) | Nombre con sufijo | `x.1 = 2` |
| Variable de **otra** función (desde una función anidada) | Por dirección, siguiendo el enlace de acceso, con el nombre comentado | `t0 = fp[12]` → `t0[-12]  ; total` |
| Campo de un objeto | Por dirección | `t0[8]  ; raza` |
| Resultado intermedio | Temporal | `t0`, `t1` |

Aunque se escriba por nombre, cada variable guarda su dirección real
(`Operand::Var { name, base, offset }`) y el intérprete la usa. Las bases
son `fp` (el marco actual), `G` (el área estática) y un temporal (un
objeto, una lista o el marco de otra función).

**Registro de activación** (lo calcula `semantico::storage`, el generador
le agrega el área de temporales):

| Offset | Contenido | Bytes | Lo arma |
|---|---|---|---|
| `fp+16…` | Parámetros, en orden. En un método `fp+16` es `this` | 4 c/u | quien llama (`param`) |
| `fp+12` | Enlace de acceso: el marco de la función que la encierra | 4 | quien llama (`link`) |
| `fp+4` | Valor de retorno | 8 | quien llama |
| `fp+0` | Enlace de control: el `fp` de quien llamó | 4 | quien llama |
| `fp-4` | Dirección de retorno | 4 | la función |
| `fp-8` | Estado guardado | 4 | la función |
| `fp-12…` | Variables locales (las de bloques internos también) | según tipo | la función |
| debajo | Temporales: `tk` en `fp - (base + 8·(k+1))` | 8 c/u | la función |

`marco M` en `func f nivel N marco M` es la parte propia de la función:
8 bytes fijos + locales + temporales. Las globales viven en el área
estática `G`, y `main` usa un marco solo para sus temporales.

## 5. Temporales: asignación y reciclaje

`src/intermedio/temps.rs`, un `TempPool` por función:

- `new_temp()` entrega el temporal libre de **número más bajo**, o uno
  nuevo si no hay libres.
- Cuando una instrucción **consume** un operando que es temporal, el
  generador lo libera (`release`) justo después de emitirla: ese valor no
  se vuelve a leer, así que su ranura queda libre.
- Las variables nunca se reciclan (pueden volver a leerse).
- **Asignación directa**: si el último paso de una expresión va a una
  variable, se escribe en la variable sin pasar por un temporal
  (`x = t0 - d`, no `t1 = t0 - d; x = t1`).

Como siempre se reusa el número más bajo, el temporal más alto que aparece
en una función más uno es el máximo de temporales vivos a la vez: esas son
las ranuras de su marco (`TacFunction::max_temps`). Cada función reporta
también cuántos temporales se pidieron en total (`temps_requested`).

```ts
let x: integer = a + b * c - d;
```
```
    t0 = b * c
    t0 = a + t0          ; t0 se consumió y se reusa
    x = t0 - d           ; asignación directa
```

Se pidieron 3 temporales, pero nunca hubo más de 1 vivo: el marco reserva
una sola ranura. En `a * b + a * b` el primer producto sigue vivo mientras
se calcula el segundo, así que se usan dos (`t0`, `t1`).

## 6. Expresiones y asignaciones

**Aritmética, precedencia y asociatividad.** La precedencia y la
asociatividad las resuelve la gramática (el árbol ya viene con la forma
correcta); el generador recorre en post-orden. `a - b - c` es `(a - b) - c`:

```
    t0 = a - b
    x = t0 - c
```

**División y módulo** van precedidos de `check_zero` sobre el divisor (si es
la constante 0 o una variable). **Concatenación** de strings con `+` se
traduce a `concat`. **Negación** a `minus`. Si el análisis marcó una
conversión `integer → float`, se emite `t = (float) x` antes de operar.

```ts
let q: integer = a / b; let r: integer = a % b;
let s: string = "ho" + "la";
let n: integer = -a;
```
```
    check_zero b
    q = a / b
    check_zero b
    r = a % b
    s = "ho" concat "la"
    n = minus a
```

**Comparaciones** como valor: `t0 = a < b`. En una condición se traducen a
un salto `if a relop b goto L` (sin temporal).

**Lógicas con cortocircuito** (código de saltos, §6.6 del libro). En una
condición, `&&` y `||` nunca evalúan el operando derecho si no hace falta.
Usadas como valor, se generan los saltos y luego se materializa `true`/
`false` en un temporal:

```ts
let ok: boolean = a < b && b != 0 || !(a == 1);
```
```
    if a >= b goto L3       ; a < b falso → probar el ||
    if b != 0 goto L2       ; las dos verdaderas → true
L3:
    if a == 1 goto L0       ; !(a == 1) falso → false
L2:
    t0 = true
    goto L1
L0:
    t0 = false
L1:
    ok = t0
```

`!x` como valor es `t = not x`. Una condición literal (`while (true)`) no
genera ninguna comparación.

**Declaraciones y asignaciones.** `let`/`var`/`const` con inicializador se
traducen a una copia (o a la asignación directa de la última operación).
Sin inicializador reciben el valor por defecto de su tipo: `0` para
`integer`, `false` para `boolean`, `null` para strings, listas y objetos.
`const` se traduce igual que `let`: el análisis semántico ya garantizó que
tiene inicializador y que no se reasigna (S004/S005).

```ts
let nombre: string;
let x: integer = 1;
{ let x: integer = 2; print(x); }
print(x);
```
```
    nombre = null
    x = 1
    x.1 = 2
    print x.1
    print x
```

## 7. Control de flujo

Todas las construcciones usan etiquetas nuevas `L0`, `L1`… (únicas en el
programa) y saltos. La condición se traduce con el código de saltos de §6,
saltando al falso con la condición invertida.

**`if` / `else if` / `else`:**

```ts
if (a < 3 && a != 0) { print(1); } else if (a == 5) { print(2); } else { print(3); }
if (a > 0) { print(4); }
```
```
    if a >= 3 goto L1
    if a == 0 goto L1
    print 1
    goto L0
L1:
    if a != 5 goto L3
    print 2
    goto L2
L3:
    print 3
L2:
L0:
    if a <= 0 goto L4
    print 4
L4:
```

**`while`, `do-while`, `for`, con `break` y `continue`.** Cada bucle apila
su par de etiquetas (`break` → salida, `continue` → donde sigue la
iteración): en `while` es la condición, en `do-while` la condición del
final y en `for` el paso de actualización.

```ts
while (i < 10) { if (i == 5) { break; } i = i + 1; if (i == 2) { continue; } print(i); }
do { i = i - 1; } while (i > 0);
for (let j: integer = 0; j < 3; j = j + 1) { if (j == 1) { continue; } print(j); }
```
```
L0:                      ; while: condición
    if i >= 10 goto L1
    if i != 5 goto L2
    goto L1              ; break
L2:
    i = i + 1
    if i != 2 goto L3
    goto L0              ; continue
L3:
    print i
    goto L0
L1:
L4:                      ; do-while: cuerpo primero
    i = i - 1
L5:                      ; continue del do-while
    if i > 0 goto L4
L6:                      ; break del do-while
    j = 0                ; for: inicialización
L7:
    if j >= 3 goto L9
    if j != 1 goto L10
    goto L8              ; continue → paso
L10:
    print j
L8:
    j = j + 1            ; paso
    goto L7
L9:
```

Las partes de un `for` son opcionales: `for (; i < 2; )` no emite
inicialización ni paso.

**`foreach`** recorre una lista por índice, con el formato de lista del
heap (`[longitud][e0][e1]…`, §Listas):

```ts
foreach (n in notas) { if (n == 0) { continue; } print(n); }
```
```
    t0 = notas           ; la lista
    t1 = t0[0]           ; longitud
    t2 = 0               ; índice
L0:
    if t2 >= t1 goto L2
    t3 = t2 * 4
    t3 = t3 + 4
    n = t0[t3]
    if n != 0 goto L3
    goto L1              ; continue
L3:
    print n
L1:
    t2 = t2 + 1
    goto L0
L2:
```

**`switch`.** El discriminante se evalúa una vez en un temporal; luego una
cadena de comparaciones salta al cuerpo de cada `case` y, si ninguno
coincide, al `default` (o a la salida). Los cuerpos van uno detrás de otro,
así que **caen** al siguiente caso como en TypeScript; `break` salta a la
salida. Un `continue` dentro de un `switch` va al bucle que lo contiene:

```ts
switch (x) { case 1: print(1); case 2: print(2); break; default: print(0); }
```
```
    t0 = x
    if t0 == 1 goto L1
    if t0 == 2 goto L2
    goto L3              ; default
L1:
    print 1              ; cae al caso 2
L2:
    print 2
    goto L0              ; break
L3:
    print 0
L0:
```

**`try` / `catch`** (extra). `try_begin L` apila un manejador; si una
verificación falla (`check_zero`, `check_bounds`) se salta al manejador
más interno con el mensaje en `exception`. Un `break`, `continue` o
`return` que sale de un `try` emite antes un `try_end` por cada manejador
que abandona:

```ts
while (x < 3) { try { x = 10 / x; break; } catch (e) { print(e); } x = x + 1; }
```
```
L0:
    if x >= 3 goto L1
    try_begin L2
    check_zero x
    x = 10 / x
    try_end              ; el break sale del try
    goto L1
    try_end
    goto L3
L2:
    e = exception
    print e
L3:
    x = x + 1
    goto L0
L1:
```

## 8. Funciones, clases y listas

Todos los ejemplos salen de `tests/tac_fase2_tests.rs`, que fija el TAC exacto.

### Instrucciones que intervienen

| Instrucción | Qué hace |
|---|---|
| `func f nivel N marco M` / `endfunc` | Una función; `M` es el marco con las ranuras de temporales. `endfunc` **implica el retorno**: una función que termina sin `return` no lo lleva |
| `param x` | Un argumento de la llamada que viene |
| `link x` | El enlace de acceso del llamado (ver abajo) |
| `x = call f, n` / `call f, n` | Llamada directa con `n` argumentos; sin `x` cuando la función no devuelve valor |
| `x = callv t, n` | Llamada indirecta a la dirección que guarda `t` (un método sacado de la vtable) |
| `return x` / `return` | Retorno, con o sin valor |
| `x = alloc n` | Reserva `n` bytes en el heap |
| `t[off] = y` / `x = t[off]` | Escritura y lectura a `off` bytes de `t` |
| `t[0] = &vt_C` | Guarda la dirección de la vtable de `C` |
| `check_bounds a, i` | Error en ejecución si `i` está fuera de la lista `a` |
| `try_end` | Desapila un manejador; un `return` lo emite una vez por cada `try` abierto |

### Convención de llamada

- **Orden:** primero se evalúan todos los argumentos, de izquierda a derecha,
  y después se emiten juntos sus `param`, luego el `link` y por último el
  `call`. Así en `dos(uno(), 2)` los `param` de `dos` no se mezclan con los de
  `uno`.
- **`this` va primero.** Un método recibe el objeto como primer `param`
  (queda en `fp[16]`) y los argumentos del usuario siguen desde `fp[20]`.
  `nargs` **cuenta a `this`**: `p.f(1)` es `callv t, 2`.
- **`link` (enlace de acceso).** Se calcula por el `Access` del nombre llamado:
  - global (`Static`): sin `link` (las funciones de nivel 1 no lo usan);
  - declarada en la función actual (`Local`): `link fp`;
  - declarada en una función de afuera (`NonLocal { hops }`): se siguen
    `hops` enlaces (`t = fp[12]`, `t = t[12]`…) y se emite `link t`.
  Es la regla `nc − ng + 1` del libro. Los métodos y `$init` son de nivel 1 y
  tampoco llevan `link`.
- **Una llamada sin valor** (`nada();`, `obj.poner(1);`) se traduce igual que
  una con valor, solo que sin destino.

```
func contador nivel 1 marco 24        ; ejemplo de PLAN_TAC §4
    total = 0
    link fp                           ; sumar está declarada aquí
    x.1 = call contador.sumar, 0
    ...
func contador.sumar nivel 2 marco 24
    t0 = fp[12]                       ; marco de contador
    t1 = fp[12]
    t0 = t0[-12] + t1[16]   ; total, paso
```

Recursión de una función anidada (un salto de enlace):

```
func externa.cuenta nivel 2 marco 16
    ...
    t0 = n - 1
    param t0
    t0 = fp[12]
    link t0
    t0 = call externa.cuenta, 1
```

Las etiquetas de función son las de la tabla de símbolos (`Symbol::label`):
`contador`, `contador.sumar`, `Animal.hablar`. `program.functions` sigue el
orden del fuente (la de afuera antes que las anidadas).

`link fp` se representa como `Operand::Var { name: "fp", base: Fp, offset: 0 }`:
es el valor del propio `$fp`, no el contenido de `fp[0]`. Quien ejecute el TAC
debe tratar el operando de `link` como una dirección de marco.

### Clases y objetos

Un objeto es `[puntero a vtable][campos…]` (offsets de `storage`); un struct no
tiene vtable. Una instancia de `Perro : Animal` con `Animal { nombre }` y
`Perro { raza }` mide 12 bytes: vtable en 0, `nombre` en 4, `raza` en 8.

**`new C(args)`:**

```
    t0 = alloc 12
    t0[0] = &vt_Perro
    param t0
    call Perro.$init, 1               ; el $init más cercano subiendo por la herencia
    param t0
    param "Toby"
    call Animal.constructor, 2        ; el constructor más cercano (puede ser heredado)
    p = t0
```

Si no hay constructor no hay segunda llamada; si ninguna clase de la cadena
declara campos, tampoco hay `$init`.

**`<Clase>.$init`:** función sintética (nivel 1, recibe `this` en `fp[16]`)
que inicializa los campos que declara la clase, en orden; primero llama al
`$init` del padre si lo hay. Cada campo se traduce como una declaración: con
inicializador o con el valor por defecto de su tipo (`0`, `false`, `null`).

```
func Perro.$init nivel 1 marco 16
    param this
    call Animal.$init, 1
    t0 = this
    t0[8] = "criollo"       ; raza
```

**Campos:** `obj.campo` es la dirección `t[offset]`, con el nombre como
comentario. Se pueden leer y escribir directamente (`print t0[8]`,
`t1[4] = t0`). La asignación calcula primero el valor y luego el objeto.
Dentro de un método, un campo sin `this.` y `this.campo` generan lo mismo.
Desde una función **anidada** dentro de un método, `this` y los campos se
alcanzan siguiendo los enlaces de acceso hasta el marco del método:

```
func Cuenta.conBono.total nivel 2 marco 24
    t0 = fp[12]
    t0 = t0[16]             ; this
    ...
```

**Métodos y vtable.** Cada clase emite un dato `vt_<Clase>` con la etiqueta de
la definición más derivada de cada método, en el orden de ranuras de
`layout.classes.vtable` (el constructor es una ranura más; una sobrescritura
conserva la ranura del padre):

```
data vt_Animal: Animal.constructor, Animal.hablar, Animal.presentar
data vt_Perro: Animal.constructor, Perro.hablar, Animal.presentar
```

La llamada `obj.m(args)` usa la ranura de `m` en la vtable de la **clase
estática** de `obj` (4 bytes por ranura), así que funciona con cualquier
subclase:

```
    param p
    t0 = p[0]               ; vtable
    t0 = t0[4]              ; ranura 1 × 4
    t0 = callv t0, 1
```

Un método llamado sin `this.` desde otro método también es virtual, sobre el
`this` actual. **Literal de struct** `P { x: 1, y: 2 }`: `alloc` y un
almacenamiento por campo, sin vtable.

### Listas

Una lista en el heap es `[longitud][e0][e1]…`: 4 bytes de cabecera y cada
elemento redondeado a la palabra (el mismo formato que recorre `foreach`).

```
    t0 = alloc 16           ; 4 + 3 × 4
    t0[0] = 3               ; longitud
    t0[4] = 1               ; e0
    t0[8] = 2
    t0[12] = 3
    lista = t0
```

Una lista vacía `[]` es `alloc 4` con longitud 0; una lista de listas guarda
punteros. El acceso `a[i]` comprueba el rango y lee en `4 + i × ancho`: con
índice constante el desplazamiento ya viene sumado; con una variable se calcula
con dos instrucciones (igual que `foreach`):

```
    check_bounds lista, 0
    t0 = lista[4]

    check_bounds lista, i
    t0 = i * 4
    t0 = t0 + 4
    t0 = lista[t0]
```

`m[1][0]` encadena dos accesos, cada uno con su `check_bounds`.

### Supuestos y límites

- La asignación a un elemento (`lista[0] = 5`) no existe: la gramática solo
  asigna a un identificador o a `objeto.campo`. Un `foreach` y el acceso
  indexado trabajan solo con listas (no con mapas, conjuntos ni tuplas, que la
  gramática actual no permite escribir).
- Los operandos de memoria (`t0[4]`) se leen cuando los consume la instrucción
  que los usa, no al evaluarlos; igual que una variable. Si una llamada
  intermedia modifica ese campo (`this.n + this.cambia()`), el valor leído es
  el nuevo.
- Las clases deben estar en el ámbito global. El análisis rechaza asignar un
  objeto a una variable de su clase padre (S006), así que la sobrescritura se
  observa dentro de la clase padre (`this.hablar()`).
- No hay tipo `float` en la gramática: la conversión `(float)` de los
  argumentos y del `return` está implementada (`gen_widened`) pero ningún
  fuente la dispara todavía.

## 9. Tabla de símbolos extendida

La tabla de símbolos del análisis semántico se complementa con lo que
necesita la generación de código, y la generación le devuelve lo que solo
ella sabe:

| Fase | Qué pone o usa en la tabla |
|---|---|
| Análisis semántico (`semantico::storage`) | Por símbolo: `area` (`G` estática, `fp` marco, `obj` campo, `heap` clase), `offset` en bytes, `size`, `nesting_level` y la etiqueta (`Symbol::label`: `contador.sumar`, `Animal.hablar`, `vt_Animal`). Por función: tamaño base del marco. Por clase: tamaño de instancia, offset de cada campo, ranuras de la vtable. |
| Generación (`gen`) | Consulta todo lo anterior para decidir cada operando (por nombre, por enlaces de acceso o por `this`), la etiqueta de cada `func`/`call` y el `alloc` de cada `new`. |
| Generación → tabla (`extend_layout`) | Devuelve las ranuras de temporales de cada marco; el marco final es base + ranuras × 8. |
| Intérprete (`vm`) | Usa los offsets y tamaños para ubicar cada variable en `G`, en el marco o en el heap. |
| Presentación (`intermedio::symtab`) | `ExtendedSymbolTable { symbols, frames, classes }` en JSON (CLI, API) y en la pestaña **SÍMBOLOS** del IDE. |

Para este programa:

```ts
let x: integer = 10;
function contador(paso: integer): integer {
  let total: integer = 0;
  function sumar(): integer { total = total + paso; return total; }
  return sumar();
}
class Animal { var edad: integer = 0; function hablar(): string { return "..."; } }
print(contador(5));
```

**Símbolos**

| Nombre | Clase | Tipo | Ámbito | Área | Offset | Tamaño | Etiqueta | Nivel |
|---|---|---|---|---|---|---|---|---|
| `x` | variable | integer | Global | `G` | 0 | 4 | | |
| `contador` | función | integer | Global | | | 12 | `contador` | 1 |
| `paso` | parámetro | integer | contador | `fp` | +16 | 4 | | |
| `total` | variable | integer | bloque de contador | `fp` | −12 | 4 | | |
| `sumar` | función | integer | bloque de contador | | | 8 | `contador.sumar` | 2 |
| `Animal` | clase | | Global | `heap` | | 8 | `vt_Animal` | |
| `edad` | campo | integer | Animal | `obj` | +4 | 4 | | |
| `hablar` | método | string | Animal | | | 8 | `Animal.hablar` | 1 |
| `this` | parámetro | Animal | Animal.hablar | `fp` | +16 | 4 | | |

**Registros de activación** (`frames`)

| Función | Nivel | Marco base | Ranuras de temporales | Marco total |
|---|---|---|---|---|
| `main` | 0 | 0 | 1 | 8 |
| `contador` | 1 | 12 | 1 | 20 |
| `contador.sumar` | 2 | 8 | 2 | 24 |
| `Animal.hablar` | 1 | 8 | 0 | 8 |

El marco base es 8 bytes fijos (dirección de retorno y estado guardado) más
los locales: `contador` tiene `total` (4) → 12. `main` no tiene locales
porque sus variables están en `G`. Lo que arma quien llama (parámetros,
enlace de acceso, valor de retorno, enlace de control) está arriba de `fp`
y no cuenta en el marco propio (§4).

**Clases**: `Animal`, instancia de 8 bytes (`+0` vtable, `+4 edad`),
`vt_Animal = [Animal.hablar]`.

## 10. Errores

Un programa con errores **no genera TAC**: se muestran los errores y nada
más. Cada error lleva código, mensaje, archivo, línea y columna:

```
[err S016] semantic_errors.cps:18:3: retorno incompatible en 'entero': se esperaba integer, se encontró string
[err S002] semantic_errors.cps:22:7: variable 'noExiste' no declarada
```

| Prefijo | Fase | Ejemplos |
|---|---|---|
| `L` | Léxica | `L001` carácter no reconocido |
| `P` | Sintáctica | `P001` token inesperado, con los tokens esperados y sugerencia "¿quisiste decir?" |
| `S` | Semántica | `S002` no declarada, `S006` asignación incompatible, `S013`/`S014` aridad/tipo de argumento, `S016` retorno… (lista completa en `CUMPLIMIENTO_REQUISITOS.md`) |
| `C` | Generación | `C001` no se genera porque hay errores semánticos; `C002` hay símbolos sin tipo (no se sabe su tamaño) |
| `R` | Ejecución (solo con `--run`) | índice fuera de rango (`check_bounds`), división entre cero (`check_zero`); se pueden capturar con `try/catch` |

Los casos de error de la batería están en `workspace/tac/error/`
(`lexical.cps`, `syntax.cps`, `semantic_errors.cps`) y los de ejecución en
`workspace/tac/ok/runtime_errors.cps`.

## 11. Supuestos generales

- **Tamaños**: `integer`, `boolean`, `string`, listas y objetos ocupan una
  palabra de 4 bytes (`boolean` ocupa 1, redondeado a 4 en parámetros y
  listas). Las ranuras de temporales y el valor de retorno miden 8 para que
  quepa un `float`, aunque la gramática actual no tiene ese tipo. Strings, listas y objetos son referencias al heap.
- **Booleanos**: cortocircuito con saltos en condiciones; `true`/`false`
  materializado cuando se usan como valor.
- **Globales** en el área estática `G`; el código de nivel de programa es
  la función `main` de nivel 0.
- **Funciones anidadas**: enlaces de acceso (no displays). Una función
  anidada lee y escribe las variables de la que la encierra en el marco de
  esta. Compiscript no tiene valores de tipo función, así que una función
  anidada nunca sobrevive al marco que la contiene: no hace falta mover
  variables capturadas al heap.
- **Métodos**: siempre virtuales, por vtable de la clase estática; `this`
  es el primer parámetro.
- **`switch`** con caída entre casos, como TypeScript.
- **Sin `throw`**: los errores de ejecución los producen `check_bounds` y
  `check_zero`, y saltan al `try` más interno o terminan el programa.
- **Asignación a elementos de lista** (`a[i] = x`) no existe porque la
  gramática no la permite (ver §8, "Supuestos y límites").
