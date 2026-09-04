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
let fantasma: ClaseInexistente;

// Invocacion: aridad y tipo, sobre metodo y sobre funcion libre.
function duplicar(n: integer): integer { return n + n; }
print(c.escalar(1, 2, 3));
print(c.escalar("texto"));
print(duplicar(1, 2));

// Sistema de tipos conectado al recorrido: inicializador, asignacion,
// aritmetica y const.
let malTipada: integer = "no soy entero";
let entera: integer = 1;
entera = "tampoco";
let texto: string = "x";
print(entera + texto);
const FIJA: integer = 10;
FIJA = 11;

// Retornos: tipo incompatible, retorno vacio en funcion tipada y valor
// retornado desde un procedimiento. El return fuera de toda funcion esta al
// FINAL del archivo: a nivel de programa corta el flujo, y todo lo que
// viniera despues seria codigo muerto W002.
function malRetorno(): integer { return "no soy entero"; }
function sinValor(): integer { return; }
function procedimiento() { return 5; }

// Aridad de una llamada RECURSIVA: la firma ya esta registrada al entrar.
function fact(n: integer): integer { return fact(1, 2, 3); }

// Struct: campo inexistente, tipo de campo, campo faltante y campo repetido
// en el literal. Un diagnostico por cada uno.
struct Punto { x: integer; y: integer; }
let malCampo: Punto = Punto { z: 1, x: 2, y: 3 };
let malTipo: Punto = Punto { x: "no soy entero", y: 2 };
let incompleto: Punto = Punto { x: 1 };
let repetido: Punto = Punto { x: 1, x: 2, y: 3 };

// return fuera de toda funcion: S019.
return 1;
