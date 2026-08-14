// Fase 15 (libro del dragón): análisis semántico — tabla de símbolos, alcance
// y chequeo de tipos.
//
// AÚN NO IMPLEMENTADO — ver ORGANIZACION.md § "Fases futuras" para el
// roadmap completo y las decisiones de diseño ya tomadas.
//
// Restricción de diseño, no negociable: el generador es agnóstico a la
// gramática (cualquier .yal/.yalp/.txt que se reciba, no un lenguaje fijo
// hardcodeado aquí). Cualquier regla semántica tiene que salir de lo que
// declara la gramática dada, igual que hoy el lexer no sabe de antemano qué
// tokens va a tokenizar.
//
// Punto de entrada esperado cuando se implemente: el `ParseNode` que ya
// construyen `sintactico::runtime::parser_lr::LRParser::parse_tree` /
// `parse_recovering_with_pos` y `sintactico::runtime::ll1::LL1Parser::
// parse_tree` — ambos ya anotan cada hoja con `line`/`col` (ver
// `sintactico::runtime::parse_tree::ParseNode`), así que los mensajes de
// error semántico ("variable X no declarada") pueden ubicarse sin trabajo
// adicional. Ninguno de los dos se invoca hoy desde la API HTTP — antes de
// conectar esta fase al pipeline hay que resolver la duplicación de drivers
// shift-reduce documentada en ORGANIZACION.md (5 variantes hoy).
