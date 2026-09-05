let a: integer = 10;
let b: string = "hola";
let c: boolean = true;
let d = null;
let lit1: integer = 123;
let lit2: string = "texto";
let lit3: boolean = false;
let x = 5 + 3 * 2;
let y = !(x < 10 || x > 20);
let z = (1 + 2) * 3;
let nombre: string;
nombre = "Compiscript";
const PI: integer = 314;
function saludar(quien: string): string {
  return "Hola " + quien;
}
let mensaje = saludar("Mundo");
let lista = [1, 2, 3];
print(lista[0]);
let notas: integer[] = [90, 85, 100];
let matriz: integer[][] = [[1, 2], [3, 4]];
function crearContador(): integer {
  function siguiente(): integer {
    return 1;
  }
  return siguiente();
}
class Animal {
  let nombreA: string;
  function constructor(n: string) {
    this.nombreA = n;
  }
  function hablar(): string {
    return this.nombreA + " hace ruido.";
  }
}
class Perro : Animal {
  function hablar(): string {
    return this.nombreA + " ladra.";
  }
}
let perro: Perro = new Perro("Toby");
print(perro.nombreA);
print(perro.hablar());
{
  let interno = 42;
  print(interno);
}
if (x > 10) {
  print("Mayor a 10");
} else {
  print("Menor o igual");
}
while (x < 5) {
  x = x + 1;
}
do {
  x = x - 1;
} while (x > 0);
for (let i: integer = 0; i < 3; i = i + 1) {
  print(i);
}
foreach (item in lista) {
  print(item);
}
foreach (n in notas) {
  if (n == 100) { break; }
  if (n < 60) { continue; }
  print(n);
}
switch (x) {
  case 1:
    print("uno");
  case 2:
    print("dos");
  default:
    print("otro");
}
try {
  let peligro = lista[100];
  print(peligro);
} catch (err) {
  print("Error atrapado: " + err);
}
function suma(p: integer, q: integer): integer {
  return p + q;
}
function factorial(n2: integer): integer {
  if (n2 <= 1) { return 1; }
  return n2 * factorial(n2 - 1);
}
