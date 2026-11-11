// Fase 3: try entre funciones, return dentro de try, valores tras el catch.
function explota(l: integer[], i: integer): integer {
  return l[i];
}
function protegido(l: integer[], i: integer): integer {
  try {
    return explota(l, i) * 2;
  } catch (e) {
    print(e);
  }
  return -1;
}
function divide(a: integer, b: integer): integer {
  return a / b;
}
function externo(l: integer[]): integer {
  let guardado: integer = 7;
  try {
    print(protegido(l, 9));
    print(divide(1, 0));
    print(123);
  } catch (e) {
    print("externo: " + e);
  }
  return guardado + 1;
}
let l = [1, 2, 3];
print(protegido(l, 1));
print((l[0] * 5) + externo(l));
let i: integer = 0;
while (i < 3) {
  try {
    print(10 % (i - 1));
  } catch (e) {
    print(e);
  }
  i = i + 1;
}
try {
  try {
    print(l[5]);
  } catch (e1) {
    print("adentro");
    print(l[6]);
  }
} catch (e2) {
  print("afuera: " + e2);
}
