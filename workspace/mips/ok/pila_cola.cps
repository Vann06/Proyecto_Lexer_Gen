// Pila y cola como clases, sobre una lista enlazada.
// salida: 3
// salida: 2
// salida: 1
// salida: true
// salida: 1
// salida: 2
// salida: 3
// salida: true
class Nodo {
  var valor: integer;
  var siguiente: Nodo;
  function constructor(v: integer) {
    this.valor = v;
    this.siguiente = null;
  }
}

class Pila {
  var tope: Nodo;
  function constructor() {
    this.tope = null;
  }
  function apilar(v: integer) {
    let nuevo: Nodo = new Nodo(v);
    nuevo.siguiente = this.tope;
    this.tope = nuevo;
  }
  function desapilar(): integer {
    let v: integer = this.tope.valor;
    this.tope = this.tope.siguiente;
    return v;
  }
  function vacia(): boolean {
    return this.tope == null;
  }
}

class Cola {
  var frente: Nodo;
  var fondo: Nodo;
  function constructor() {
    this.frente = null;
    this.fondo = null;
  }
  function encolar(v: integer) {
    let nuevo: Nodo = new Nodo(v);
    if (this.fondo == null) {
      this.frente = nuevo;
    } else {
      this.fondo.siguiente = nuevo;
    }
    this.fondo = nuevo;
  }
  function desencolar(): integer {
    let v: integer = this.frente.valor;
    this.frente = this.frente.siguiente;
    if (this.frente == null) {
      this.fondo = null;
    }
    return v;
  }
  function vacia(): boolean {
    return this.frente == null;
  }
}

let pila = new Pila();
pila.apilar(1);
pila.apilar(2);
pila.apilar(3);
print(pila.desapilar());
print(pila.desapilar());
print(pila.desapilar());
print(pila.vacia());

let cola = new Cola();
cola.encolar(1);
cola.encolar(2);
cola.encolar(3);
print(cola.desencolar());
print(cola.desencolar());
print(cola.desencolar());
print(cola.vacia());
