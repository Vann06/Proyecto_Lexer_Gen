// FizzBuzz del 1 al 15.
// salida: 1
// salida: 2
// salida: Fizz
// salida: 4
// salida: Buzz
// salida: Fizz
// salida: 7
// salida: 8
// salida: Fizz
// salida: Buzz
// salida: 11
// salida: Fizz
// salida: 13
// salida: 14
// salida: FizzBuzz
for (let i: integer = 1; i <= 15; i = i + 1) {
  if (i % 15 == 0) {
    print("FizzBuzz");
  } else if (i % 3 == 0) {
    print("Fizz");
  } else if (i % 5 == 0) {
    print("Buzz");
  } else {
    print(i);
  }
}
