// Fase 2: recursion, muchos parametros, enlace de acceso y pila balanceada.
function factorial(n: integer): integer {
  if (n <= 1) { return 1; }
  return n * factorial(n - 1);
}
function fib(n: integer): integer {
  if (n < 2) { return n; }
  return fib(n - 1) + fib(n - 2);
}
function siete(a: integer, b: integer, c: integer, d: integer, e: integer, f: integer, g: integer): integer {
  return a - b + c - d + e - f + g * 2;
}
let x: integer = 10;
function contador(paso: integer): integer {
  let total: integer = 0;
  function sumar(): integer {
    total = total + paso;
    return total;
  }
  let x: integer = sumar();
  x = sumar();
  return x + 1;
}
function afuera(a: integer): integer {
  function medio(b: integer): integer {
    function adentro(c: integer): integer {
      return a * 100 + b * 10 + c;
    }
    return adentro(b + 1);
  }
  return medio(a + 1);
}
function uno(): integer { return 1; }
print(factorial(12));
print(factorial(13));
print(fib(15));
print(siete(1, 2, 3, 4, 5, 6, 7));
print(contador(5));
print(afuera(1));
print(x);
let acumulado: integer = 0;
for (let i: integer = 0; i < 1000; i = i + 1) {
  acumulado = acumulado + uno();
}
print(acumulado);
