# Módulo `flow`

`flow` valida el control de flujo después de que el lexer y el parser
construyen el árbol sintáctico. No reconoce palabras reservadas por sí mismo:
la gramática `.yalp` le indica qué producciones representan condiciones,
bucles, `break` y `continue`.

## Qué valida

- Una condición conocida debe tener tipo `bool`.
- `break` y `continue` necesitan un bucle en la función actual.
- Una función anidada no puede saltar hacia el bucle de la función exterior.
- `return` continúa siendo responsabilidad del módulo `functions`.

## Pila de contexto

El analizador registra entradas y salidas de funciones y bucles:

```text
Loop
└── Function
    └── Loop
```

Al validar un salto, la pila se recorre desde adentro hacia afuera. Un `Loop`
lo permite, pero una frontera `Function` detiene la búsqueda. Por eso el bucle
exterior del ejemplo no puede recibir saltos desde la función anidada.

## Directivas `.yalp`

```yalp
%condition if_stmt expr
%condition do_while_stmt 4
%condition for_stmt      4
%loop while_stmt
%loop for_stmt
%loop foreach_stmt
%break break_stmt
%continue continue_stmt

%switch switch_stmt 2
%case   switch_case 1
```

En `%condition`, `%switch` y `%case`, el último valor puede ser el nombre del
hijo que contiene la expresión o su índice dentro de la producción. Los nombres
son propios de cada gramática; el código Rust no depende de Compiscript.

El índice hace falta cuando la expresión no es "el primer hijo con tal
símbolo": en un `do-while` la condición va después del cuerpo, y en un `for` es
el SEGUNDO hijo `expr` — buscar por símbolo devolvería el inicializador.

### `switch`: por qué no usa `%condition`

Un `switch` no selecciona sobre un booleano sino sobre un entero o una cadena,
así que exigirle una condición booleana lo volvería inservible. `%switch` marca
el discriminante y `%case` el valor de cada rama, y lo que se valida es la
**compatibilidad entre ambos** (`S035`), delegada en `types::resolve_assignment`
para no abrir una segunda tabla de coerciones. La rama `default` se declara
como una producción distinta justamente porque no lleva valor con el cual
comparar.

Además, un `%switch` abre un contexto donde **`break` es válido pero
`continue` no**: en un `switch` el `break` termina la rama —su uso idiomático
en TypeScript, el lenguaje del que Compiscript es subconjunto—, mientras que
`continue` sigue exigiendo un bucle real al cual volver. Por eso
`ContextKind::Switch` es una variante propia y no reusa `Loop`.

### Dónde se validan las condiciones

En `exit`, no en `enter`. Una condición puede depender de lo que declare un
hermano a su izquierda dentro de la misma producción: en
`for (let i = 0; i < 3; ...)` la `i` de la condición la declara el
inicializador. En `enter` todavía no se recorrió ningún hijo, así que `i` no
existía, la condición se tipaba como no resoluble y el `S025` se perdía en
silencio. En `exit` los hijos ya pasaron y el ámbito que abrió el nodo sigue
vivo.

## Archivos relacionados

- `mod.rs`: errores, validación de condiciones y pila de contexto.
- `../analyzer/mod.rs`: aplica las reglas mientras recorre el árbol.
- `../spec/mod.rs`: convierte las directivas en `FlowSpec`.
- `../../sintactico/gramatica/grammar.rs`: lee las directivas del `.yalp`.
- `../../../tests/control_flow_tests.rs`: pruebas de integración.

## Cómo estudiarlo y probarlo

Empieza por `validate_condition`, sigue con `FlowContext` y finalmente busca
`spec.flow` en el analizador para observar cuándo se apilan los contextos.

```powershell
cargo test semantico::flow -- --nocapture
cargo test --test control_flow_tests -- --nocapture
```
