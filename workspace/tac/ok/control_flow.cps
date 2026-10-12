// cubre: booleans, short_circuit, if_else, while, do_while, for, switch, break_continue
// salida: 1
// salida: 1
// salida: 3
// salida: 2
// salida: 0
// salida: 2
// salida: 20
let guard: boolean = false && (10 / 0 > 0) || true;
if (guard) { print(1); } else { print(0); }

let i: integer = 0;
while (i < 4) {
  i = i + 1;
  if (i == 2) { continue; }
  if (i == 4) { break; }
  print(i);
}

do { i = i - 1; } while (i > 2);
print(i);

for (let j: integer = 0; j < 4; j = j + 1) {
  if (j == 1) { continue; }
  if (j == 3) { break; }
  print(j);
}

switch (i) {
  case 1:
    print(10);
  case 2:
    print(20);
    break;
  default:
    print(99);
}
