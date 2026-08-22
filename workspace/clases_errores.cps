class Figura {
  var area: integer = 0;
  function constructor(areaInicial: integer) { this.area = areaInicial; }
  function escalar(factor: integer) { this.area = factor; }
}
class Circulo : Figura {
  var radio: integer = 0;
  function constructor(r: integer) { this.radio = r; }
  function romper() {
    this.noExisteEnLaClase = 1;
    this.radio = "no es entero";
  }
}

let c: Circulo = new Circulo(5);
print(c.noExiste);
let malo1 = new Circulo("texto");
let malo2 = new Circulo(1, 2);
let malo3 = new NoDeclarada();
print(this.area);
c.noExisteTampoco = 3;

// Anotacion de tipo que nombra una clase que nunca se declaro.
let fantasma: ClaseInexistente = 1;

// Invocacion: aridad y tipo, sobre metodo y sobre funcion libre.
function duplicar(n: integer): integer { return n + n; }
print(c.escalar(1, 2, 3));
print(c.escalar("texto"));
print(duplicar(1, 2));
