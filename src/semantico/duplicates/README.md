# Semana 4 — `duplicates` (Vianka)

Este módulo comprueba declaraciones repetidas y datos declarados pero nunca
leídos. No reemplaza `functions` ni `flow`, y no elimina código muerto.

## Cómo funciona

1. Al declarar, `SymbolTable` consulta solo el ámbito actual y llama a
   `validate_declaration`. Si el nombre existe, conserva la primera declaración
   y reporta **S001** con las posiciones de ambas. Se aplica también a parámetros.
2. El analizador marca `Symbol.used` cuando visita una lectura real de un nombre.
   Consultar su tipo internamente o asignarle un valor **no** es una lectura.
3. Al cerrar cada ámbito (y al terminar el global), `unused_diagnostics` produce
   **W001**, una advertencia, para variables y parámetros no leídos.

Declarar el mismo nombre en otro ámbito es *shadowing*, no un duplicado. Leer el
nombre interior no marca el exterior; una lectura desde una closure sí marca
el símbolo exterior que realmente se resolvió. Las constantes cuentan como
variables. Los campos de clases/structs y el `this` sintético quedan excluidos:
los campos pueden utilizarse desde fuera después de analizar la clase.

## Configuración y límites

Los duplicados mantienen la validación existente. Para habilitar las nuevas
advertencias, añade antes de `%%` en el `.yalp`:

```yalp
%warn_unused
```

También se necesitan las directivas semánticas normales (`%ident`, `%declare`,
`%scope` y `%assign` cuando corresponda). No se adivina la semántica de cualquier
lexema: cada lenguaje describe sus declaraciones, ámbitos y asignaciones.
Sin `%warn_unused` se conserva el comportamiento anterior.

El análisis es estático: una lectura en una rama cuenta aunque esa rama nunca
se ejecute. No determina alcanzabilidad ni valores escritos y sobrescritos;
eso no es la tarea de este módulo. En la API se ejecuta con **LALR(1)/SLR(1)**,
no con LL(1), que transforma los nombres originales de las producciones.

## Probar desde el IDE sin cambiar sus gramáticas guardadas

1. Reconstruye el backend: desde la raíz, `docker compose up --build`.
2. Abre `http://localhost:4000` y carga `workspace/compiscript.yal`,
   `workspace/compiscript.yalp` y `workspace/duplicates_casos.txt`.
3. En el editor del `.yalp`, añade `%warn_unused` antes de `%%`.
   No necesitas pulsar SAVE: se usa el contenido del editor al ejecutar.
4. Selecciona LALR(1), pulsa RUN y ejecuta cada línea en el panel de casos.
   Cada línea del `.txt` es un programa completo e independiente.
   Al ejecutar un caso aislado, sus posiciones empiezan en la línea 1.
5. Consulta PROBLEMAS: `S001` aparece como ERR y `W001` como WRN.
   “Aceptado” se refiere al parser; revisa también los diagnósticos semánticos.

| Caso | Qué comprueba | Resultado esperado |
|---|---|---|
| 1 | Variable leída | Sin problemas |
| 2 | Variable duplicada | S001 |
| 3 | Shadowing con ambas variables leídas | Sin problemas |
| 4 | Variable sin leer | W001 |
| 5 | Variable que solo recibe asignaciones | W001 |
| 6 | Parámetro duplicado | S001 |
| 7 | Parámetros distintos y utilizados | Sin problemas |
| 8 | Parámetro sin leer | W001 |
| 9 | Variable capturada por una closure | Sin problemas |
| 10 | Variable sin leer en bloque anidado | W001 |
| 11 | Leer la variable interior, no la exterior | W001 |
| 12 | Clases, atributos, constructor y `this` | Sin problemas |
| 13 | Asignación: se lee origen, no destino | W001 |
| 14 | Duplicado con inicializador incompatible | S001, preserva el original |
| 15 | Integración con listas, operadores y `flow` | Sin problemas |

Prueba un caso fallido seguido del 1: los problemas anteriores deben limpiarse.

## Pruebas automáticas y recorrido de estudio

```powershell
cargo test --test duplicates_tests -- --nocapture
cargo test --all-targets
```

La prueba usa el mismo `.txt` y el mismo pipeline que el IDE, activando
`%warn_unused` en memoria. No modifica la gramática ni las pruebas preexistentes.
Para estudiar, empieza por `mod.rs`, sigue las llamadas a `duplicates` en
`../symbols/mod.rs` y `../analyzer/mod.rs`, y revisa
`../../../tests/duplicates_tests.rs` para los resultados esperados.
