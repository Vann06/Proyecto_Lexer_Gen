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

// Sistema de tipos: inicializador correcto, tipo inferido, aritmetica
// valida, asignacion compatible y una const bien usada.
let entera: integer = 1;
let inferida = 2;
entera = entera + inferida * 3 - 1;
const FIJA: integer = 10;
print(entera + FIJA);

// Retorno tipo-correcto y llamada recursiva con la aridad correcta:
// ninguno de los dos debe generar diagnostico.
function contarAtras(n: integer): integer {
  return contarAtras(n);
}

// Struct: tipo registro con campos tipados, literal con campos nombrados,
// acceso y asignacion de campo. Todo debe pasar limpio.
struct Punto { x: integer; y: integer; }

let origen: Punto = Punto { x: 0, y: 0 };
print(origen.x);
origen.y = 5;

// Struct anidado dentro de otro struct.
struct Caja { esquina: Punto; alto: integer; }
let caja: Caja = Caja { esquina: Punto { x: 1, y: 2 }, alto: 10 };
print(caja.esquina.y);
