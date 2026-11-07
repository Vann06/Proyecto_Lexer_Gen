// Busqueda binaria iterativa y recursiva sobre una lista ordenada.
// salida: 3
// salida: 0
// salida: 7
// salida: -1
// salida: 4
// salida: -1
function iterativa(datos: integer[], n: integer, buscado: integer): integer {
  let inicio: integer = 0;
  let fin: integer = n - 1;
  while (inicio <= fin) {
    let medio: integer = (inicio + fin) / 2;
    if (datos[medio] == buscado) {
      return medio;
    }
    if (datos[medio] < buscado) {
      inicio = medio + 1;
    } else {
      fin = medio - 1;
    }
  }
  return -1;
}

function recursiva(datos: integer[], inicio: integer, fin: integer, buscado: integer): integer {
  if (inicio > fin) {
    return -1;
  }
  let medio: integer = (inicio + fin) / 2;
  if (datos[medio] == buscado) {
    return medio;
  }
  if (datos[medio] < buscado) {
    return recursiva(datos, medio + 1, fin, buscado);
  }
  return recursiva(datos, inicio, medio - 1, buscado);
}

let datos: integer[] = [1, 3, 5, 7, 9, 11, 13, 15];
print(iterativa(datos, 8, 7));
print(iterativa(datos, 8, 1));
print(iterativa(datos, 8, 15));
print(iterativa(datos, 8, 8));
print(recursiva(datos, 0, 7, 9));
print(recursiva(datos, 0, 7, 2));
