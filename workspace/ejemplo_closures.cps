function contador(): integer {
  let total: integer = 0;

  function incrementar(paso: integer): integer {
    total = total + paso;
    return total;
  }

  incrementar(1);
  incrementar(2);
  return total;
}

class Punto {
  var x: integer = 0;
  var y: integer = 0;
}

let resultado: integer = contador();
print(resultado);
