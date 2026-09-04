// Una instruccion inalcanzable por cada clase de sentencia terminal:
// cuatro W002, uno por bloque.
function tras_return(): integer {
    return 1;
    print(2);
}

while (true) {
    break;
    print(3);
}

while (true) {
    continue;
    print(4);
}

let n: integer = 1;
switch (n) {
    case 1: break; print(5);
    default: print(6);
}
