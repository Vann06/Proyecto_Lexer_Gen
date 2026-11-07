// Numeros primos menores que 30 por division de prueba.
// salida: 2
// salida: 3
// salida: 5
// salida: 7
// salida: 11
// salida: 13
// salida: 17
// salida: 19
// salida: 23
// salida: 29
// salida: 10
function esPrimo(n: integer): boolean {
  if (n < 2) {
    return false;
  }
  let d: integer = 2;
  while (d * d <= n) {
    if (n % d == 0) {
      return false;
    }
    d = d + 1;
  }
  return true;
}

let cantidad: integer = 0;
for (let n: integer = 0; n < 30; n = n + 1) {
  if (esPrimo(n)) {
    print(n);
    cantidad = cantidad + 1;
  }
}
print(cantidad);
