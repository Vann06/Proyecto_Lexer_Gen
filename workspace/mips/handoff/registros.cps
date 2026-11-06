// Fase 4: valores vivos durante llamadas, derrames y registros s entre funciones.
function doble(n: integer): integer { return n * 2; }
function triple(n: integer): integer { return n * 3; }
function suma(p: integer, q: integer): integer { return p + q; }
function muchos(a: integer, b: integer): integer {
  return (a * 1) + ((a * 2) + ((a * 3) + ((a * 4) + ((a * 5) + ((a * 6) + ((a * 7) + ((a * 8) + ((a * 9) + ((a * 10) + ((b * 11) + (b * 12)))))))))));
}
function conLlamadas(a: integer): integer {
  return (a * 1) + ((a * 2) + ((a * 3) + ((a * 4) + ((a * 5) + ((a * 6) + ((a * 7) + ((a * 8) + ((a * 9) + ((a * 10) + doble(a))))))))));
}
function recursivo(n: integer): integer {
  if (n == 0) { return 0; }
  return (n * 2) + (recursivo(n - 1) + (n * 3));
}
print(doble(3) + triple(4));
print(suma(doble(1), suma(triple(2), doble(3))));
print((doble(1) + doble(2)) * (triple(1) + triple(2)));
print(muchos(1, 2));
print(conLlamadas(2));
print(recursivo(10));
let i: integer = 0;
let t: integer = 0;
while (i < 10) {
  if (i % 2 == 0 && i > 2 || i == 1) {
    t = t + doble(i) * triple(i);
  } else {
    t = t - i;
  }
  i = i + 1;
}
print(t);
