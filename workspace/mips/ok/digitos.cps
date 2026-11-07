// Ejercicios con digitos: suma, conteo, numero de Armstrong y binario.
// salida: 15
// salida: 4
// salida: true
// salida: false
// salida: 1010
// salida: 11111111
function sumaDigitos(n: integer): integer {
  let suma: integer = 0;
  while (n > 0) {
    suma = suma + n % 10;
    n = n / 10;
  }
  return suma;
}

function contarDigitos(n: integer): integer {
  if (n < 10) {
    return 1;
  }
  return 1 + contarDigitos(n / 10);
}

function cubo(n: integer): integer {
  return n * n * n;
}

function esArmstrong(n: integer): boolean {
  let suma: integer = 0;
  let resto: integer = n;
  while (resto > 0) {
    suma = suma + cubo(resto % 10);
    resto = resto / 10;
  }
  return suma == n;
}

function binario(n: integer): integer {
  if (n == 0) {
    return 0;
  }
  return binario(n / 2) * 10 + n % 2;
}

print(sumaDigitos(12345));
print(contarDigitos(1000));
print(esArmstrong(153));
print(esArmstrong(154));
print(binario(10));
print(binario(255));
