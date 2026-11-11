// Two sum: indices de dos valores que suman el objetivo.
// salida: 0
// salida: 1
// salida: 1
// salida: 2
// salida: -1
function dosSuma(datos: integer[], n: integer, objetivo: integer): boolean {
  for (let i: integer = 0; i < n; i = i + 1) {
    for (let j: integer = i + 1; j < n; j = j + 1) {
      if (datos[i] + datos[j] == objetivo) {
        print(i);
        print(j);
        return true;
      }
    }
  }
  print(-1);
  return false;
}

let datos: integer[] = [2, 7, 11, 15];
dosSuma(datos, 4, 9);
dosSuma(datos, 4, 18);
dosSuma(datos, 4, 100);
