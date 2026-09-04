# Batería de casos semánticos

`casos_semanticos.txt` cubre **cada regla semántica con un caso exitoso y uno
fallido**. Cada línea es un programa completo e independiente —un bloque
`{ ... }`— así que el IDE muestra una entrada por caso en el panel de TEST
CASES y se pueden ejecutar de a uno.

El mismo archivo lo usa `tests/bateria_semantica_tests.rs`, que lo analiza por
el mismo pipeline del IDE en LALR(1) y SLR(1) y exige que cada línea produzca
**exactamente** el diagnóstico de su regla: ni un error derivado de más, ni uno
de menos.

## Cómo correrla desde el IDE

1. Levanta el backend: desde la raíz, `docker compose up --build`.
2. Abre `http://localhost:4000` y carga `workspace/compiscript.yal`,
   `workspace/compiscript.yalp` y `workspace/casos_semanticos.txt`.
3. Selecciona LALR(1) o SLR(1) y pulsa **RUN** para compilar la gramática.
4. Pulsa **PARSEAR** y ve seleccionando cada caso en el panel de la izquierda.
5. Mira la pestaña **PROBLEMAS**: los `S###` salen como ERR y los `W###` como
   WRN. El gutter del editor marca la línea en rojo o amarillo.

Un `.cps` multilínea, en cambio, se carga y compila **entero**: el panel de
casos por línea solo aplica a los `.txt` de batería como este.

## Qué comprueba cada caso

| # | Regla | Esperado |
|---|---|---|
| 1 | Declarar y leer una variable | ✅ sin problemas |
| 2 | Variable no declarada | `S002` |
| 3 | Redeclaración en el mismo ámbito | `S001` |
| 4 | Asignación a una constante | `S005` |
| 5 | Aritmética válida entre enteros | ✅ |
| 6 | Inicializador incompatible con el tipo declarado | `S006` |
| 7 | Aritmética entre `integer` y `string` | `S015` |
| 8 | Atributo de clase accedido con `.` | ✅ |
| 9 | Miembro inexistente | `S010` |
| 10 | Clase desconocida en una anotación de tipo | `S007` |
| 11 | Clase padre inexistente | `S008` |
| 12 | `this` fuera del ámbito de una clase | `S009` |
| 13 | Constructor invocado correctamente | ✅ |
| 14 | Aridad incorrecta del constructor | `S011` |
| 15 | Tipo de argumento incorrecto del constructor | `S012` |
| 16 | Llamada correcta a una función libre | ✅ |
| 17 | Aridad incorrecta en la llamada | `S013` |
| 18 | Tipo de argumento incorrecto en la llamada | `S014` |
| 19 | `return` con tipo distinto al declarado | `S016` |
| 20 | `return` sin valor en función tipada | `S017` |
| 21 | `return` con valor en un procedimiento | `S018` |
| 22 | `return` fuera de toda función | `S019` |
| 23 | Función anidada que captura su entorno (closure) | ✅ |
| 24 | Literal de struct correcto y acceso a campo | ✅ |
| 25 | Campo de struct mal tipado | `S022` |
| 26 | Campo de struct faltante | `S023` |
| 27 | Campo de struct repetido | `S024` |
| 28 | `while` con condición booleana | ✅ |
| 29 | Condición que no es booleana | `S025` |
| 30 | `break` fuera de un bucle | `S026` |
| 31 | `continue` fuera de un bucle | `S027` |
| 32 | Operador lógico sobre booleanos | ✅ |
| 33 | Operando no booleano en un operador lógico | `S028` |
| 34 | Comparación entre tipos incompatibles | `S029` |
| 35 | Operando inválido para un operador unario | `S030` |
| 36 | El nombre de una función no es un valor | `S031` |
| 37 | Literal de lista homogéneo e indexado | ✅ |
| 38 | Elementos heterogéneos en el literal | `S032` |
| 39 | Índice que no es entero | `S033` |
| 40 | Se indexa algo que no es un arreglo | `S034` |
| 41 | `switch` con un `case` compatible | ✅ |
| 42 | `case` incompatible con el discriminante | `S035` |
| 43 | `foreach` sobre algo que no es una colección | `S036` |
| 44 | Código inalcanzable tras un `return` | `W002` |
| 45 | Se invoca algo que no es una función | `S020` |

## Baterías relacionadas

- `duplicates_casos.txt` — 15 casos de declaraciones duplicadas y símbolos
  nunca leídos (`S001`/`W001`). Necesita `%warn_unused` en el `.yalp`; ver
  `src/semantico/duplicates/README.md`.
- `flujo_ok.cps` / `flujo_errores.cps` / `flujo_ambitos.cps` — control de flujo
  completo como programas `.cps` de verdad.
- `codigo_muerto.cps` / `codigo_muerto_corte.cps` — código inalcanzable.
- `clases_ok.cps` / `clases_errores.cps` — clases, herencia y constructores.

## Códigos sin caso

`S003` (cerrar el ámbito global) y `S004` (constante sin inicializador) no son
alcanzables desde esta gramática: el primero es un invariante interno de la
pila de ámbitos, y el segundo lo impide la propia sintaxis, porque
`const_decl` exige el `= expr`.

`S021` (falta la firma del invocado) tampoco: todo símbolo invocable recibe su
firma en `enter`, antes de recorrer su cuerpo —es lo que permite validar una
llamada recursiva—, así que un símbolo sin firma nunca es una función. Ese caso
lo cubre `S020`, y `S021` queda como red de seguridad de
`functions::validate_call`, probada en sus propios tests unitarios.
