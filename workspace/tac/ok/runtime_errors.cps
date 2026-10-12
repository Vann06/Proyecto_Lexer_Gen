// cubre: index_error, division_zero, try_catch
// salida: 100
// salida: 200
let numeros = [1, 2, 3];

try {
  print(10 / 0);
} catch (division) {
  print(100);
}

try {
  print(numeros[10]);
} catch (indice) {
  print(200);
}
