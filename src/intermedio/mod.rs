// Fase 16 (libro del dragón): generación de código intermedio — código de
// tres direcciones (TAC) / cuádruplos, a partir de la salida de `semantico`.
//
// El generador todavía no está escrito. Lo que ya existe es su
// configuración por gramática: `spec` lee las directivas `%flow` (la forma
// de cada `if`/`while`/`for`/... — qué hijo es la condición, el cuerpo, el
// `else`), que es lo único que el generador necesita saber de la gramática y
// el análisis semántico no le da. Ver ARQUITECTURA.md §8 para el contrato de
// traspaso completo.
//
// Igual que `semantico`, depende de la gramática que se reciba en cada
// práctica — no hay un lenguaje intermedio fijo asumido de antemano más
// allá de la forma genérica de TAC/cuádruplos.

pub mod spec;
