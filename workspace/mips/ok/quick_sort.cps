// Quick sort con el primer elemento como pivote. Las listas de Compiscript no se pueden modificar, asi que ordena una lista enlazada.
// salida: 1
// salida: 1
// salida: 2
// salida: 3
// salida: 4
// salida: 5
// salida: 6
// salida: 9
class Nodo {
  var valor: integer;
  var siguiente: Nodo;
  function constructor(v: integer) {
    this.valor = v;
    this.siguiente = null;
  }
}

function crear(datos: integer[], n: integer): Nodo {
  let cabeza: Nodo = null;
  let i: integer = n - 1;
  while (i >= 0) {
    let nuevo: Nodo = new Nodo(datos[i]);
    nuevo.siguiente = cabeza;
    cabeza = nuevo;
    i = i - 1;
  }
  return cabeza;
}

function imprimir(lista: Nodo) {
  let p: Nodo = lista;
  while (p != null) {
    print(p.valor);
    p = p.siguiente;
  }
}

function ordenar(lista: Nodo): Nodo {
  if (lista == null) {
    return null;
  }
  let pivote: Nodo = lista;
  let menores: Nodo = null;
  let mayores: Nodo = null;
  let p: Nodo = lista.siguiente;
  while (p != null) {
    let siguiente: Nodo = p.siguiente;
    if (p.valor < pivote.valor) {
      p.siguiente = menores;
      menores = p;
    } else {
      p.siguiente = mayores;
      mayores = p;
    }
    p = siguiente;
  }
  pivote.siguiente = ordenar(mayores);
  let inicio: Nodo = ordenar(menores);
  if (inicio == null) {
    return pivote;
  }
  let ultimo: Nodo = inicio;
  while (ultimo.siguiente != null) {
    ultimo = ultimo.siguiente;
  }
  ultimo.siguiente = pivote;
  return inicio;
}

imprimir(ordenar(crear([3, 1, 4, 1, 5, 9, 2, 6], 8)));
