# Plan — Generación de código MIPS

Plan de trabajo del grupo para la fase final de Compiscript: traducir el TAC
a **ensamblador MIPS** que se ejecute en un simulador. Dice qué hay que
construir, qué ya existe, cómo se reparte entre los tres integrantes y, para
cada fase, **qué recibe y qué entrega**. Leer completo antes de empezar tu
parte; las decisiones de §4 son el acuerdo del grupo y no se cambian sin
avisar.

---

## 1. Qué pide el enunciado

| Punto | Lo que se evalúa | Fase de este plan |
|---|---|---|
| Secuencias de llamada y retorno | Saltar hacia y desde funciones; preservar la memoria manejando `$sp`; paso de parámetros, dirección de retorno y valor de retorno **según el registro de activación** | Fase 2 (B) |
| Asignación de registros | `getReg()` que asigna registros libres; uso apropiado de `$t` y `$s`; usar la pila cuando no hay registros | Fase 0 (pila) y Fase 4 (`getReg`) (A) |
| Generación de código MIPS | Código ensamblador a partir del TAC, **ejecutable en un simulador** | Fases 1 y 3 (A, B), Fase 5 (C) |
| Entrada / salida | `.cps` → `.asm` + reporte de errores | Fase 5 (C) |
| Batería de pruebas | Casos exitosos y fallidos de la generación | Fase 6 (los tres) |
| IDE | Escribir y compilar Compiscript | Fase 5 (C) |
| Documentación | Arquitectura y cómo ejecutar | Fase 6 (los tres) |
| Repositorio | **Commits individuales** por integrante | Todas (§5) |

---

## 2. Lo que ya existe (no hay que hacerlo)

El TAC ya está preparado para esta fase (fila "TAC listo para MIPS" de
`PLAN_TAC.md`). La fase de MIPS **solo necesita el `TacProgram`**: cada
operando ya trae su dirección, cada función su marco, y los datos ya están
listados. No hace falta volver a consultar el árbol ni la tabla de símbolos.

| Pieza | Dónde | Para qué le sirve a MIPS |
|---|---|---|
| `TacProgram { static_size, data, main, functions }` | `src/intermedio/tac.rs` | La entrada completa de esta fase |
| `TacFunction { name, level, frame_size, max_temps, body }` y `total_frame_size()` | `tac.rs` | Etiqueta, tamaño del marco (con temporales) e instrucciones de cada función |
| `Operand::Var { name, base, offset }`, `Operand::Mem { base, offset, name }` con `Base::{Fp, Global, Temp(k)}` | `tac.rs` | La dirección de cada variable: `off($fp)`, `G+off` o `off(registro de tk)` |
| `Operand::{Temp, Int, Bool, Str, Null, Addr}` | `tac.rs` | Temporales y constantes |
| 23 instrucciones (`Instr`), con `print_*` tipado y `streq`/`strne` | `tac.rs` | Una traducción por instrucción (§6) |
| `DataItem { name, entries }`: las vtables | `tac.rs` | `.word` en `.data` |
| Enteros de 32 bits con vuelta y mensajes fijos de error | `intermedio/vm.rs` | El intérprete da **exactamente** la misma salida que MARS: sirve de oráculo para las pruebas |
| `CodegenError { code, message, line, col }` | `intermedio/gen/mod.rs` | Se reusa para los errores `M###` de esta fase |
| Contrato TAC → MIPS (datos, secuencia de llamada, rutinas, decisiones pendientes) | `CODIGO_INTERMEDIO.md` §12 | Base de este plan; §4 cierra lo que quedó pendiente |
| Pipeline, CLI `cpsc`, API `/api/pipeline`, IDE-lite con pestaña TAC | `src/api/`, `src/bin/cpsc.rs`, `frontend/IDE-lite/` | Se extienden con MIPS (Fase 5) |
| Batería de TAC con salidas esperadas | `workspace/tac/ok/*.cps`, `tests/tac_battery_tests.rs` | Los mismos programas deben dar la misma salida en MARS |

Módulo de destino: `src/codigo_objetivo/` (hoy solo tiene un comentario
"AÚN NO IMPLEMENTADO").

---

## 3. Lo que falta (lo que hace este plan)

1. La representación del ensamblador y la sección `.data`.
2. La traducción de cada instrucción del TAC.
3. Las secuencias de llamada y retorno (prólogo, epílogo, `param`, `link`,
   `call`, `callv`, `return`).
4. Las rutinas de apoyo escritas en MIPS (`print_*`, `concat`, `streq`,
   `alloc`, `check_bounds`, `check_zero`, `try`).
5. `getReg()`: vida de los temporales, descriptores de registros y de
   direcciones, `$t`/`$s`, derrame a la pila.
6. Correr el `.asm` en MARS desde las pruebas, la CLI y la API.
7. CLI (`--mips`, `-o`), API (`mips`) e IDE (pestaña MIPS).
8. Batería de pruebas de MIPS y documentación (`CODIGO_OBJETIVO.md`).

---

## 4. Decisiones de diseño (acuerdo del grupo)

### 4.1 Simulador: MARS 4.5
- El `.jar` va en el repositorio: `tools/Mars4_5.jar` (licencia MIT). Así
  las pruebas y el CI usan siempre la misma versión, sin descargar nada.
- Se corre sin interfaz:
  `java -Dline.separator=<LF> -Dfile.encoding=ISO-8859-1 -jar tools/Mars4_5.jar nc sm se2 ae3 programa.asm`
  - `nc`: sin aviso de copyright; `sm`: empieza en `main`;
  - `se2` / `ae3`: código de salida 2 si falla la simulación, 3 si falla el
    ensamblado (para distinguirlos del `1` del programa, §4.7).
  - `-Dfile.encoding=ISO-8859-1` (y `-Dstdout.encoding`, para Java 18+): MARS
    escribe cada byte como un carácter y con Latin-1 salen tal cual.
  - `-Dline.separator`: al terminar, MARS escribe siempre un salto de línea
    propio, aunque el programa no imprima nada. Con esta opción es `
` en
    todos los sistemas, y `run_mars` quita ese único salto: lo que devuelve es
    exactamente lo que imprimió el programa.
- Requiere Java 8 o superior. Si no hay `java` o no está el `.jar`, las
  pruebas que ejecutan MARS **se saltan con un aviso**, salvo que exista la
  variable `MIPS_REQUIRED=1` (el CI la pone y ahí fallan).

### 4.2 Registros

| Registro | Uso |
|---|---|
| `$t0–$t7` | Temporales del TAC que **no** siguen vivos después de una llamada (`getReg`) |
| `$s0–$s7` | Temporales del TAC que **sí** siguen vivos después de una llamada. La función que los usa los guarda en su prólogo |
| `$t8`, `$t9` | Auxiliares: direcciones (`base + índice`), variables cargadas para una sola instrucción, recarga de un derrame. Nunca guardan un valor de una instrucción a otra |
| `$a0–$a3`, `$v0`, `$v1` | Solo para `syscall` y para las rutinas de apoyo (§4.6). Nunca guardan un temporal |
| `$fp`, `$sp`, `$ra` | Marco, pila y dirección de retorno |
| `$at`, `$k0`, `$k1`, `$gp` | No se usan (`$at` es del ensamblador) |

### 4.3 Registro de activación (no cambia; es el de `storage`)

| Offset | Contenido | Lo escribe |
|---|---|---|
| `$fp+16…` | Parámetros en orden (`this` primero en un método), 4 bytes c/u | Quien llama |
| `$fp+12` | Enlace de acceso | Quien llama |
| `$fp+4` | **Valor de retorno** (8 bytes) | La función, en `return` |
| `$fp+0` | Enlace de control (el `$fp` de quien llama) | Quien llama |
| `$fp-4` | `$ra` | Prólogo |
| `$fp-8` | Estado guardado (reservado) | — |
| `$fp-12…` | Locales | La función |
| `$fp-(frame_size + 8·(k+1))` | Ranura del temporal `tk` (derrames) | `getReg` |
| debajo de los temporales | Un word por cada `$s` usado | Prólogo |

**Decisión:** el valor de retorno va en `$fp+4` (no en `$v0`), porque el
enunciado pide manejarlo "conforme al diseño de registros de activación".

Tamaño del marco en MIPS: `marco_mips = total_frame_size() + 4 · (número de
$s usados)`. Los `$s` se conocen solo al terminar la función, así que el
cuerpo se traduce primero y el prólogo se arma al final.

### 4.4 Secuencia de llamada (detalle exacto)

Un `call` del TAC siempre viene en un grupo seguido: `param*`, `link?` y
`call`/`callv` (la Fase 2 del TAC junta todos los `param`). La traducción
trata el grupo entero de una vez:

```
# quien llama, para  param a / param b / link t0 / t1 = call f, 2
    (getReg guarda los $t vivos después de la llamada en sus ranuras)
    addiu $sp, $sp, -24          # 16 + 4·n
    sw    <a>,   16($sp)
    sw    <b>,   20($sp)
    sw    <t0>,  12($sp)         # enlace de acceso (sin link: no se escribe)
    sw    $fp,    0($sp)         # enlace de control
    jal   _f                     # callv: jalr <registro del destino>
    lw    <t1>,   4($sp)         # valor de retorno (si hay destino)
    addiu $sp, $sp, 24

# la función f
_f:
    move  $fp, $sp
    sw    $ra, -4($fp)
    addiu $sp, $sp, -<marco_mips>
    sw    $s0, -<total_frame_size + 4>($fp)    # uno por cada $s usado
    ...cuerpo...
    # return x  →   sw <x>, 4($fp)  y  j _f.0ret
_f.0ret:
    lw    $s0, -<total_frame_size + 4>($fp)
    lw    $ra, -4($fp)
    move  $sp, $fp
    lw    $fp, 0($fp)
    jr    $ra
```

`main` empieza con `move $fp, $sp` y `addiu $sp, $sp, -<marco_mips>`, y
termina con `li $v0, 10` / `syscall`.

### 4.5 Etiquetas y datos
- Todo nombre que viene del programa lleva el prefijo `_`: `_contador`,
  `_contador.sumar`, `_Animal.hablar`, `_Animal.0init`. Así un usuario no
  puede chocar con una instrucción de MIPS (`add`, `j`, `b`…) ni con el
  compilador. El código de nivel de programa es `main` (un `main` del
  usuario sería `_main`).
- Epílogo de cada función: `<etiqueta>.0ret` (empieza con dígito, igual que
  `0init`, así que no choca con una función anidada).
- Etiquetas del TAC: `L0`, `L1`… sin cambios.
- Textos: `S0`, `S1`… (`.asciiz`; si tienen caracteres no ASCII, `.byte` con
  los bytes UTF-8 y un `0` al final).
- Vtables: `vt_Animal: .word _Animal.hablar, …` (`.align 2` antes).
- Área estática: `G: .space <static_size>` (mínimo 4). Una global es
  `G+offset`.
- Rutinas de apoyo: `rt_*`; datos de apoyo: `rt_try_top`, `rt_exception`,
  `rt_true`, `rt_false`, `rt_null`, `rt_msg_*`.
- **Verificar en la Fase 0** que MARS acepta `.` dentro de una etiqueta. Si
  no, se reemplaza por `__` en `names.rs` (un solo lugar).

### 4.6 Rutinas de apoyo: convención ligera
- Reciben argumentos en `$a0–$a2` y devuelven en `$v0`.
- Solo pueden modificar `$a0–$a3`, `$v0`, `$v1`, `$t8`, `$t9` y `$ra`. Así no
  pisan ningún registro de `getReg` y quien las llama **no** tiene que
  guardar nada.
- Si una rutina llama a otra (`rt_concat` llama a `rt_alloc`), guarda su
  `$ra` en la pila.

### 4.7 Errores en tiempo de ejecución y `try`
- `try_begin L` apila un registro de 48 bytes **en la pila del programa**:
  `[anterior, &L, $fp, $s0…$s7, libre]`, y `rt_try_top` apunta a él.
  `try_end` lo desapila. (El TAC ya emite un `try_end` antes de cada
  `return` o `break` que sale de un `try`, así que la pila queda balanceada.)
- Al fallar (`rt_error` con la dirección del mensaje en `$a0`):
  - si `rt_try_top` es 0: imprime `Error en tiempo de ejecucion: <mensaje>`
    y termina con código 1 (`syscall` 17);
  - si no: desapila, restaura `$fp`, `$s0–$s7` y `$sp`, guarda el mensaje en
    `rt_exception` y salta a `L`. `x = exception` lee `rt_exception`.
- Mensajes fijos (los mismos del intérprete), sin acentos para que sean
  ASCII: `indice fuera de rango`, `division por cero`.

### 4.8 Supuestos
- `float` no existe en la gramática: `+f`, `-f`, `*f`, `/f`, `minusf`,
  `(float)` y `print_float` dan el error **`M001`** ("operación de punto
  flotante no soportada en MIPS"). No se genera `.asm`.
- `print_ref` de un valor no nulo imprime su dirección como entero; el
  intérprete imprime `ptr(N)`. Por eso las pruebas que comparan salidas no
  imprimen referencias no nulas.
- `div` / `rem` nativos de MIPS: `-2147483648 / -1` da `-2147483648` en MARS
  y en el intérprete (los dos envuelven).
- La memoria de `sbrk` empieza en cero (los campos sin inicializar valen 0).

### 4.9 Códigos de error de esta fase
| Código | Significado |
|---|---|
| `M001` | Instrucción de punto flotante (no soportada) |
| `M900` | Instrucción todavía no traducida (solo mientras se construye; al final no debe quedar ninguno) |

Si el TAC no se generó (errores léxicos, sintácticos, semánticos o `C###`),
tampoco se genera MIPS: esos errores ya se reportan.

---

## 5. Reparto, archivos y orden

| Integrante | Fases | Lo que se evalúa de su parte |
|---|---|---|
| **A** | **0** Cimientos y asignación por pila · **1** Traducción de instrucciones · **4** `getReg()` | Asignación de registros y generación |
| **B** | **2** Secuencias de llamada y retorno · **3** Rutinas de apoyo y `try` | Secuencias de llamada y retorno |
| **C** | **5** Simulador, CLI, API e IDE · infraestructura de la **6** | Ejecución en el simulador, batería, IDE |

| Integrante | Archivos (cada quien solo toca los suyos) |
|---|---|
| A | `src/codigo_objetivo/{mod.rs, asm.rs, names.rs, data.rs, regs.rs, liveness.rs, instr.rs}`, `tests/mips_fase{0,1,4}_tests.rs` |
| B | `src/codigo_objetivo/{calls.rs, runtime.rs, runtime.asm}`, `tests/mips_fase{2,3}_tests.rs` |
| C | `src/codigo_objetivo/sim.rs`, `tools/Mars4_5.jar`, `.github/workflows/ci.yml`, `src/api/*`, `src/bin/cpsc.rs`, `frontend/IDE-lite/*`, `tests/mips_battery_tests.rs`, `tests/api_mips_tests.rs`, `workspace/mips/**` |

**Orden:**

```
semana 1   A: Fase 0 ──────────────► (interfaz fija)
           B: runtime.asm a mano (no depende de nada) ──┐
           C: sim.rs + MARS en el CI (no depende de nada)┤
semana 2   A: Fase 1 (instrucciones, con asignación por pila)
           B: Fase 2 (llamadas) y Fase 3 (engancha runtime.asm)
           C: CLI, API e IDE sobre generate() de la Fase 0
semana 3   A: Fase 4 (getReg reemplaza la asignación por pila;
              todas las pruebas deben seguir dando la misma salida)
           los tres: Fase 6 (batería y documentación)
```

- La Fase 0 incluye una **asignación por pila** completa y correcta (cada
  temporal vive en su ranura). Con eso B y C trabajan y prueban sin esperar
  a `getReg`, y además cubre el punto "usar la pila cuando no hay
  registros".
- Cada quien en **su rama** y con PR a `main`, como en el TAC. Ningún commit
  compartido.
- Antes de empezar: hacer commit del trabajo pendiente del TAC y confirmar
  §4 entre los tres.

---

## 6. Fases

Formato de cada fase: **Recibe** (lo que tiene que existir antes) →
**Hace** → **Entrega** (lo que la siguiente fase puede usar) → **Pruebas** →
**Lista cuando**.

### Fase 0 — Cimientos y asignación por pila (A)

**Recibe:**
- `TacProgram` de `intermedio::gen::generate` (§2).
- Las decisiones de §4.

**Hace:**
- `asm.rs`:
  - `enum Reg` (`Zero, V0, V1, A(0..=3), T(0..=9), S(0..=7), Sp, Fp, Ra`)
    con `Display` (`$t3`).
  - `enum AsmLine { Label(String), Instr { op: &'static str, args: Vec<String> }, Directive(String), Comment(String) }`.
  - `struct MipsProgram { data: Vec<AsmLine>, text: Vec<AsmLine>, stats: Vec<FunctionStats> }`
    con `Display` que produce el `.asm` completo (`.data`, `.text`,
    `.globl main`) y `to_json()`.
- `names.rs`: las reglas de §4.5 (`func_label`, `ret_label`, `vtable_label`)
  y `StringTable` (asigna `S0`, `S1`… sin repetir textos, y escapa `"`, `\`,
  saltos de línea y bytes no ASCII).
- `data.rs`: la sección `.data` (`G`, vtables, textos, datos de apoyo).
- `regs.rs`: la **interfaz** de asignación que usan las Fases 1–3, y su
  primera implementación, `StackAlloc`:

  ```rust
  pub trait RegAlloc {
      fn begin_function(&mut self, f: &TacFunction);
      fn begin_instr(&mut self, index: usize);
      /// Un registro que contiene el valor de `op` (emite el lw/li/la necesario).
      fn use_operand(&mut self, out: &mut Vec<AsmLine>, op: &Operand) -> Reg;
      /// El registro donde calcular el valor de `dst`.
      fn def_operand(&mut self, out: &mut Vec<AsmLine>, dst: &Operand) -> Reg;
      /// Después de calcular: si `dst` es variable o memoria, el sw.
      fn finish_def(&mut self, out: &mut Vec<AsmLine>, dst: &Operand, reg: Reg);
      fn end_instr(&mut self, out: &mut Vec<AsmLine>);
      /// En cada etiqueta y después de cada salto.
      fn block_boundary(&mut self, out: &mut Vec<AsmLine>);
      /// Antes de un call: guarda los $t vivos después de la llamada.
      fn before_call(&mut self, out: &mut Vec<AsmLine>);
      /// Después de un call: los valores cacheados de variables ya no sirven.
      fn after_call(&mut self);
      /// Los $s que usó la función (para el prólogo y el epílogo).
      fn saved_registers(&self) -> Vec<Reg>;
      fn stats(&self) -> FunctionStats;
  }
  ```

  `StackAlloc`: cada operando se carga en `$t0`, `$t1` o `$t2` (rotando)
  justo antes de usarlo, y cada resultado se guarda en seguida en su ranura
  o su variable. No usa `$s`.
- `mod.rs`:
  - `pub fn generate(program: &TacProgram) -> Result<MipsProgram, Vec<CodegenError>>`
    (y `generate_with(program, Allocator::Pila | Allocator::GetReg)` para
    comparar las dos).
  - Recorre `main` y cada función; por instrucción llama al traductor:
    `instr.rs` (A), `calls.rs` (B) o `runtime.rs` (B). Lo que todavía no
    tenga traducción da `M900` (como `try_gen_*` en el TAC).
  - Un `FunctionEmitter` con `emit(op, args)`, `label(..)`, `comment(..)`
    y el `RegAlloc` actual, que comparten las tres partes.
  - Comentario con la instrucción TAC original encima de su traducción
    (`# t0 = a + b`): sirve para depurar y para la presentación.

**Entrega:**
- `generate()` produce un `.asm` que **ensambla en MARS** para cualquier
  programa (con `M900` donde falte traducción).
- La interfaz `RegAlloc` y `FunctionEmitter` **fijas**: B y C se basan en
  ellas.
- `StackAlloc` correcto.

**Pruebas** (`tests/mips_fase0_tests.rs`):
- `.data` de `rubrica.cps`: `G` con su tamaño, `vt_Animal` con sus
  métodos, los textos sin repetir.
- Nombres: `_contador.sumar`, `_Animal.0init`, `_f.0ret`; un texto con
  comillas y con `í` bien escapado.
- `M001` para un `+f`.
- Con MARS (si está): un programa vacío ensambla y termina con código 0.

**Lista cuando:** la interfaz está en `main` y B y C la pueden usar.

### Fase 1 — Traducción de instrucciones (A) — `instr.rs`

**Recibe:** de la Fase 0, `FunctionEmitter` y `RegAlloc` (`StackAlloc`).

**Hace:** una traducción por instrucción, siempre pidiendo los registros a
`RegAlloc`:

| TAC | MIPS |
|---|---|
| `x = y` | `move` / `li` / `la` según `y`, luego `finish_def` |
| `x = y + z`, `-`, `*` | `addu`, `subu`, `mul` (con constante: `addiu` si cabe en 16 bits) |
| `x = y / z`, `%` | `div y, z` + `mflo` / `mfhi` (el TAC ya puso `check_zero` antes) |
| `x = y == z`, `!=`, `<`, `<=`, `>`, `>=` | `seq`, `sne`, `slt`, `sle`, `sgt`, `sge` |
| `x = minus y` / `x = not y` | `subu x, $zero, y` / `xori x, y, 1` |
| `L:` / `goto L` | etiqueta + `block_boundary` / `j L` |
| `if x goto L` / `ifFalse x goto L` | `bnez` / `beqz` |
| `if x relop y goto L` | `beq`, `bne`, `blt`, `ble`, `bgt`, `bge` (`streq`/`strne` los hace B) |
| `x = y[i]` / `x[i] = y` | `lw` / `sw` con `i(y)` si `i` es constante; si no, `addu $t8, y, i` primero |
| `x = &vt_C` | `la` |
| Operandos `Var`/`Mem` | `lw`/`sw` con `off($fp)`, `G+off` u `off(reg)` |

**Entrega:** todo el TAC de expresiones, control de flujo, listas y campos
traducido. Programas sin funciones, sin `print` ni `try` corren en MARS.

**Pruebas** (`tests/mips_fase1_tests.rs`):
- El `.asm` exacto de una aritmética, de un `if`/`else` y de un `while`
  (con `StackAlloc`).
- Envoltura de 32 bits en MARS (`2147483647 + 1`), `/` y `%` con negativos
  (mismo resultado que el intérprete).
- Acceso a una global, a una local y a un campo.

**Lista cuando:** `M900` solo queda en `param`/`call`/`return`/`print`/
`check_*`/`try`/`alloc`/`concat`/`streq`.

### Fase 2 — Secuencias de llamada y retorno (B) — `calls.rs`

**Recibe:**
- De la Fase 0: `FunctionEmitter`, `RegAlloc` (`before_call`,
  `after_call`, `saved_registers`) y `names::{func_label, ret_label}`.
- De la Fase 1: `use_operand`/`def_operand` ya probados.
- §4.3 y §4.4.

**Hace:**
- Prólogo y epílogo de cada función (§4.4), armados **después** de traducir
  el cuerpo, con `saved_registers()`. `main` con su inicio y el `exit`.
- El grupo `param*` / `link?` / `call` o `callv`:
  1. `before_call()`;
  2. reservar `16 + 4·n` bytes;
  3. guardar cada parámetro en `16 + 4·i($sp)`, el enlace de acceso en
     `12($sp)` y `$fp` en `0($sp)`;
  4. `jal <etiqueta>` o `jalr <registro>` (`callv`);
  5. `lw` del resultado desde `4($sp)` si hay destino;
  6. liberar los bytes; `after_call()`.
- `return x`: `sw x, 4($fp)` y `j <etiqueta>.0ret`. Una función sin
  `return` cae al epílogo.
- Enlaces de acceso: `link fp` pasa el `$fp` actual; `link t` pasa el
  registro de `t` (el TAC ya siguió los enlaces).

**Entrega:**
- Funciones, recursión, funciones anidadas (closures), métodos con `this`
  y despacho virtual corren en MARS.
- Para la Fase 4: la lista de puntos donde se llama a `before_call()`
  (la vida de los temporales la usa ahí).

**Pruebas** (`tests/mips_fase2_tests.rs`):
- El `.asm` exacto de una llamada con dos parámetros y su función.
- En MARS:
  - factorial recursivo (`factorial(12)`, `factorial(13)` con vuelta);
  - `contador`/`sumar` de `PLAN_TAC.md` §4 (enlace de acceso);
  - herencia con sobrescritura llamada desde una variable del tipo padre;
  - `$sp` y `$fp` iguales antes y después de mil llamadas (un ciclo que
    llama, sin desbordar la pila).

**Lista cuando:** no queda `M900` de llamadas.

### Fase 3 — Rutinas de apoyo y `try` (B) — `runtime.asm`, `runtime.rs`

**Recibe:**
- §4.6 y §4.7.
- De la Fase 0: `FunctionEmitter`, `data.rs` (B le agrega los datos de
  apoyo) y `RegAlloc`.
- De C (Fase 5): `sim.rs`, para probar las rutinas sueltas.

**Hace:**
- `runtime.asm`, escrito a mano y agregado al final del `.asm` con
  `include_str!` (solo las rutinas que el programa usa):

| Rutina | Entrada → salida | Qué hace |
|---|---|---|
| `rt_print_int` | `$a0` | `syscall` 1 + salto de línea (`syscall` 11) |
| `rt_print_str` | `$a0` = dirección | `syscall` 4 + salto de línea |
| `rt_print_bool` | `$a0` = 0/1 | `true` / `false` |
| `rt_print_ref` | `$a0` | `null` si es 0; si no, la dirección como entero |
| `rt_alloc` | `$a0` = bytes → `$v0` | `syscall` 9 |
| `rt_concat` | `$a0`, `$a1` → `$v0` | Mide, reserva con `rt_alloc` y copia |
| `rt_streq` | `$a0`, `$a1` → `$v0` = 0/1 | Compara byte a byte |
| `rt_check_bounds` | `$a0` = lista, `$a1` = índice | Si `índice < 0` o `≥ lista[0]`: `rt_error` con `rt_msg_index` |
| `rt_error` | `$a0` = mensaje | §4.7: salta al `catch` o termina con código 1 |

- `runtime.rs`, la traducción de las instrucciones que usan esas rutinas:
  - `print_int`/`print_str`/`print_bool`/`print_ref x` → `move $a0, x` +
    `jal rt_print_*`;
  - `x = alloc n` → `rt_alloc`;
  - `x = y concat z`, `x = y streq z`, `x = y strne z` (`strne` invierte el
    resultado) y `if y streq z goto L`;
  - `check_bounds a, i` → `rt_check_bounds`;
  - `check_zero x` → `beqz x, rt_err_div0` (en línea; `rt_err_div0` carga
    el mensaje y va a `rt_error`);
  - `try_begin L`, `try_end` y `x = exception` (§4.7).

**Entrega:** todo el conjunto de instrucciones traducido: **ningún `M900`**.
`rubrica.cps` corre en MARS con la misma salida que `cpsc --run`.

**Pruebas** (`tests/mips_fase3_tests.rs`):
- Cada rutina suelta en MARS con un programa de prueba escrito a mano.
- `print` de los cuatro tipos; `"a" + "b"`; `streq` en `if` y en `switch`.
- `try` dentro de una función llamada desde otra función con `try`
  (desapila el correcto); un error **sin** `try` termina con código 1 y el
  mensaje; un `return` dentro de un `try`.

**Lista cuando:** `workspace/tac/ok/*.cps` y `rubrica.cps` dan en MARS
exactamente la salida del intérprete.

### Fase 4 — `getReg()` (A) — `liveness.rs`, `regs.rs`

**Recibe:**
- De la Fase 0: la interfaz `RegAlloc`.
- De las Fases 1–3: todo traducido y probado con `StackAlloc`, y los puntos
  de llamada de la Fase 2.

**Hace:**
- `liveness.rs` (libro, §8.4 y §9.2.5):
  - bloques básicos (líderes: primera instrucción, etiquetas, la siguiente
    a un salto o a un `call`) y su grafo de flujo;
  - temporales vivos a la entrada y salida de cada bloque, y por
    instrucción;
  - `vivos_tras_llamada`: los temporales vivos después de algún `call`.
- `GetRegAlloc` en `regs.rs` (libro, §8.6):
  - **descriptor de registros**: qué temporal o variable tiene cada
    registro; **descriptor de direcciones**: dónde está cada valor
    (registro, ranura, memoria);
  - `getReg` para un temporal: si ya tiene registro, ese; si no, uno libre
    de su clase (`$s` si está en `vivos_tras_llamada`, `$t` si no); si no
    hay libre, **derrama** el temporal cuyo próximo uso está más lejos a su
    ranura (`sw` a `fp-(frame_size+8(k+1))`) y usa su registro;
  - libera el registro en cuanto el temporal muere;
  - variables: se cargan en un registro libre y se reutilizan dentro del
    bloque; cada escritura se guarda en memoria en ese momento (así al
    terminar el bloque no hay nada pendiente). En `after_call()` y en
    `block_boundary()` se olvidan;
  - `before_call()`: guarda en su ranura los `$t` vivos tras la llamada
    (los que no consiguieron `$s`) y los recarga cuando se usan;
  - `saved_registers()`: los `$s` usados;
  - comentarios en el `.asm` (`# t3 -> $s0`, `# derrame t5`) y
    `FunctionStats { registros_usados, derrames, s_guardados }`.
- `generate()` usa `GetRegAlloc` por defecto.

**Entrega:**
- La asignación de registros del enunciado.
- `stats` en `MipsProgram` para la API y el IDE.

**Pruebas** (`tests/mips_fase4_tests.rs`):
- Liveness exacta de un `while` y de un `if` (unitarias).
- `a + b*c - d` no usa la pila; una expresión con más de 8 valores vivos
  derrama (aparece el `sw` a la ranura).
- Un temporal vivo durante una llamada (`f(x) + g(y)`) queda en `$s` y la
  función lo guarda.
- **Las mismas salidas con `StackAlloc` y con `GetRegAlloc`** en toda la
  batería, y el `.asm` con `getReg` tiene menos `lw`/`sw`.

**Lista cuando:** toda la suite pasa con `GetRegAlloc`.

### Fase 5 — Simulador, CLI, API e IDE (C)

**Recibe:**
- Al empezar: nada (el ejecutor de MARS se prueba con un `.asm` escrito a
  mano).
- Para la CLI, la API y el IDE: `codigo_objetivo::generate` y
  `MipsProgram` de la Fase 0 (`Display`, `to_json`, `stats`).

**Hace:**
- `sim.rs`:
  - `pub fn mars_available() -> bool`;
  - `pub fn run_mars(asm: &str, timeout: Duration) -> Result<SimResult, SimError>`
    con `SimResult { stdout: String, exit_code: i32, stop: Option<SimStop> }`
    y `SimError::{NoJava, NoJar, Assemble(String)}`;
  - `SimStop` dice por qué MARS no dejó terminar al programa, y la salida
    impresa hasta ahí se conserva: `StepLimit` (`M902`) y `Timeout` (`M903`)
    son un posible ciclo infinito, `StackOverflow` (`M904`) una posible
    recursión infinita, `OutOfMemory` (`M905`) y `Fault` (`M906`) el resto;
  - escribe el `.asm` a un archivo temporal, corre el comando de §4.1,
    decodifica la salida como UTF-8 y corta a los 10 segundos.
- `tools/Mars4_5.jar` y, en `ci.yml`, `actions/setup-java` (Java 17) y
  `MIPS_REQUIRED=1`.
- CLI `cpsc`:
  - `--mips`: imprime la sección `MIPS`;
  - `-o salida.asm`: lo escribe a un archivo;
  - `--sim`: lo corre en MARS e imprime la salida;
  - `--pila`: usa `StackAlloc` (para comparar);
  - actualizar la ayuda y sus pruebas.
- API (`src/api/pipeline.rs`, `src/api/mod.rs`):
  - campos `mips` (texto) y `mips_stats` (JSON), cuando hay TAC;
  - `run_mips: true` → `mips_output` y `mips_exit_code` (o un problema si
    MARS no está en el servidor);
  - los `M###` van a `problems`.
- IDE (`frontend/IDE-lite/`):
  - pestaña **MIPS** con resaltado (registros, instrucciones, directivas,
    etiquetas, comentarios);
  - botón **⬇ DESCARGAR .asm** y botón **▶ EJECUTAR EN MARS**;
  - tabla de `mips_stats` (registros por función, derrames);
  - subir el `?v=` de los `<script>` en `index.html`.
- `Dockerfile.api`: agregar Java si la API desplegada debe ejecutar MARS
  (si no, el botón muestra que el servidor no lo tiene).

**Entrega:**
- `run_mars` para las pruebas de A y B y para la batería.
- Compilar a MIPS desde la consola y desde el IDE.

**Pruebas** (`tests/api_mips_tests.rs` y las de `cpsc`):
- `run_mars` con un `hola.asm` a mano (salida y código de salida), con un
  `.asm` que no ensambla (`SimError::Assemble`) y con cada `SimStop`.
- `/api/pipeline` devuelve `mips` para un programa válido y no lo devuelve
  si hay errores.
- `cpsc --mips -o` escribe el archivo.

**Lista cuando:** desde el IDE se escribe un `.cps`, se ve su MIPS y se
ejecuta.

### Fase 6 — Batería y documentación (los tres; cada quien su parte)

**Recibe:** las Fases 0–5 en `main`.

**Hace:**
- `tests/mips_battery_tests.rs` (C):
  - recorre `workspace/tac/ok/*.cps` y `workspace/mips/ok/*.cps`: genera el
    MIPS, lo corre en MARS y compara con `// salida:` **y** con la salida
    del intérprete;
  - recorre `workspace/mips/error/*.cps`: exige el `// error:` esperado y
    que no haya `.asm`; o, para errores en tiempo de ejecución, `// salida:`
    y `// codigo: 1`;
  - `rubrica.cps` completa;
  - una prueba que corre **siempre**, aunque no haya MARS: todo `.asm` de la
    batería tiene cada etiqueta usada definida, ninguna `$` en una etiqueta
    y ningún `M900`.
- Programas (cada quien los de su fase), uno por punto del enunciado:
  - A: `registros.cps` (expresión que derrama, valores vivos en una llamada),
    `aritmetica32.cps`, `control.cps`;
  - B: `llamadas.cps` (recursión, anidadas, muchos parámetros),
    `objetos.cps`, `excepciones.cps`, `textos.cps`;
  - C: los de `error/`: errores de compilación (sin `.asm`), punto flotante
    (`M001`), error sin `try` (código 1).
- Documentación:
  - **`CODIGO_OBJETIVO.md`** (nuevo): arquitectura del módulo, registros
    (§4.2), registro de activación y secuencia de llamada con un ejemplo
    real, `getReg` con un ejemplo antes y después, rutinas de apoyo, `try`,
    supuestos y errores. A escribe asignación e instrucciones, B llamadas,
    rutinas y `try`, y C simulador, CLI, API e IDE.
  - `README.md` y `GUIA_USO.md` (C): instalar Java, `cpsc --mips --sim`,
    abrir el `.asm` en MARS con interfaz, el IDE y cómo correr la batería.
  - `ARQUITECTURA.md` (A): el lugar de `codigo_objetivo` en el pipeline.
  - `ORGANIZACION.md` (A): "Fases futuras" → la fase de código objetivo
    está implementada.
  - `API_REFERENCE.md` (C): `mips`, `mips_stats`, `run_mips`.
  - Una nota de que el proyecto usa su propio generador léxico y LALR en
    lugar de ANTLR.

**Entrega:** lo que se presenta.

**Lista cuando:** `cargo test --no-fail-fast` pasa en el CI con
`MIPS_REQUIRED=1`.

---

## 7. Cómo se pasa el trabajo de una fase a otra

| De | A | Qué se pasa | Cómo se usa |
|---|---|---|---|
| TAC | Fase 0 | `TacProgram` | `codigo_objetivo::generate(&program)` |
| Fase 0 | 1, 2, 3 | `FunctionEmitter`, `RegAlloc`, `names`, `data` | Cada traductor emite con el emisor y pide los registros a la interfaz |
| Fase 0 | 5 | `generate`, `MipsProgram` (`Display`, `to_json`, `stats`) | CLI, API e IDE |
| Fase 1 | 2, 3 | `use_operand`/`def_operand` probados con cada tipo de operando | Los parámetros, el resultado y los argumentos de las rutinas son operandos |
| Fase 2 | 4 | Puntos de `before_call`/`after_call` | La vida de los temporales decide qué va a `$s` |
| Fase 3 | 6 | Ningún `M900` | La batería exige que no quede ninguno |
| Fase 4 | 5 | `FunctionStats` | Tabla del IDE |
| Fase 5 | 1–4, 6 | `run_mars` | Toda prueba que ejecuta MIPS |
| Intérprete | 1–6 | `cpsc --run` / `Vm::run` | El oráculo: MARS debe dar la misma salida |

**Regla de oro:** si dos fases discrepan sobre la salida de un programa, el
intérprete de TAC manda (ya se comporta como MIPS, §2). Si el intérprete
está mal, se arregla ahí y se avisa.

---

## 8. Riesgos conocidos

- **MARS y los acentos:** ver §4.1. Si `-Dfile.encoding` no basta, los
  mensajes fijos se escriben como `.byte` y la prueba compara bytes.
- **Puntos en las etiquetas:** ver §4.5 (un solo lugar para cambiarlo).
- **Pila desbordada** con recursión muy profunda: MARS falla con `se2`.
  Las pruebas no pasan de unas mil llamadas anidadas.
- **Windows Application Control** bloquea de vez en cuando un binario de
  prueba: borrar el `.exe` de `target/debug/deps` y volver a correr.
- **Memoria al correr toda la suite en Windows:** correr por archivos con
  `-j 2 -- --test-threads=2`. El CI corre en Linux.

---

## 9. Estado

| Fase | Estado |
|---|---|
| 0 — Cimientos y asignación por pila (A) | Hecha (sin MARS local) |
| 1 — Traducción de instrucciones (A) | Hecha (sin MARS local) |
| 2 — Secuencias de llamada y retorno (B) | Pendiente |
| 3 — Rutinas de apoyo y `try` (B) | Pendiente |
| 4 — `getReg()` (A) | Pendiente |
| 5 — Simulador, CLI, API e IDE (C) | Pendiente |
| 6 — Batería y documentación (los tres) | Pendiente |
