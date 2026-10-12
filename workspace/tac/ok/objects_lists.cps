// cubre: class, override_callv, list
// salida: Toby: guau
// salida: guau
// salida: 1
// salida: 3
class Animal {
  var nombre: string;
  function constructor(n: string) { this.nombre = n; }
  function hablar(): string { return "..."; }
  function presentar(): string { return this.nombre + ": " + this.hablar(); }
}

class Perro : Animal {
  function hablar(): string { return "guau"; }
}

let perro = new Perro("Toby");
print(perro.presentar());
print(perro.hablar());

let numeros = [1, 2, 3];
print(numeros[0]);
print(numeros[2]);
