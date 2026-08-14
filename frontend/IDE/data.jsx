/* ============================================================================
   Mock data — LR(1) clásico:   S' → S    S → C C    C → c C  |  d
   ========================================================================== */

// Contenido crudo de cada archivo — usado por el API cuando se hace RUN / PARSEAR
const YAL_RAW = `let digit = ['0'-'9']
let letter = ['a'-'z' 'A'-'Z']
let id = letter (letter|digit)*
rule tokens =
  | id         { return ID }
  | digit+     { return NUM }
  | '+'        { return PLUS }
  | '*'        { return STAR }
  | ' '        { skip }`;

const YALP_RAW = `%token c d
%%
S : C C ;
C : c C
  | d
;`;

const TEST_RAW = `c c d c d
c d d
d c d`;

const FILES = {
  yal:  { name: "lexer.yal",   path: "src/lexer.yal",   kind:"yal",  current: 9, badges:{warn:1}, rawContent: YAL_RAW,  dirty: false },
  yalp: { name: "parser.yalp", path: "src/parser.yalp", kind:"yalp", current: 7,                  rawContent: YALP_RAW, dirty: false },
  test: { name: "input.txt",   path: "tests/input.txt", kind:"txt",  current: 2,                  rawContent: TEST_RAW, dirty: false },
};

/* ---- LR(1) states for S' → S    S → CC    C → cC | d ---- */
const STATES = [
  { id:0, items:[ "S' → · S , $", "S → · C C , $", "C → · c C , c/d", "C → · d , c/d" ] },
  { id:1, items:[ "S' → S · , $" ] },
  { id:2, items:[ "S → C · C , $", "C → · c C , $", "C → · d , $" ] },
  { id:3, items:[ "C → c · C , c/d", "C → · c C , c/d", "C → · d , c/d" ] },
  { id:4, items:[ "C → d · , c/d" ] },
  { id:5, items:[ "S → C C · , $" ] },
  { id:6, items:[ "C → c · C , $", "C → · c C , $", "C → · d , $" ] },
  { id:7, items:[ "C → d · , $" ] },
  { id:8, items:[ "C → c C · , c/d" ] },
  { id:9, items:[ "C → c C · , $" ] },
];

/* ACTION[state][terminal]   GOTO[state][nonterminal] */
/* terminals: c d $   nonterminals: S C */
const ACTION = {
  0:{c:"s3", d:"s4"},
  1:{$:"acc"},
  2:{c:"s6", d:"s7"},
  3:{c:"s3", d:"s4"},
  4:{c:"r3", d:"r3"},
  5:{$:"r1"},
  6:{c:"s6", d:"s7"},
  7:{$:"r3"},
  8:{c:"r2", d:"r2"},
  9:{$:"r2"},
};
const GOTO = {
  0:{S:1, C:2},
  2:{C:5},
  3:{C:8},
  6:{C:9},
};
const TERMINALS    = ["c","d","$"];
const NONTERMINALS = ["S","C"];

const FIRST = { S:["c","d"], C:["c","d"] };
const FOLLOW= { S:["$"],     C:["c","d","$"] };

/* Productions: 1: S→CC   2: C→cC   3: C→d */
const PRODS = [
  { n:1, lhs:"S", rhs:["C","C"] },
  { n:2, lhs:"C", rhs:["c","C"] },
  { n:3, lhs:"C", rhs:["d"] },
];

/* Trace of parsing  "c c d c d $"  (input from spec) */
/* stack shown as [state symbol state ...] */
const TRACE = [
  { stack:[0], remaining:["c","c","d","c","d","$"], action:"s3",      desc:"Estado 0, símbolo 'c' → Shift a I3" },
  { stack:[0,"c",3], remaining:["c","d","c","d","$"], action:"s3",    desc:"Estado 3, símbolo 'c' → Shift a I3" },
  { stack:[0,"c",3,"c",3], remaining:["d","c","d","$"], action:"s4",  desc:"Estado 3, símbolo 'd' → Shift a I4" },
  { stack:[0,"c",3,"c",3,"d",4], remaining:["c","d","$"], action:"r3",desc:"Estado 4, ver 'c' → Reduce 3 (C→d), GOTO(3,C)=8" },
  { stack:[0,"c",3,"c",3,"C",8], remaining:["c","d","$"], action:"r2",desc:"Estado 8, ver 'c' → Reduce 2 (C→cC), GOTO(3,C)=8" },
  { stack:[0,"c",3,"C",8], remaining:["c","d","$"], action:"r2",      desc:"Estado 8, ver 'c' → Reduce 2 (C→cC), GOTO(0,C)=2" },
  { stack:[0,"C",2], remaining:["c","d","$"], action:"s6",            desc:"Estado 2, símbolo 'c' → Shift a I6" },
  { stack:[0,"C",2,"c",6], remaining:["d","$"], action:"s7",          desc:"Estado 6, símbolo 'd' → Shift a I7" },
  { stack:[0,"C",2,"c",6,"d",7], remaining:["$"], action:"r3",        desc:"Estado 7, ver '$' → Reduce 3 (C→d), GOTO(6,C)=9" },
  { stack:[0,"C",2,"c",6,"C",9], remaining:["$"], action:"r2",        desc:"Estado 9, ver '$' → Reduce 2 (C→cC), GOTO(2,C)=5" },
  { stack:[0,"C",2,"C",5], remaining:["$"], action:"r1",              desc:"Estado 5, ver '$' → Reduce 1 (S→CC), GOTO(0,S)=1" },
  { stack:[0,"S",1], remaining:["$"], action:"acc",                   desc:"Estado 1, ver '$' → ACEPTAR ✓" },
];

const TOKENS = [
  { i:1, k:"C",   lx:"c",  l:1, c:1 },
  { i:2, k:"C",   lx:"c",  l:1, c:3 },
  { i:3, k:"D",   lx:"d",  l:1, c:5 },
  { i:4, k:"C",   lx:"c",  l:1, c:7 },
  { i:5, k:"D",   lx:"d",  l:1, c:9 },
  { i:6, k:"EOF", lx:"$",  l:1, c:10 },
];

const PROBLEMS = [
  { level:"warn", code:"W201", msg:"regla 'skip' sin acción explícita", loc:"lexer.yal:11" },
  { level:"info", code:"I100", msg:"gramática es LR(1) sin conflictos", loc:"parser.yalp" },
  { level:"warn", code:"W104", msg:"terminal 'c' usado 4 veces — considera token nombrado", loc:"parser.yalp:9" },
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
