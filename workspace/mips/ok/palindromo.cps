// Palindromos de numeros y de palabras. Una palabra va como lista de letras: un texto no se puede indexar.
// salida: true
// salida: false
// salida: true
// salida: true
// salida: false
function invertir(n: integer): integer {
  let resultado: integer = 0;
  while (n > 0) {
    resultado = resultado * 10 + n % 10;
    n = n / 10;
  }
  return resultado;
}

function numeroPalindromo(n: integer): boolean {
  return n == invertir(n);
}

function palabraPalindromo(letras: string[], n: integer): boolean {
  let i: integer = 0;
  let j: integer = n - 1;
  while (i < j) {
    if (letras[i] != letras[j]) {
      return false;
    }
    i = i + 1;
    j = j - 1;
  }
  return true;
}

print(numeroPalindromo(12321));
print(numeroPalindromo(1231));
print(numeroPalindromo(7));
print(palabraPalindromo(["r", "e", "c", "o", "n", "o", "c", "e", "r"], 9));
print(palabraPalindromo(["h", "o", "l", "a"], 4));
