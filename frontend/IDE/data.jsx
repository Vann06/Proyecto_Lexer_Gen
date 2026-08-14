/* ============================================================================
   Mock data — LR(1) clásico:   S' → S    S → C C    C → TC C  |  TD
   (terminales TC/TD en vez de c/d minúscula: el pipeline real siempre pasa
   el kind del lexer por mayúsculas — ver api::lexico::lex_normalize_kind —
   así que un terminal declarado en minúscula nunca podría matchear contra
   lo que produce el lexer. Se mantiene 'c'/'d' como caracteres de fuente
   literales en TEST_RAW; lo que cambia es el nombre del token que el lexer
   les asigna.)
   ========================================================================== */

// Contenido crudo de cada archivo — usado por el API cuando se hace RUN / PARSEAR
const YAL_RAW = `let c  = 'c'
let d  = 'd'
let ws = [' ''\t''\n''\r']
rule tokens =
  | c    { return TC }
  | d    { return TD }
  | ws   { skip }`;

const YALP_RAW = `%token TC TD
%%
S : C C ;
C : TC C
  | TD
;`;

const TEST_RAW = `c c d c d
c d d
d c d`;

const FILES = {
  yal:  { name: "lexer.yal",   kind:"yal",  current: 6, rawContent: YAL_RAW,  dirty: false },
  yalp: { name: "parser.yalp", kind:"yalp", current: 5, rawContent: YALP_RAW, dirty: false },
  test: { name: "input.txt",   kind:"txt",  current: 2, rawContent: TEST_RAW, dirty: false },
};

/* ---- LR(1) states for S' → S    S → CC    C → TC C | TD ---- */
const STATES = [
  { id:0, items:[ "S' → · S , $", "S → · C C , $", "C → · TC C , TC/TD", "C → · TD , TC/TD" ] },
  { id:1, items:[ "S' → S · , $" ] },
  { id:2, items:[ "S → C · C , $", "C → · TC C , $", "C → · TD , $" ] },
  { id:3, items:[ "C → TC · C , TC/TD", "C → · TC C , TC/TD", "C → · TD , TC/TD" ] },
  { id:4, items:[ "C → TD · , TC/TD" ] },
  { id:5, items:[ "S → C C · , $" ] },
  { id:6, items:[ "C → TC · C , $", "C → · TC C , $", "C → · TD , $" ] },
  { id:7, items:[ "C → TD · , $" ] },
  { id:8, items:[ "C → TC C · , TC/TD" ] },
  { id:9, items:[ "C → TC C · , $" ] },
];

/* ACTION[state][terminal]   GOTO[state][nonterminal] */
/* terminals: TC TD $   nonterminals: S C */
const ACTION = {
  0:{TC:"s3", TD:"s4"},
  1:{$:"acc"},
  2:{TC:"s6", TD:"s7"},
  3:{TC:"s3", TD:"s4"},
  4:{TC:"r3", TD:"r3"},
  5:{$:"r1"},
  6:{TC:"s6", TD:"s7"},
  7:{$:"r3"},
  8:{TC:"r2", TD:"r2"},
  9:{$:"r2"},
};
const GOTO = {
  0:{S:1, C:2},
  2:{C:5},
  3:{C:8},
  6:{C:9},
};
const TERMINALS    = ["TC","TD","$"];
const NONTERMINALS = ["S","C"];

const FIRST = { S:["TC","TD"], C:["TC","TD"] };
const FOLLOW= { S:["$"],       C:["TC","TD","$"] };

/* Productions: 1: S→CC   2: C→TC C   3: C→TD */
const PRODS = [
  { n:1, lhs:"S", rhs:["C","C"] },
  { n:2, lhs:"C", rhs:["TC","C"] },
  { n:3, lhs:"C", rhs:["TD"] },
];

/* Trace of parsing  "TC TC TD TC TD $"  (primera línea de TEST_RAW) */
/* stack shown as [state symbol state ...] */
const TRACE = [
  { stack:[0], remaining:["TC","TC","TD","TC","TD","$"], action:"s3",       desc:"Estado 0, símbolo 'TC' → Shift a I3" },
  { stack:[0,"TC",3], remaining:["TC","TD","TC","TD","$"], action:"s3",     desc:"Estado 3, símbolo 'TC' → Shift a I3" },
  { stack:[0,"TC",3,"TC",3], remaining:["TD","TC","TD","$"], action:"s4",   desc:"Estado 3, símbolo 'TD' → Shift a I4" },
  { stack:[0,"TC",3,"TC",3,"TD",4], remaining:["TC","TD","$"], action:"r3", desc:"Estado 4, ver 'TC' → Reduce 3 (C→TD), GOTO(3,C)=8" },
  { stack:[0,"TC",3,"TC",3,"C",8], remaining:["TC","TD","$"], action:"r2",  desc:"Estado 8, ver 'TC' → Reduce 2 (C→TC C), GOTO(3,C)=8" },
  { stack:[0,"TC",3,"C",8], remaining:["TC","TD","$"], action:"r2",         desc:"Estado 8, ver 'TC' → Reduce 2 (C→TC C), GOTO(0,C)=2" },
  { stack:[0,"C",2], remaining:["TC","TD","$"], action:"s6",                desc:"Estado 2, símbolo 'TC' → Shift a I6" },
  { stack:[0,"C",2,"TC",6], remaining:["TD","$"], action:"s7",              desc:"Estado 6, símbolo 'TD' → Shift a I7" },
  { stack:[0,"C",2,"TC",6,"TD",7], remaining:["$"], action:"r3",            desc:"Estado 7, ver '$' → Reduce 3 (C→TD), GOTO(6,C)=9" },
  { stack:[0,"C",2,"TC",6,"C",9], remaining:["$"], action:"r2",             desc:"Estado 9, ver '$' → Reduce 2 (C→TC C), GOTO(2,C)=5" },
  { stack:[0,"C",2,"C",5], remaining:["$"], action:"r1",                    desc:"Estado 5, ver '$' → Reduce 1 (S→CC), GOTO(0,S)=1" },
  { stack:[0,"S",1], remaining:["$"], action:"acc",                         desc:"Estado 1, ver '$' → ACEPTAR ✓" },
];

const TOKENS = [
  { i:1, k:"TC",  lx:"c",  l:1, c:1 },
  { i:2, k:"TC",  lx:"c",  l:1, c:3 },
  { i:3, k:"TD",  lx:"d",  l:1, c:5 },
  { i:4, k:"TC",  lx:"c",  l:1, c:7 },
  { i:5, k:"TD",  lx:"d",  l:1, c:9 },
  { i:6, k:"EOF", lx:"$",  l:1, c:10 },
];

const PROBLEMS = [
  { level:"info", code:"I100", msg:"gramática LR(1) sin conflictos · 9 estados", loc:"parser.yalp" },
  { level:"info", code:"I101", msg:"lexer sin ambigüedades · 2 reglas de token, 1 de espacios", loc:"lexer.yal" },
];

window.IDE_DATA = {
  FILES, STATES, ACTION, GOTO, TERMINALS, NONTERMINALS,
  FIRST, FOLLOW, PRODS, TRACE, TOKENS, PROBLEMS,
  // Llenado por /api/codegen (ver handleRun en app.jsx) — antes era un mock
  // hardcodeado que nunca reflejaba el .yal cargado.
  GEN_CODE: "",
  LR0_DOT: "",
  PARSE_TREE_DOT: "",
};
