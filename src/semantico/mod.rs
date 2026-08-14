// Fase 15 (libro del dragón): análisis semántico — tabla de símbolos, alcance
// y chequeo de tipos.
//
// `scopes`/`symbols` ya están implementados: la tabla de símbolos con
// entornos anidados (global/función/clase/bloque), declare/lookup con
// shadowing correcto y volcado del estado — ver sus propios doc-comments.
// Lo que sigue AÚN NO IMPLEMENTADO: un walker que recorra un `ParseNode`
// real y llame a esa tabla (ver ORGANIZACION.md § "Fases futuras" para el
// roadmap completo), y el chequeo de tipos.
//
// Restricción de diseño, no negociable: el generador es agnóstico a la
// gramática (cualquier .yal/.yalp/.txt que se reciba, no un lenguaje fijo
// hardcodeado aquí). `scopes`/`symbols` ya respetan esto — no saben nada de
// ninguna gramática concreta, solo la mecánica y las reglas de entornos
// anidados. Cualquier regla semántica adicional tiene que salir de lo que
// declara la gramática dada, igual que hoy el lexer no sabe de antemano qué
// tokens va a tokenizar.
//
// Punto de entrada esperado para el futuro walker: el `ParseNode` que ya
// construyen `sintactico::runtime::parser_lr::LRParser::parse_tree` /
// `parse_recovering_with_pos` y `sintactico::runtime::ll1::LL1Parser::
// parse_tree` — ambos ya anotan cada hoja con `line`/`col` (ver
// `sintactico::runtime::parse_tree::ParseNode`), que es justo lo que
// `symbols::SymbolTable::declare`/`lookup_or_err` esperan para ubicar sus
// errores. Ninguno de los dos se invoca hoy desde la API HTTP — antes de
// conectar esta fase al pipeline hay que resolver la duplicación de drivers
// shift-reduce documentada en ORGANIZACION.md (5 variantes hoy).

pub mod scopes;
pub mod symbols;
