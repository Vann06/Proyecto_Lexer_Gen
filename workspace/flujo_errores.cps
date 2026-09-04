// Un error por regla de control de flujo. Codigos esperados:
//   S025 x4 condicion no booleana: if, while, do-while y for
//   S026 x2 break fuera de bucle: en funcion anidada y a nivel de programa
//   S027 x3 continue fuera de bucle: funcion anidada, programa y switch
//   S035 x1 case de tipo incompatible con el discriminante
//   S036 x1 foreach sobre algo que no es una coleccion
// Ninguna sentencia terminal lleva otra detras dentro del mismo bloque, asi
// que este archivo no debe producir ningun W002 de codigo muerto.
if (1) { print(1); }
while ("texto") { print(1); }
do { print(1); } while (5);
for (let i: integer = 0; i; i = i + 1) { print(i); }

foreach (x in 7) { print(x); }

switch (1) { case "dos": break; }
switch (2) { case 1: continue; }

// Una funcion anidada no puede saltar al bucle que la rodea: la frontera de
// funcion corta la busqueda del contexto.
while (true) {
    function saleMal(): integer { break; }
    function sigueMal(): integer { continue; }
    print(1);
}

// A nivel de programa no hay bucle al cual saltar. Cada uno en su propio
// bloque para que ninguno deje al otro como codigo muerto.
{ break; }
{ continue; }
