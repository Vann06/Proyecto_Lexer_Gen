// Fase 3: concat, streq, strne, print_bool, null.
function saluda(q: string): string { return "Hola, " + q + "!"; }
let a: string = "abc";
let b: string = "ab" + "c";
print(saluda("Mundo"));
print(a == b);
print(a != b);
if (a == b) { print("iguales"); } else { print("distintos"); }
if (a != "xyz") { print("no es xyz"); }
switch (b) {
  case "zzz":
    print("zzz");
  case "abc":
    print("encontrado");
    break;
  default:
    print("otro");
}
print(true);
print(1 < 2);
print(!(1 < 2));
let n = null;
print(n);
let vacio: string = "";
print(vacio + "x" + vacio);
