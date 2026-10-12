// cubre: function, call_return, recursion, closure
// salida: 5
// salida: 120
// salida: 3
function suma(a: integer, b: integer): integer {
  return a + b;
}

function factorial(n: integer): integer {
  if (n <= 1) { return 1; }
  return n * factorial(n - 1);
}

function contador(): integer {
  let total: integer = 0;
  function incrementar(paso: integer): integer {
    total = total + paso;
    return total;
  }
  incrementar(1);
  return incrementar(2);
}

print(suma(2, 3));
print(factorial(5));
print(contador());
