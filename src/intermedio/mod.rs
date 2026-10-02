// Fase 16 (libro del dragón): generación de código intermedio — código de
// tres direcciones (TAC) / cuádruplos, a partir de la salida de `semantico`.
//
// - `tac`: el lenguaje intermedio (instrucciones, operandos, sintaxis).
// - `temps`: asignación y reciclaje de temporales.
// - `spec`: lo que el generador necesita saber de la gramática y el análisis
//   no le da (`%flow`, `%print`).
// - `gen`: el generador (traducción dirigida por la sintaxis).
//
// Plan de trabajo y reparto en `PLAN_TAC.md`; contrato de entrada en
// ARQUITECTURA.md §8.
//
// Igual que `semantico`, depende de la gramática que se reciba en cada
// práctica — no hay un lenguaje intermedio fijo asumido de antemano más
// allá de la forma genérica de TAC/cuádruplos.

pub mod gen;
pub mod spec;
pub mod tac;
pub mod temps;
