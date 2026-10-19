# Análisis de cumplimiento — Proyecto 2: Generación de Código Intermedio (TAC)

Revisión punto por punto del enunciado (`Proyecto_2__Compiladores.pdf`, sept. 2026)
contra la rama `fase3-tac` (ya integrada en `main`). Leyenda:
✅ cumple · ⚠️ cumple con observación · ❌ falta.

---

## 1. Diseño del Código Intermedio (25 pts)

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Conjunto de instrucciones de tres direcciones (asignación, binarias, unarias, copia, saltos cond./incond., etiquetas, llamadas, acceso indexado) | ✅ | `src/intermedio/tac.rs:185` (`enum Instr`): `Copy`, `Binary`, `Unary`, `Label`, `Goto`, `IfTrue`, `IfFalse`, `IfRel`, `Param`, `Link`, `Call`, `CallV`, `Return`, `Load`/`Store` (indexado `x = y[i]`, `x[i] = y`), `Alloc`, `AddrOf`, `Print`, `CheckBounds`, `CheckZero`, `TryBegin`/`TryEnd`/`GetException`. Forma de cuádruplos (§6.2 del libro). |
| Sintaxis consistente | ✅ | Una sola implementación `Display` en `tac.rs` produce todo el texto; los tests fijan el TAC exacto (`tests/tac_fase1_tests.rs`, `tac_fase2_tests.rs`). |
| Sintaxis documentada, con ejemplos y supuestos | ⚠️ | `CODIGO_INTERMEDIO.md` **solo tiene la sección de funciones, clases y listas** (dice textualmente que las secciones de instrucciones/expresiones/control de flujo y arquitectura/uso "se agregan en la Fase 4"). El diseño completo sí existe pero está en `PLAN_TAC.md §4` (tabla de instrucciones, convención de variables, registro de activación, reciclaje, supuestos) y en el doc-comment de `tac.rs`. **Es el riesgo principal de la nota: 25 pts dependen de este documento.** |

## 2. Expresiones y Asignaciones

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Aritméticas `+ - * /` con precedencia y asociatividad | ✅ | `src/intermedio/gen/expr.rs`. La precedencia/asociatividad la da la gramática LALR (el árbol ya viene con la forma correcta); el generador recorre en post-orden. Además `%`, `(float)` y `concat`. `check_zero` antes de `/` y `%`. Test: `precedencia_y_reciclaje_de_temporales` (`tac_fase1_tests.rs:77`). |
| Lógicas `&& \|\| !` | ✅ | `gen/expr.rs`, `gen_cond`: código de saltos con **cortocircuito** (§6.6); valor 0/1 cuando se usa como expresión. Caso en `workspace/tac/ok/control_flow.cps` (`false && (10/0 > 0) \|\| true` no divide). |
| Comparaciones `== != < <= > >=` | ✅ | `gen/expr.rs`; en condiciones se emite `if x relop y goto L` (`IfRel`). |
| Declaraciones y asignaciones de variables y constantes | ✅ | `gen/stmt.rs:73` (`try_gen_declaration`, valor por defecto según tipo) y `:109` (`try_gen_assignment`, asignación directa sin temporal extra). `const` se traduce igual; la inmutabilidad la garantiza el semántico (S005). |

## 3. Control de Flujo

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| `if/else` | ✅ | `gen/stmt.rs:162` |
| `while` | ✅ | `gen/stmt.rs:193` |
| `do-while` | ✅ | `gen/stmt.rs:215` |
| `for` | ✅ | `gen/stmt.rs:241` (+ `foreach` en `:282`) |
| `switch` | ✅ | `gen/stmt.rs:361`, con caída entre casos como TS. |
| `break` / `continue` a la etiqueta del bucle contenedor | ✅ | `gen/stmt.rs:472-485` (`gen_loop_body`, `gen_jump`) con pila `self.jumps`; `continue` dentro de `switch` salta al bucle de afuera; un `break` dentro de `try` emite `try_end`. Tests: `while_do_while_y_for_con_break_y_continue`, `switch_cae_entre_casos_y_continue_sigue_al_bucle_de_afuera`. |
| Extra (no pedido) | ➕ | `try/catch` con `try_begin/try_end/exception`. |

Nota: la forma de cada construcción no está hardcodeada; la da la directiva `%flow` del `.yalp` (`src/intermedio/spec.rs`).

## 4. Funciones y Procedimientos

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Definición, paso de parámetros, llamadas, retorno | ✅ | `gen/func.rs`: `func f nivel N marco M … endfunc`, `param`, `x = call f, n`, `return x`. Todos los argumentos se evalúan antes de emitir sus `param` (test `los_argumentos_de_una_llamada_anidada_no_se_entrelazan`). |
| Recursión | ✅ | `factorial` en `workspace/tac/ok/functions.cps` (ejecuta y da 120); tests `la_recursion_llama_a_la_funcion_sin_enlace` y `la_recursion_de_una_funcion_anidada_sigue_un_enlace`. |
| Funciones anidadas y closures (acceso al entorno de definición) | ✅ ⚠️ | **Enlaces de acceso** (§7.3 del libro): `link fp` / `link t` según `Access::{Static, Local, NonLocal{hops}}`, y lectura de variables de la función externa con `t = fp[12]; t[-12]`. Verificado: `contador.incrementar` modifica `total` del marco de `contador`. Observación: es un modelo de enlaces de acceso sobre la pila; no hay closures que **escapen** (devolver una función), porque la gramática de Compiscript no tiene valores de tipo función. Conviene escribirlo como supuesto en la doc. |

## 5. Clases y Objetos

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Creación de objetos e invocación del constructor | ✅ | `gen/objects.rs`: `t = alloc n`, `t[0] = &vt_C`, `call C.$init` (inicializa campos, encadena al padre), `call C.constructor` (el más cercano en la herencia). |
| Acceso a atributos | ✅ | `t[offset]` con el nombre del campo como comentario; offsets de `LayoutReport::classes`. |
| Invocación de métodos y `this` | ✅ | `this` es el primer `param` (`fp[16]`); despacho virtual por vtable: `t = obj[0]; t = t[slot*4]; x = callv t, n`. Campos/`this` desde una función anidada dentro de un método siguen los enlaces (`un_campo_usado_desde_una_funcion_anidada_en_un_metodo_sigue_los_enlaces`). |

## 6. Listas

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Creación de listas | ✅ | `gen/lists.rs`: `alloc 4+n·4`, cabecera de longitud y un `Store` por elemento. |
| Acceso por índice | ✅ ⚠️ | `check_bounds a, i` + `Load` con `4 + i×4`. Observación: **no existe asignación `a[i] = x`** porque la gramática no la permite (documentado en `CODIGO_INTERMEDIO.md` "Supuestos y límites"). El enunciado solo pide creación y acceso, así que cumple. |

## 7. Variables Temporales

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Algoritmo de asignación | ✅ | `src/intermedio/temps.rs` (`TempPool`, uno por función): `new_temp()` entrega el número libre más bajo. |
| Reciclaje al consumir el valor | ✅ | `release(t)` se llama después de emitir la instrucción que consume el temporal (40 llamadas en `gen/`). Se reporta por función `temps_requested` vs `max_temps`; `a + b * c - d` usa 2 temporales. Tests: `precedencia_y_reciclaje_de_temporales`, `el_reciclaje_deja_un_solo_temporal_vivo`. |

## 8. Tabla de Símbolos y Entornos de Ejecución (10 pts)

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Direcciones/desplazamientos, tamaños, etiquetas por símbolo | ✅ | `semantico::storage` calcula `area` (`G`/`fp`/`obj`), `offset`, `size`, `nesting_level`; `Symbol::label` da la etiqueta calificada (`contador.sumar`). Presentación en `src/intermedio/symtab.rs` (`ExtendedSymbolTable { symbols, frames, classes }`). |
| Registros de activación (parámetros, locales, temporales, valor y dirección de retorno, enlaces de control y de acceso) | ✅ | Layout fijo documentado en `PLAN_TAC.md §4`: params `fp+16…`, enlace de acceso `fp+12`, valor de retorno `fp+4`, enlace de control `fp+0`, `$ra` `fp-4`, estado `fp-8`, locales desde `fp-12`, temporales en ranuras de 8 bytes. `extend_layout` devuelve a la tabla las ranuras de temporales de cada marco. |
| La tabla interactúa con cada fase | ✅ | La construye el semántico, el generador la consulta (acceso/offsets/etiquetas) y la complementa (`extend_layout`), y la VM (`vm.rs`) la usa para ejecutar. |

## 9. Entrada / Salida

| Requisito | Estado | Dónde / cómo |
|---|---|---|
| Entrada `.cps` | ✅ | `cargo run --bin cpsc -- archivo.cps [--run]`; el IDE carga `.cps`; ejemplos en `workspace/tac/`. |
| Salida: TAC | ✅ | Sección `TAC` del CLI; pestaña **TAC** del IDE; `tac` y `tac_quads` en la respuesta de `/api/pipeline`. |
| Salida: reporte de errores | ✅ | Formato `[err S016] archivo.cps:18:3: mensaje` (código, tipo, línea y columna). Con errores **no se genera TAC** (verificado con `workspace/tac/error/semantic_errors.cps`). |
| Salida: estado de la tabla de símbolos extendida | ✅ | Sección `TABLA DE SÍMBOLOS EXTENDIDA` del CLI (JSON con símbolos, marcos, clases) y pestaña **SÍMBOLOS** del IDE. |

## 10. Funcionamiento del programa (pasos 1-6)

| Paso | Estado | Observación |
|---|---|---|
| 1. Cargar/escribir `.cps` en el IDE | ✅ | `frontend/IDE-lite/`. |
| 2. Léxico y sintáctico **(ANTLR)** | ⚠️ | **No se usa ANTLR**: el lexer y el parser LALR son generados por el propio proyecto desde `compiscript.yal`/`.yalp`. Está justificado en `ARQUITECTURA.md` ("por qué no se usó ANTLR"). Hay que defenderlo en la presentación o confirmar con el catedrático que es aceptable. |
| 3. Semántico + tabla de símbolos | ✅ | `src/semantico/`. |
| 4. Si no hay errores, generar TAC consultando/complementando la tabla | ✅ | Pipeline en `src/api/`. |
| 5. Mostrar TAC o errores con tipo y ubicación | ✅ | Pestañas TAC y PROBLEMAS (fase `C` = TAC, `R` = runtime). |
| 6. Ejecutar la batería de pruebas | ✅ ⚠️ | `cargo test`. Desde el IDE se pueden abrir los casos de `workspace/tac/`, pero la batería no se lanza con un botón; se corre por terminal. |

## 11. Entregables

| Entregable | Estado | Observación |
|---|---|---|
| Repo GitHub con commits individuales | ✅ | Autores: Ricgo/Ricgo01 (Fase 0-1), Javier (Fase 2), Vianka (Fase 3: VM, API, IDE, batería). Cada fase tiene commits de su integrante. |
| Batería de pruebas exitosa y fallida | ✅ | `tests/tac_battery_tests.rs` recorre `workspace/tac/ok/*.cps` (compara TAC/`// salida:` ejecutando la VM) y `workspace/tac/error/*.cps` (léxico, sintáctico, semántico: exige los `// error:` y que no haya TAC). Más `tac_fase0/1/2_tests.rs`, `api_fase3_tests.rs`. `cargo test --no-fail-fast` (2026-10-18): **474 pasan, 0 fallan**, 39 suites. |
| Documentación de arquitectura y de cómo ejecutar | ⚠️ | `ARQUITECTURA.md §8` cubre el traspaso al generador, pero **`README.md` todavía dice "Código intermedio y código objetivo siguen en el roadmap"** y no menciona `cpsc`. `GUIA_USO.md` no explica el TAC. `PLAN_TAC.md §6` sigue marcando Fase 3 y 4 como "Pendiente". |
| Documentación detallada del lenguaje intermedio con ejemplos y supuestos | ⚠️ | Ver punto 1: falta completar `CODIGO_INTERMEDIO.md`. |
| IDE funcional para escribir y compilar Compiscript | ✅ | `docker compose up` → IDE :4000, API :8080; pestañas TAC, SÍMBOLOS, PROBLEMAS y botón ejecutar. |

## 12. Extra: más allá de lo pedido

- Intérprete del TAC (`src/intermedio/vm.rs`, ~2900 líneas) que ejecuta el código y valida la salida esperada.
- Errores en tiempo de ejecución (`check_bounds`, `check_zero`) y `try/catch`.
- Generador independiente de la gramática: la estructura de control sale de `%flow`/`%print` en el `.yalp`.
