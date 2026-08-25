# `pascalito` — prueba end-to-end sobre una gramática de otra *forma*

Tercera gramática de prueba, escrita **desde cero** en un lenguaje inventado de
estilo Pascal/Ada. Se ejecuta en `tests/pascalito_tests.rs`.

## Qué prueba, que las otras no

`objetos_es` demuestra que los **nombres** de tokens y producciones son libres:
es compiscript renombrado token por token. `pascalito` demuestra que la
**forma** también lo es.

| | compiscript / objetos_es | pascalito |
|---|---|---|
| Bloques | `{ ... }` | `is ... end` — **no hay llaves en el lenguaje** |
| Asignación | `=` | `:=` |
| Condicional | `if (e) { ... }` | `if e then ... end` |
| Bucle | `while (e) { ... }` | `loop e do ... end` |
| Bloque suelto | `{ ... }` | `do ... end` |
| Literal de registro | `Punto { x: 1 }` | `Punto[ x := 1 ]` — corchetes, `:=` |
| Comentario | `//` , `#` | `--` |
| `this` / `new` / constructor | `this` / `new` / `constructor` | `self` / `make` / `init` |
| `return` / `print` | `return` / `print` | `give` / `show` |

La gramática compila **LALR sin conflictos** (169 estados) pese a que el
literal de registro y el operador de asignación comparten el token `:=`, y a
que `%new` y `%struct_literal` apuntan a la **misma** producción (`atomo`) —
se distinguen por la forma del nodo, no por configuración extra.

## Archivos

- `examples/lexer/pascalito.yal` — tokens
- `examples/grammar/pascalito.yalp` — gramática + directivas semánticas
- `examples/source/pascalito.txt` — programa válido, **0 diagnósticos**
- `examples/source/pascalito_errores.txt` — 22 diagnósticos

## Qué cubre el programa válido

Todo lo implementado, en un solo archivo:

- **Registros**: declaración con campos tipados, literal con campos nombrados,
  acceso a campo, y un registro **anidado** dentro de otro
  (`Caja[ esquina := Punto[ x := 1, y := 2 ], alto := 10 ]`, y `caja.esquina.y`)
- **Clases**: atributos, `self`, constructor `init`, **herencia**, acceso a
  miembros heredados
- **Funciones**: tipos de argumento posicionales, tipo de retorno, recursión, y
  un procedimiento sin tipo declarado con `give` vacío
- **Closures**: una función anidada que captura una variable de la función
  encerradora
- **Tipos y ámbitos**: inferencia, constantes, aritmética, y un bloque anidado
  cuya variable tapa a la de afuera

## Qué cubre el archivo de errores

Emite **los mismos códigos** que las otras dos gramáticas ante los mismos
errores conceptuales: `S001`, `S002`, `S005`–`S019`, `S022`–`S024`. Son todos
los que el analizador puede producir desde código fuente. Quedan fuera:

- `S003` — invariante interna (no se puede cerrar el ámbito global), inalcanzable
- `S004` — constante sin inicializador: la sintaxis de `let` lo exige, igual que
  en compiscript
- `S020`/`S021` — sin productores por diseño (el walker resuelve el llamado por
  su cuenta antes de validar)

## Un hueco real que este ejemplo destapó

Al escribirlo apareció que **`S008` (herencia de una clase inexistente) no lo
emitía nadie**: la variante y su código existían, pero ninguna ruta los
producía. Fallaba igual en las tres gramáticas. Se corrigió con una pasada
diferida —el mismo mecanismo que ya validaba las anotaciones de tipo— para que
declarar el padre *después* de la hija siga siendo válido.

## Correr

```
cargo test --test pascalito_tests
```
