# `objetos_es` — prueba de agnosticidad del análisis semántico

Gramática de un lenguaje de objetos escrita **a propósito con otros nombres**
para demostrar empíricamente que `src/semantico` no está atado a
`compiscript.yalp`. Se ejecuta en `tests/gramatica_agnostica_tests.rs`.

## Archivos

- `examples/lexer/objetos_es.yal` — tokens en español
- `examples/grammar/objetos_es.yalp` — gramática + directivas semánticas
- `examples/source/objetos_es.txt` — programa válido (0 diagnósticos)
- `examples/source/objetos_es_errores.txt` — 8 errores semánticos

## Nada se llama igual

| compiscript.yalp | objetos_es.yalp |
|---|---|
| `THIS` / `DOT` / `NEW` | `PROPIO` / `PUNTO` / `CREAR` |
| `CLASS` / `FUNCTION` | `OBJETO` / `METODO` |
| `ID` | `IDENT` |
| `class_decl` / `func_decl` | `declaracion_objeto` / `declaracion_metodo` |
| `var_decl` / `param` / `bloque` | `campo` / `parametro` / `cuerpo` |
| `primary` / `atom` / `args` | `expresion_primaria` / `atomo` / `argumentos` |
| método `constructor` | método `iniciar` |
| `var x: integer` (tipo después) | `x: entero` (nombre primero) |

El código Rust del analizador **no cambia**: toda la diferencia vive en las
directivas `%ident` / `%declare` / `%scope` / `%type_of` / `%type_token` /
`%this` / `%member_access` / `%new` / `%call` / `%arg_list_symbol` /
`%constructor`.

## Qué prueba exactamente

El mismo `analyze()` resuelve sobre esta gramática:

- entorno de clase y miembros (`objeto Figura { area: entero; ... }`)
- acceso a miembros con `.`, en lectura y asignación
- **herencia**: `c.obtenerArea()` y `propio.area` resueltos subiendo de
  `Circulo` a `Figura`
- `propio` (el `this` de este lenguaje) dentro de un método, y su error
  correspondiente fuera de uno
- constructor por convención de nombre — acá `iniciar`, no `constructor` —
  con chequeo de aridad y de tipo de argumentos
- aridad y tipos de una invocación, tanto a método (`c.escalar(1,2,3)`) como
  a función libre (`duplicar(1,2)`)
- una anotación de tipo que nombra un objeto que nunca se declaró

Y emite **los mismos códigos de diagnóstico** (`S006`–`S014`) que produce
Compiscript ante los mismos errores conceptuales.

## El límite que este ejemplo dejó a la vista

Los nombres de *tokens* y *producciones* son libres, pero el lado derecho de
`%type_token` **no**: pertenece al vocabulario fijo del sistema de tipos
(`integer`, `float`, `string`, `bool`, `void`), porque nombra una variante del
enum `Type` de `src/semantico/types`, no algo de la gramática.

Al escribir la primera versión de esta gramática se puso `%type_token ENTERO_T
entero`; compiló y parseó igual, pero el tipo cayó en `Unknown` y **los dos
diagnósticos de tipo dejaron de emitirse en silencio** (`S006` y `S012`). Es un
modo de fallo real y silencioso a tener presente al escribir un `.yalp` nuevo.

## Correr

```
cargo test --test gramatica_agnostica_tests -- --nocapture
```
