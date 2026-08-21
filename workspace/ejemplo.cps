function suma(a: integer, b: integer): integer {
  let resultado: integer = a + b;
  return resultado;
}

class Contador {
  var valor: integer = 0;

  function incrementar(): integer {
    valor = valor + 1;
    return valor;
  }
}

let x: integer = suma(2, 3);
let y: boolean = x > 4;

if (y) {
  print("x es mayor a 4");
} else {
  print("x no es mayor a 4");
}

while (x > 0) {
  x = x - 1;
}

print(faltante);
