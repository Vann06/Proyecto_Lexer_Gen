// Maximo comun divisor, minimo comun multiplo, potencia y factorial.
// salida: 6
// salida: 12
// salida: 1024
// salida: 243
// salida: 120
// salida: 3628800
function mcd(a: integer, b: integer): integer {
  if (b == 0) {
    return a;
  }
  return mcd(b, a % b);
}

function mcm(a: integer, b: integer): integer {
  return a / mcd(a, b) * b;
}

function potencia(base: integer, exponente: integer): integer {
  let resultado: integer = 1;
  for (let i: integer = 0; i < exponente; i = i + 1) {
    resultado = resultado * base;
  }
  return resultado;
}

function potenciaRapida(base: integer, exponente: integer): integer {
  if (exponente == 0) {
    return 1;
  }
  let mitad: integer = potenciaRapida(base, exponente / 2);
  if (exponente % 2 == 0) {
    return mitad * mitad;
  }
  return mitad * mitad * base;
}

function factorial(n: integer): integer {
  if (n <= 1) {
    return 1;
  }
  return n * factorial(n - 1);
}

print(mcd(48, 18));
print(mcm(4, 6));
print(potencia(2, 10));
print(potenciaRapida(3, 5));
print(factorial(5));
print(factorial(10));
