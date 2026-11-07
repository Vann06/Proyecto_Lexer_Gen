// Insertion sort. Las listas de Compiscript no se pueden modificar, asi que ordena una lista enlazada.
// salida: -3
// salida: 0
// salida: 2
// salida: 8
// salida: 10
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

function insertar(ordenada: Nodo, nodo: Nodo): Nodo {
  if (ordenada == null || nodo.valor <= ordenada.valor) {
    nodo.siguiente = ordenada;
    return nodo;
  }
  let p: Nodo = ordenada;
  while (p.siguiente != null && p.siguiente.valor < nodo.valor) {
    p = p.siguiente;
  }
  nodo.siguiente = p.siguiente;
  p.siguiente = nodo;
  return ordenada;
}

function ordenar(lista: Nodo): Nodo {
  let ordenada: Nodo = null;
  let p: Nodo = lista;
  while (p != null) {
    let siguiente: Nodo = p.siguiente;
    ordenada = insertar(ordenada, p);
    p = siguiente;
  }
  return ordenada;
}

imprimir(ordenar(crear([8, -3, 10, 0, 2], 5)));
