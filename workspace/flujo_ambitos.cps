// La variable de un for, la de un foreach y la de un catch viven en el
// ambito que abre su propia construccion: los tres usos finales son S002.
let numeros: integer[] = [1, 2, 3];
for (let indice: integer = 0; indice < 3; indice = indice + 1) { print(indice); }
foreach (elemento in numeros) { print(elemento); }
try { print(1); } catch (problema) { print(2); }
print(indice);
print(elemento);
print(problema);
