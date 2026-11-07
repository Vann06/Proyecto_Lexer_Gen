// Merge sort. Las listas de Compiscript no se pueden modificar, asi que ordena una lista enlazada.
// salida: 1
// salida: 2
// salida: 3
// salida: 4
// salida: 5
// salida: 6
// salida: 7
// salida: 8
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

function partir(lista: Nodo): Nodo {
  let lento: Nodo = lista;
  let rapido: Nodo = lista.siguiente;
  while (rapido != null && rapido.siguiente != null) {
    lento = lento.siguiente;
    rapido = rapido.siguiente.siguiente;
  }
  let segunda: Nodo = lento.siguiente;
  lento.siguiente = null;
  return segunda;
}

function mezclar(a: Nodo, b: Nodo): Nodo {
  if (a == null) {
    return b;
  }
  if (b == null) {
    return a;
  }
  if (a.valor <= b.valor) {
    a.siguiente = mezclar(a.siguiente, b);
    return a;
  }
  b.siguiente = mezclar(a, b.siguiente);
  return b;
}

function ordenar(lista: Nodo): Nodo {
  if (lista == null || lista.siguiente == null) {
    return lista;
  }
  let segunda: Nodo = partir(lista);
  return mezclar(ordenar(lista), ordenar(segunda));
}

imprimir(ordenar(crear([8, 3, 5, 1, 7, 2, 6, 4], 8)));
