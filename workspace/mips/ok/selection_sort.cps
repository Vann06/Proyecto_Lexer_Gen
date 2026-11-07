// Selection sort. Las listas de Compiscript no se pueden modificar, asi que ordena una lista enlazada.
// salida: 1
// salida: 2
// salida: 3
// salida: 4
// salida: 4
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

function ordenar(lista: Nodo) {
  let p: Nodo = lista;
  while (p != null) {
    let menor: Nodo = p;
    let q: Nodo = p.siguiente;
    while (q != null) {
      if (q.valor < menor.valor) {
        menor = q;
      }
      q = q.siguiente;
    }
    let temporal: integer = p.valor;
    p.valor = menor.valor;
    menor.valor = temporal;
    p = p.siguiente;
  }
}

let lista: Nodo = crear([4, 2, 4, 1, 3], 5);
ordenar(lista);
imprimir(lista);
