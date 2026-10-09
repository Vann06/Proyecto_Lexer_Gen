# Código intermedio (TAC) de Compiscript

> Este archivo reúne la documentación del lenguaje intermedio. Por ahora trae
> solo la sección del integrante B (funciones, clases y listas); las del
> conjunto completo de instrucciones, expresiones y control de flujo (A) y la
> de arquitectura y uso (C) se agregan en la Fase 4. Las instrucciones están
> definidas en `src/intermedio/tac.rs` y el diseño general en `PLAN_TAC.md` §4.

## Funciones, clases y listas

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
