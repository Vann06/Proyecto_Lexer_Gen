// error: S006
// error: S002
// error: S016
// error: S013
// error: S014
// error: S010
// error: S033
class Punto {
  var x: integer;
  function constructor(valor: integer) { this.x = valor; }
}

function identidad(valor: integer): integer {
  return valor;
}

function entero(): integer {
  return "texto";
}

let numero: integer = "texto";
print(noExiste);
print(identidad());
print(identidad("texto"));
let punto = new Punto(1);
print(punto.noExiste);
print(entero());
let numeros = [1, 2, 3];
print(numeros["uno"]);
