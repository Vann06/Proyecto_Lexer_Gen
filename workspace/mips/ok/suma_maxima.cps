// Subarreglo de suma maxima con el algoritmo de Kadane.
// salida: 6
// salida: -1
// salida: 15
function kadane(datos: integer[], n: integer): integer {
  let mejor: integer = datos[0];
  let actual: integer = datos[0];
  for (let i: integer = 1; i < n; i = i + 1) {
    if (actual < 0) {
      actual = datos[i];
    } else {
      actual = actual + datos[i];
    }
    if (actual > mejor) {
      mejor = actual;
    }
  }
  return mejor;
}

print(kadane([-2, 1, -3, 4, -1, 2, 1, -5, 4], 9));
print(kadane([-4, -1, -7], 3));
print(kadane([1, 2, 3, 4, 5], 5));
