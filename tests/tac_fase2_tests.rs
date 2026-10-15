//! Fase 2 del generador de código intermedio (ver `PLAN_TAC.md`): funciones,
//! clases y listas, sobre el pipeline real de Compiscript.
//!
//! Igual que `tac_fase1_tests.rs`, cada caso fija el TAC EXACTO del programa
//! completo (datos, `main` y funciones); el comentario de cada prueba dice qué
//! mirar en la traducción.
//!
//! Lo que el lenguaje no permite probar: no hay tipo `float` ni literales
//! flotantes (no puede aparecer un `(float)` en un argumento), y la
//! asignación a un subtipo (`let a: Animal = new Perro()`) la rechaza el
//! análisis (S006), así que la sobrescritura se prueba por `this` dentro de
//! la clase padre.
mod common;
use common::*;
use lexer_generator::intermedio::gen::extend_layout;

/// Los argumentos se evalúan y se pasan con `param` en orden; el resultado de la llamada entra por `call` (que la asignación directa lleva a `r`).  Una función que no devuelve valor no necesita `return`: `endfunc` lo implica.
#[test]
fn una_funcion_con_parametros_y_retorno_se_llama_desde_main() {
    let tac = programa(
        r#"function suma(a: integer, b: integer): integer {
  return a + b;
}
let r: integer = suma(2, 3);
print(suma(r, 1));
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
func main nivel 0 marco 8
    param 2
    param 3
    r = call suma, 2
    param r
    param 1
    t0 = call suma, 2
    print t0
endfunc

func suma nivel 1 marco 16
    t0 = a + b
    return t0
endfunc"#
    );
}

/// `factorial` es global (nivel 1): no lleva `link`. El temporal del argumento se recicla para el resultado.
#[test]
fn la_recursion_llama_a_la_funcion_sin_enlace() {
    let tac = programa(
        r#"function factorial(n: integer): integer {
  if (n <= 1) { return 1; }
  return n * factorial(n - 1);
}
print(factorial(5));
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 0 bytes
func main nivel 0 marco 8
    param 5
    t0 = call factorial, 1
    print t0
endfunc

func factorial nivel 1 marco 16
    if n > 1 goto L0
    return 1
L0:
    t0 = n - 1
    param t0
    t0 = call factorial, 1
    t0 = n * t0
    return t0
endfunc"#
    );
}

/// El ejemplo de PLAN_TAC §4: `sumar` recibe `link fp` (su dueña es la función en curso) y llega a `total` y `paso` por `fp[12]`.
#[test]
fn la_funcion_anidada_lee_el_marco_de_la_que_la_encierra() {
    let tac = programa(
        r#"let x: integer = 10;
function contador(paso: integer): integer {
  let total: integer = 0;
  function sumar(): integer {
    total = total + paso;
    return total;
  }
  let x: integer = sumar();
  return x + 1;
}
print(contador(5));
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
func main nivel 0 marco 8
    x = 10
    param 5
    t0 = call contador, 1
    print t0
endfunc

func contador nivel 1 marco 24
    total = 0
    link fp
    x.1 = call contador.sumar, 0
    t0 = x.1 + 1
    return t0
endfunc

func contador.sumar nivel 2 marco 24
    t0 = fp[12]
    t1 = fp[12]
    t0 = t0[-12] + t1[16]   ; total, paso
    t1 = fp[12]
    t1[-12] = t0            ; total
    t0 = fp[12]
    return t0[-12]          ; total
endfunc"#
    );
}

/// `workspace/ejemplo_closures.cps`: la función anidada se llama dos veces con `link fp`. La clase `Punto` aporta su `$init` y su vtable vacía.
#[test]
fn el_ejemplo_de_closures_del_workspace() {
    let tac = programa(
        r#"function contador(): integer {
  let total: integer = 0;

  function incrementar(paso: integer): integer {
    total = total + paso;
    return total;
  }

  incrementar(1);
  incrementar(2);
  return total;
}

class Punto {
  var x: integer = 0;
  var y: integer = 0;
}

let resultado: integer = contador();
print(resultado);
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
data vt_Punto:

func main nivel 0 marco 0
    resultado = call contador, 0
    print resultado
endfunc

func contador nivel 1 marco 20
    total = 0
    param 1
    link fp
    t0 = call contador.incrementar, 1
    param 2
    link fp
    t0 = call contador.incrementar, 1
    return total
endfunc

func contador.incrementar nivel 2 marco 24
    t0 = fp[12]
    t0 = t0[-12] + paso     ; total
    t1 = fp[12]
    t1[-12] = t0            ; total
    t0 = fp[12]
    return t0[-12]          ; total
endfunc

func Punto.$init nivel 1 marco 16
    t0 = this
    t0[4] = 0               ; x
    t0 = this
    t0[8] = 0               ; y
endfunc"#
    );
}

/// Desde `externa.cuenta`, la función declarada en `externa` está a un salto: `t = fp[12]` y `link t` (la regla `nc - ng + 1`).
#[test]
fn la_recursion_de_una_funcion_anidada_sigue_un_enlace() {
    let tac = programa(
        r#"function externa(): integer {
  function cuenta(n: integer): integer {
    if (n <= 0) { return 0; }
    return n + cuenta(n - 1);
  }
  return cuenta(3);
}
print(externa());
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 0 bytes
func main nivel 0 marco 8
    t0 = call externa, 0
    print t0
endfunc

func externa nivel 1 marco 16
    param 3
    link fp
    t0 = call externa.cuenta, 1
    return t0
endfunc

func externa.cuenta nivel 2 marco 16
    if n > 0 goto L0
    return 0
L0:
    t0 = n - 1
    param t0
    t0 = fp[12]
    link t0
    t0 = call externa.cuenta, 1
    t0 = n + t0
    return t0
endfunc"#
    );
}

/// `dos(uno(), 2)`: primero se calcula `uno()`, luego van juntos los `param` de `dos`. Una llamada sin valor (`nada()`) usada como sentencia sí se traduce.
#[test]
fn los_argumentos_de_una_llamada_anidada_no_se_entrelazan() {
    let tac = programa(
        r#"function uno(): integer { return 1; }
function dos(a: integer, b: integer): integer { return a + b; }
function nada() { print("nada"); }
print(dos(uno(), 2));
nada();
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 0 bytes
func main nivel 0 marco 8
    t0 = call uno, 0
    param t0
    param 2
    t0 = call dos, 2
    print t0
    call nada, 0
endfunc

func uno nivel 1 marco 8
    return 1
endfunc

func dos nivel 1 marco 16
    t0 = a + b
    return t0
endfunc

func nada nivel 1 marco 8
    print "nada"
endfunc"#
    );
}

/// `new` reserva, guarda la vtable, llama a `$init` y luego al constructor (`this` primero; `nargs` cuenta a `this`). `obj.campo` es `t[offset]`.
#[test]
fn new_con_constructor_campos_y_this() {
    let tac = programa(
        r#"class Contador {
  var cuenta: integer = 0;
  var nombre: string;
  function constructor(n: string, inicio: integer) {
    this.nombre = n;
    this.cuenta = inicio;
  }
  function incrementar() {
    cuenta = cuenta + 1;
  }
  function valor(): integer {
    return this.cuenta;
  }
}
let c = new Contador("a", 5);
c.incrementar();
print(c.valor());
print(c.nombre);
c.cuenta = c.cuenta + 10;
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
data vt_Contador: Contador.constructor, Contador.incrementar, Contador.valor

func main nivel 0 marco 16
    t0 = alloc 12
    t0[0] = &vt_Contador
    param t0
    call Contador.$init, 1
    param t0
    param "a"
    param 5
    call Contador.constructor, 3
    c = t0
    param c
    t0 = c[0]
    t0 = t0[4]
    callv t0, 1
    param c
    t0 = c[0]
    t0 = t0[8]
    t0 = callv t0, 1
    print t0
    t0 = c
    print t0[8]             ; nombre
    t0 = c
    t0 = t0[4] + 10         ; cuenta
    t1 = c
    t1[4] = t0              ; cuenta
endfunc

func Contador.$init nivel 1 marco 16
    t0 = this
    t0[4] = 0               ; cuenta
    t0 = this
    t0[8] = null            ; nombre
endfunc

func Contador.constructor nivel 1 marco 16
    t0 = this
    t0[8] = n               ; nombre
    t0 = this
    t0[4] = inicio          ; cuenta
endfunc

func Contador.incrementar nivel 1 marco 24
    t0 = this
    t0 = t0[4] + 1          ; cuenta
    t1 = this
    t1[4] = t0              ; cuenta
endfunc

func Contador.valor nivel 1 marco 16
    t0 = this
    return t0[4]            ; cuenta
endfunc"#
    );
}

/// Sin constructor no hay segunda llamada; una clase sin campos tampoco tiene `$init`.
#[test]
fn una_clase_sin_constructor_solo_llama_a_su_init() {
    let tac = programa(
        r#"class Punto { var x: integer = 1; var y: integer = 2; }
class Caja { }
let p = new Punto();
let q = new Caja();
print(p.x + p.y);
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 8 bytes
data vt_Punto:
data vt_Caja:

func main nivel 0 marco 16
    t0 = alloc 12
    t0[0] = &vt_Punto
    param t0
    call Punto.$init, 1
    p = t0
    t0 = alloc 4
    t0[0] = &vt_Caja
    q = t0
    t0 = p
    t1 = p
    t0 = t0[4] + t1[8]      ; x, y
    print t0
endfunc

func Punto.$init nivel 1 marco 16
    t0 = this
    t0[4] = 1               ; x
    t0 = this
    t0[8] = 2               ; y
endfunc"#
    );
}

/// `this.hablar()` dentro de `Animal.presentar` (tipo estático `Animal`) y `p.hablar()` usan la ranura 1; `vt_Perro` la apunta a `Perro.hablar`. El constructor es heredado y `Perro.$init` encadena a `Animal.$init`.
#[test]
fn la_sobrescritura_se_llama_por_la_misma_ranura_de_la_vtable() {
    let tac = programa(
        r#"class Animal {
  var nombre: string;
  function constructor(n: string) { this.nombre = n; }
  function hablar(): string { return "..."; }
  function presentar(): string { return this.nombre + this.hablar(); }
}
class Perro : Animal {
  var raza: string = "criollo";
  function hablar(): string { return "guau"; }
}
let p = new Perro("Toby");
print(p.presentar());
print(p.hablar());
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
data vt_Animal: Animal.constructor, Animal.hablar, Animal.presentar
data vt_Perro: Animal.constructor, Perro.hablar, Animal.presentar

func main nivel 0 marco 8
    t0 = alloc 12
    t0[0] = &vt_Perro
    param t0
    call Perro.$init, 1
    param t0
    param "Toby"
    call Animal.constructor, 2
    p = t0
    param p
    t0 = p[0]
    t0 = t0[8]
    t0 = callv t0, 1
    print t0
    param p
    t0 = p[0]
    t0 = t0[4]
    t0 = callv t0, 1
    print t0
endfunc

func Animal.$init nivel 1 marco 16
    t0 = this
    t0[4] = null            ; nombre
endfunc

func Animal.constructor nivel 1 marco 16
    t0 = this
    t0[4] = n               ; nombre
endfunc

func Animal.hablar nivel 1 marco 8
    return "..."
endfunc

func Animal.presentar nivel 1 marco 24
    t0 = this
    param this
    t1 = this[0]
    t1 = t1[4]
    t1 = callv t1, 1
    t0 = t0[4] concat t1    ; nombre
    return t0
endfunc

func Perro.$init nivel 1 marco 16
    param this
    call Animal.$init, 1
    t0 = this
    t0[8] = "criollo"       ; raza
endfunc

func Perro.hablar nivel 1 marco 8
    return "guau"
endfunc"#
    );
}

/// `this` y `saldo` se alcanzan desde `total` yendo por `fp[12]` hasta el marco del método.
#[test]
fn un_campo_usado_desde_una_funcion_anidada_en_un_metodo_sigue_los_enlaces() {
    let tac = programa(
        r#"class Cuenta {
  var saldo: integer = 100;
  function conBono(b: integer): integer {
    function total(): integer { return this.saldo + saldo + b; }
    return total();
  }
}
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 0 bytes
data vt_Cuenta: Cuenta.conBono

func main nivel 0 marco 0
endfunc

func Cuenta.$init nivel 1 marco 16
    t0 = this
    t0[4] = 100             ; saldo
endfunc

func Cuenta.conBono nivel 1 marco 16
    link fp
    t0 = call Cuenta.conBono.total, 0
    return t0
endfunc

func Cuenta.conBono.total nivel 2 marco 24
    t0 = fp[12]
    t0 = t0[16]             ; this
    t1 = fp[12]
    t1 = t1[16]
    t0 = t0[4] + t1[4]      ; saldo, saldo
    t1 = fp[12]
    t0 = t0 + t1[20]        ; b
    return t0
endfunc"#
    );
}

/// Literal (`alloc`, longitud, elementos), índice constante (offset ya sumado), índice variable y lista anidada.
#[test]
fn los_indices_de_listas_llevan_check_bounds() {
    let tac = programa(
        r#"let lista = [1, 2, 3];
let i: integer = 1;
print(lista[0]);
print(lista[i]);
let m: integer[][] = [[1, 2], [3, 4]];
print(m[1][0]);
let vacia: integer[] = [];
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 16 bytes
func main nivel 0 marco 16
    t0 = alloc 16
    t0[0] = 3
    t0[4] = 1
    t0[8] = 2
    t0[12] = 3
    lista = t0
    i = 1
    check_bounds lista, 0
    t0 = lista[4]
    print t0
    check_bounds lista, i
    t0 = i * 4
    t0 = t0 + 4
    t0 = lista[t0]
    print t0
    t0 = alloc 12
    t0[0] = 2
    t1 = alloc 12
    t1[0] = 2
    t1[4] = 1
    t1[8] = 2
    t0[4] = t1
    t1 = alloc 12
    t1[0] = 2
    t1[4] = 3
    t1[8] = 4
    t0[8] = t1
    m = t0
    check_bounds m, 1
    t0 = m[8]
    check_bounds t0, 0
    t0 = t0[4]
    print t0
    t0 = alloc 4
    t0[0] = 0
    vacia = t0
endfunc"#
    );
}

/// Sin vtable: un `store` por campo.
#[test]
fn un_literal_de_struct_guarda_cada_campo_en_su_offset() {
    let tac = programa(
        r#"struct Punto { x: integer; y: integer; }
let p = Punto { x: 1, y: 2 };
p.x = p.x + 1;
print(p.y);
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 4 bytes
func main nivel 0 marco 16
    t0 = alloc 8
    t0[0] = 1
    t0[4] = 2
    p = t0
    t0 = p
    t0 = t0[0] + 1          ; x
    t1 = p
    t1[0] = t0              ; x
    t0 = p
    print t0[4]             ; y
endfunc"#
    );
}

/// `return` dentro de `try` emite `try_end` antes de retornar; el `break` del `while` salta dentro de `tope`.
#[test]
fn return_cierra_los_try_y_break_no_sale_de_la_funcion() {
    let tac = programa(
        r#"function primero(l: integer[]): integer {
  try {
    return l[0];
  } catch (err) {
    return 0;
  }
  return 1;
}
function tope(): integer {
  let k: integer = 0;
  while (k < 5) {
    k = k + 1;
    if (k == 3) { break; }
  }
  return k;
}
print(primero([7]));
print(tope());
"#,
    );
    assert_eq!(
        tac,
        r#"; área estática: 0 bytes
func main nivel 0 marco 8
    t0 = alloc 8
    t0[0] = 1
    t0[4] = 7
    param t0
    t0 = call primero, 1
    print t0
    t0 = call tope, 0
    print t0
endfunc

func primero nivel 1 marco 20
    try_begin L0
    check_bounds l, 0
    t0 = l[4]
    try_end
    return t0
    try_end
    goto L1
L0:
    err = exception
    return 0
L1:
    return 1
endfunc

func tope nivel 1 marco 12
    k = 0
L2:
    if k >= 5 goto L3
    k = k + 1
    if k != 3 goto L4
    goto L3
L4:
    goto L2
L3:
    return k
endfunc"#
    );
}

/// `workspace/rubrica.cps` ya no deja construcciones sin traducir (C900) y
/// `extend_layout` le devuelve a la tabla las ranuras de temporales de cada
/// función con su etiqueta.
#[test]
fn la_rubrica_se_genera_completa_y_cada_funcion_recibe_sus_temporales() {
    let (program, mut analysis) = compilar(&read("workspace/rubrica.cps"));
    extend_layout(&mut analysis.layout, &program);
    let nombres: Vec<&str> = program.functions.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        nombres,
        ["saludar", "crearContador", "crearContador.siguiente", "Animal.$init", "Animal.constructor", "Animal.hablar", "Perro.hablar", "suma", "factorial"]
    );
    assert_eq!(analysis.layout.main_temp_slots, program.main.as_ref().map(|m| m.max_temps));
    for owner in analysis.layout.functions.values() {
        let f = program.functions.iter().find(|f| f.name == owner.name).expect("toda función declarada se genera");
        assert_eq!(owner.temp_slots, Some(f.max_temps), "{}", owner.name);
    }
    // Las vtables: `Perro` sobrescribe `hablar` en la misma ranura.
    let vt = |n: &str| program.data.iter().find(|d| d.name == n).map(|d| d.entries.clone());
    assert_eq!(vt("vt_Animal"), Some(vec!["Animal.constructor".into(), "Animal.hablar".into()]));
    assert_eq!(vt("vt_Perro"), Some(vec!["Animal.constructor".into(), "Perro.hablar".into()]));
}
