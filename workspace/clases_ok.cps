class Figura {
  var area: integer = 0;

  function constructor(areaInicial: integer) {
    this.area = areaInicial;
  }

  function obtenerArea(): integer {
    return this.area;
  }

  function escalar(factor: integer) {
    this.area = this.area * factor;
  }
}

class Circulo : Figura {
  var radio: integer = 0;

  function constructor(r: integer) {
    this.radio = r;
    this.area = 3 * r * r;
  }

  function obtenerRadio(): integer {
    return radio;
  }
}

let f: Figura = new Figura(10);
print(f.obtenerArea());

let c: Circulo = new Circulo(5);
print(c.obtenerRadio());
print(c.obtenerArea());
print(c.area);

c.radio = 7;
c.area = 100;

// Invocaciones con aridad y tipos correctos: metodo propio, metodo
// heredado, y funcion libre. Ninguna debe generar diagnostico.
function duplicar(n: integer): integer {
  return n + n;
}

c.escalar(3);
f.escalar(2);
print(duplicar(21));
