# Módulo `operators`

`operators` valida las expresiones binarias y unarias que la tabla aritmética
de `types` no cubre. No reconoce ningún símbolo por sí mismo: la gramática
`.yalp` declara qué token es cada operador, y aquí solo llegan tipos ya
resueltos y posiciones.

## Qué valida

- **Lógicas** (`&& ||`): los dos operandos deben ser `bool`.
- **Unarias**: `!` exige `bool`; `-` exige un numérico y conserva su tipo
  (negar un `integer` da `integer`, no `float`).
- **Comparaciones**: `< <= > >=` exigen operandos numéricos; `== !=` solo
  exigen que los dos lados sean compatibles.
- **Sentido semántico**: una función, una clase o un tipo registro *nombrados
  a secas* no son valores. Es el caso "no multiplicar funciones".

Las comparaciones y las lógicas producen `bool` **siempre**, aunque un operando
no se haya podido tipar: son booleanas por construcción. `Unknown` es neutro y
nunca genera un diagnóstico por sí solo — pero tampoco tapa al operando de al
lado, que se sigue validando.

## Por qué la regla de "no es un valor" hace falta

`classes::resolve_expr_type` sobre el identificador de una función devuelve su
**tipo de retorno**. Sin esta regla, `f * 2` con `f(): integer` se ve idéntico
a multiplicar un entero y pasa sin diagnóstico.

La comprobación mira la FORMA del nodo: baja por la cadena de precedencia
(`term → unary → primary → atom → ID`, todos nodos de un solo hijo) hasta la
hoja. Un nodo con varios hijos corta el descenso, y eso es lo que distingue el
nombre pelado de una llamada:

```text
f * 2      ->  el operando baja hasta la hoja ID "f"     -> S031
f(1) * 2   ->  el operando es el nodo de llamada         -> válido
```

## Directivas `.yalp`

```yalp
%logic AND and
%logic OR  or

%compare EQ  eq      %compare NEQ neq
%compare LT  lt      %compare LTE lte
%compare GT  gt      %compare GTE gte

%unary NOT   not
%unary MINUS negate
```

Se declaran por TOKEN, no por producción: el nodo se reconoce por su forma
—tres hijos con el operador en el medio, o dos con el operador adelante—, así
que no hay que enumerar `or_expr`/`and_expr`/`equality_expr`/`relational_expr`.

Un token puede estar en dos familias sin colisionar: `MINUS` es a la vez
`%arith MINUS subtract` y `%unary MINUS negate`, y las dos formas se
distinguen por el número de hijos del nodo.

Un operador no reconocido en el lado derecho hace que esa línea se ignore
—mismo criterio que `%arith`—, así que ese token simplemente no se valida.

## Códigos

| código | significado |
|---|---|
| `S028` | operando no booleano en `&&`, `\|\|` o `!` |
| `S029` | operandos incompatibles en una comparación |
| `S030` | operando no numérico en `-` unario |
| `S031` | función/clase/registro usada como valor |

## Efecto sobre el control de flujo

Antes de este módulo, `classes::resolve_expr_type` devolvía `None` para toda
comparación, así que una directiva `%condition` sobre un `if`/`while` nunca
veía el tipo real de su condición y se rendía en silencio. Al tipar las
comparaciones como `bool`, esos chequeos pasan a validar de verdad.

## Archivos relacionados

- `mod.rs`: enums, reglas, detección por forma y la regla de operando-no-valor.
- `../classes/mod.rs`: `resolve_expr_type` delega aquí para tipar las tres formas.
- `../analyzer/mod.rs`: aplica las reglas mientras recorre el árbol.
- `../spec/mod.rs`: convierte las directivas en los tres mapas de tokens.
- `../types/mod.rs`: dueño de `is_numeric` y de toda coerción entre tipos.
- `../../../tests/operator_tests.rs`: pruebas de integración.

## Cómo probarlo

```powershell
cargo test semantico::operators -- --nocapture
cargo test --test operator_tests -- --nocapture
```
