// Bubble sort. Las listas de Compiscript no se pueden modificar, asi que ordena una lista enlazada.
// salida: 1
// salida: 2
// salida: 3
// salida: 5
// salida: 7
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

function ordenar(lista: Nodo) {
  let cambio: boolean = true;
  while (cambio) {
    cambio = false;
    let p: Nodo = lista;
    while (p != null && p.siguiente != null) {
      let q: Nodo = p.siguiente;
      if (p.valor > q.valor) {
        let temporal: integer = p.valor;
        p.valor = q.valor;
        q.valor = temporal;
        cambio = true;
      }
      p = q;
    }
  }
}

let lista: Nodo = crear([5, 2, 9, 1, 7, 3], 6);
ordenar(lista);
imprimir(lista);
