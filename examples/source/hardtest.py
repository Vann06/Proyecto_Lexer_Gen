def calcularSuma(n1, n2):
    # Esta funcion recibe dos numeros y devuelve su suma
    """ 
        ESTO ES UN COMENTARIO MULTILINEA!
    """
    # Que hago aqui ?
    x = n1 + n2
    return x

def calcularResta(n1, n2):
    # Esta funcion recibe dos numeros y devuelve su resta
    
    x = n1 - n2
    return x

x = 5
y = 3
suma = calcularSuma(x, y)
resta = calcularResta(x, y)
print("La suma de ",x," y ",y," es ",suma)

if suma > 10:
    print("La suma es mayor que 10")
else:
    while (resta > 0):
        print("La resta es ",resta)
        resta = resta - 1
print("Fin del programa")
adios = "adios"
for i in adios:
    print(i)