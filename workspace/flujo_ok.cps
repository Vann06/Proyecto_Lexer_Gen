// Control de flujo completo, sin un solo diagnostico semantico.
// Lo usa tal cual el test control_flow_tests.
let activo: boolean = true;
let numeros: integer[] = [1, 2, 3];

while (activo) {
    if (activo) { continue; } else { break; }
}

do { print(1); } while (activo);

for (let i: integer = 0; i < 3; i = i + 1) { print(i); }

foreach (n in numeros) { print(n); }

switch (numeros[0]) {
    case 1: print(1); break;
    default: print(0);
}

try { print(1); } catch (e) { print(2); }

function revisar(valor: boolean): integer {
    while (valor) { break; }
    return 1;
}
