// Conjetura de Collatz: pasos hasta llegar a 1.
// salida: 0
// salida: 8
// salida: 111
function pasos(n: integer): integer {
  let cuenta: integer = 0;
  while (n != 1) {
    if (n % 2 == 0) {
      n = n / 2;
    } else {
      n = 3 * n + 1;
    }
    cuenta = cuenta + 1;
  }
  return cuenta;
}

print(pasos(1));
print(pasos(6));
print(pasos(27));
