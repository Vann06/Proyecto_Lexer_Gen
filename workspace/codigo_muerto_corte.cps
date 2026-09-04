// Corte de evaluacion: lo que sigue a la sentencia terminal no se analiza,
// asi que las dos lineas muertas NO producen los S002 que producirian si
// se recorrieran. El unico diagnostico es el W002 que las senala.
function corta(): integer {
    return 1;
    noExisteEsteNombre = 5;
    tampocoEste = niEste;
}
