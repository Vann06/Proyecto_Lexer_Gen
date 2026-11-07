// Torres de Hanoi con 3 discos: imprime cada movimiento y el total.
// salida: A a C
// salida: A a B
// salida: C a B
// salida: A a C
// salida: B a A
// salida: B a C
// salida: A a C
// salida: 7
function mover(discos: integer, origen: string, destino: string, auxiliar: string): integer {
  if (discos == 0) {
    return 0;
  }
  let antes: integer = mover(discos - 1, origen, auxiliar, destino);
  print(origen + " a " + destino);
  let despues: integer = mover(discos - 1, auxiliar, destino, origen);
  return antes + 1 + despues;
}

print(mover(3, "A", "C", "B"));
