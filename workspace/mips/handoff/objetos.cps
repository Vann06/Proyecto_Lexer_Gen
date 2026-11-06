// Fases 1 y 2: campos, this, despacho virtual, listas anidadas.
class Figura {
  let nombre: string;
  let lados: integer = 3;
  let activa: boolean = true;
  let visible: boolean = false;
  function constructor(n: string) { this.nombre = n; }
  function area(): integer { return 0; }
  function describir(): string { return this.nombre + " base"; }
  function total(): integer { return this.area() + this.lados; }
}
class Cuadrado : Figura {
  let lado: integer = 4;
  function area(): integer { return this.lado * this.lado; }
  function describir(): string { return this.nombre + " cuadrado"; }
}
let f = new Cuadrado("C1");
print(f.area());
print(f.describir());
print(f.total());
print(f.activa);
print(f.visible);
let b: Figura = new Figura("B1");
print(b.total());
print(b.describir());
let matriz: integer[][] = [[1, 2], [3, 4, 5]];
print(matriz[1][2]);
let k: integer = 1;
print(matriz[k][k - 1] + matriz[0][k]);
foreach (fila in matriz) {
  foreach (v in fila) {
    print(v);
  }
}
