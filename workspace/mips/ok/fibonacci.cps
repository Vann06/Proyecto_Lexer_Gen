// Fibonacci iterativo y recursivo.
// salida: 0
// salida: 1
// salida: 1
// salida: 2
// salida: 3
// salida: 5
// salida: 8
// salida: 13
// salida: 21
// salida: 34
// salida: 55
function iterativo(n: integer): integer {
  let a: integer = 0;
  let b: integer = 1;
  for (let i: integer = 0; i < n; i = i + 1) {
    let siguiente: integer = a + b;
    a = b;
    b = siguiente;
  }
  return a;
}

function recursivo(n: integer): integer {
  if (n < 2) {
    return n;
  }
  return recursivo(n - 1) + recursivo(n - 2);
}

for (let i: integer = 0; i < 10; i = i + 1) {
  print(iterativo(i));
}
print(recursivo(10));
