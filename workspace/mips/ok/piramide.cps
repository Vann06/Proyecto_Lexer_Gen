// Piramide y triangulo con ciclos anidados. Usa x porque los textos no aceptan asteriscos.
// salida:    x
// salida:   xxx
// salida:  xxxxx
// salida: xxxxxxx
// salida: x
// salida: xx
// salida: xxx
function repetir(texto: string, veces: integer): string {
  let resultado: string = "";
  for (let i: integer = 0; i < veces; i = i + 1) {
    resultado = resultado + texto;
  }
  return resultado;
}

function piramide(altura: integer) {
  for (let fila: integer = 1; fila <= altura; fila = fila + 1) {
    print(repetir(" ", altura - fila) + repetir("x", 2 * fila - 1));
  }
}

function triangulo(altura: integer) {
  for (let fila: integer = 1; fila <= altura; fila = fila + 1) {
    print(repetir("x", fila));
  }
}

piramide(4);
triangulo(3);
