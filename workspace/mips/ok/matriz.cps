// Matrices: multiplicacion y transpuesta de 2 por 2.
// salida: 19
// salida: 22
// salida: 43
// salida: 50
// salida: 1
// salida: 3
// salida: 2
// salida: 4
function celda(a: integer[][], b: integer[][], i: integer, j: integer, n: integer): integer {
  let suma: integer = 0;
  for (let k: integer = 0; k < n; k = k + 1) {
    suma = suma + a[i][k] * b[k][j];
  }
  return suma;
}

let a: integer[][] = [[1, 2], [3, 4]];
let b: integer[][] = [[5, 6], [7, 8]];

for (let i: integer = 0; i < 2; i = i + 1) {
  for (let j: integer = 0; j < 2; j = j + 1) {
    print(celda(a, b, i, j, 2));
  }
}

for (let i: integer = 0; i < 2; i = i + 1) {
  for (let j: integer = 0; j < 2; j = j + 1) {
    print(a[j][i]);
  }
}
