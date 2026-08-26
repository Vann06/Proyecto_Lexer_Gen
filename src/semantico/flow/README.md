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
%condition for_stmt 2
%loop while_stmt
%loop for_stmt
%break break_stmt
%continue continue_stmt
```

En `%condition`, el último valor puede ser el nombre del hijo que contiene la
expresión o su índice dentro de la producción. Los nombres son propios de cada
gramática; el código Rust no depende de Compiscript.

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
