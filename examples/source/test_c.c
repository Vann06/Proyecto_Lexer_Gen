int suma(int a, int b) {
    int resultado = a + b;
    return resultado;
}

int maximo(int x, int y) {
    if (x > y) {
        return x;
    } else {
        return y;
    }
}

int factorial(int n) {
    int resultado = 1;
    int i = 1;
    while (i <= n) {
        resultado = resultado * i;
        i = i + 1;
    }
    return resultado;
}

int esPar(int n) {
    if (n % 2 == 0) {
        return 1;
    }
    return 0;
}

int signo(int n) {
    if (n > 0) {
        return 1;
    } else {
        if (n < 0) {
            return -1;
        } else {
            return 0;
        }
    }
}

int main(void) {
    int a = 5;
    int b = 3;
    int s = suma(a, b);
    int m = maximo(a, b);
    int f = factorial(5);
    int par = esPar(a);
    int sg = signo(b);
    int total = s + m + f;
    return 0;
}
