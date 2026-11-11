// Lista enlazada: invertir, contar, buscar el maximo y detectar si contiene un valor.
// salida: 4
// salida: 3
// salida: 2
// salida: 1
// salida: 4
// salida: 4
// salida: true
// salida: false
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

function invertir(lista: Nodo): Nodo {
  let anterior: Nodo = null;
  let actual: Nodo = lista;
  while (actual != null) {
    let siguiente: Nodo = actual.siguiente;
    actual.siguiente = anterior;
    anterior = actual;
    actual = siguiente;
  }
  return anterior;
}

function contar(lista: Nodo): integer {
  if (lista == null) {
    return 0;
  }
  return 1 + contar(lista.siguiente);
}

function maximo(lista: Nodo): integer {
  let mayor: integer = lista.valor;
  let p: Nodo = lista.siguiente;
  while (p != null) {
    if (p.valor > mayor) {
      mayor = p.valor;
    }
    p = p.siguiente;
  }
  return mayor;
}

function contiene(lista: Nodo, buscado: integer): boolean {
  let p: Nodo = lista;
  while (p != null) {
    if (p.valor == buscado) {
      return true;
    }
    p = p.siguiente;
  }
  return false;
}

let lista: Nodo = invertir(crear([1, 2, 3, 4], 4));
imprimir(lista);
print(contar(lista));
print(maximo(lista));
print(contiene(lista, 3));
print(contiene(lista, 9));
