// Fase 17 (libro del dragón): generación de código objetivo/ensamblador a
// partir de la salida de `intermedio`.
//
// AÚN NO IMPLEMENTADO — ver ORGANIZACION.md § "Fases futuras" para el
// roadmap completo y las decisiones de diseño ya tomadas.
//
// Nombre elegido para no chocar con dos módulos ya existentes que también
// se llaman "codegen" pero son otra cosa:
//   - `lexico::codegen::rust_codegen` — emite el LEXER standalone en Rust
//     (Fase 13, ya implementada), no código objetivo del programa fuente.
//   - `api::codegen` — el handler HTTP que expone lo anterior (`/api/codegen`).
