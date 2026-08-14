
#include <ctype.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* ==============================================================
 *  SECCION 1 – TIPOS DE TOKEN  (corresponden a hardtest.yal)
 * ============================================================== */

typedef enum {
  /* Palabras reservadas */
  TOK_DEF = 1,
  TOK_IF,
  TOK_ELSE,
  TOK_ELIF,
  TOK_WHILE,
  TOK_FOR,
  TOK_IN,
  TOK_RETURN,
  TOK_PRINT,
  TOK_TRUE,
  TOK_FALSE,
  TOK_NONE,
  TOK_AND,
  TOK_OR,
  TOK_NOT,
  TOK_CLASS,
  TOK_IMPORT,
  TOK_FROM,
  TOK_PASS,
  TOK_BREAK,
  TOK_CONTINUE,
  /* Literales */
  TOK_ID,
  TOK_INTEGER,
  TOK_FLOAT,
  TOK_STRING,
  TOK_DOCSTRING,
  TOK_COMMENT,
  /* Operadores compuestos */
  TOK_EQ,
  TOK_NEQ,
  TOK_LTE,
  TOK_GTE,
  TOK_PLUS_ASSIGN,
  TOK_MINUS_ASSIGN,
  TOK_POW,
  /* Operadores simples */
  TOK_ASSIGN,
  TOK_PLUS,
  TOK_MINUS,
  TOK_STAR,
  TOK_SLASH,
  TOK_MOD,
  TOK_LT,
  TOK_GT,
  /* Delimitadores */
  TOK_LPAREN,
  TOK_RPAREN,
  TOK_LBRACKET,
  TOK_RBRACKET,
  TOK_LBRACE,
  TOK_RBRACE,
  TOK_COLON,
  TOK_COMMA,
  TOK_DOT,
  /* Control de bloque */
  TOK_NEWLINE,
  TOK_INDENT,
  TOK_DEDENT,
  /* Fin de archivo y error */
  TOK_EOF,
  TOK_ERROR
} TokKind;

static const char *TOK_NAME[] = {[TOK_DEF] = "DEF",
                                 [TOK_IF] = "IF",
                                 [TOK_ELSE] = "ELSE",
                                 [TOK_ELIF] = "ELIF",
                                 [TOK_WHILE] = "WHILE",
                                 [TOK_FOR] = "FOR",
                                 [TOK_IN] = "IN",
                                 [TOK_RETURN] = "RETURN",
                                 [TOK_PRINT] = "PRINT",
                                 [TOK_TRUE] = "TRUE",
                                 [TOK_FALSE] = "FALSE",
                                 [TOK_NONE] = "NONE",
                                 [TOK_AND] = "AND",
                                 [TOK_OR] = "OR",
                                 [TOK_NOT] = "NOT",
                                 [TOK_CLASS] = "CLASS",
                                 [TOK_IMPORT] = "IMPORT",
                                 [TOK_FROM] = "FROM",
                                 [TOK_PASS] = "PASS",
                                 [TOK_BREAK] = "BREAK",
                                 [TOK_CONTINUE] = "CONTINUE",
                                 [TOK_ID] = "ID",
                                 [TOK_INTEGER] = "INTEGER",
                                 [TOK_FLOAT] = "FLOAT",
                                 [TOK_STRING] = "STRING",
                                 [TOK_DOCSTRING] = "DOCSTRING",
                                 [TOK_COMMENT] = "COMMENT",
                                 [TOK_EQ] = "EQ",
                                 [TOK_NEQ] = "NEQ",
                                 [TOK_LTE] = "LTE",
                                 [TOK_GTE] = "GTE",
                                 [TOK_PLUS_ASSIGN] = "PLUS_ASSIGN",
                                 [TOK_MINUS_ASSIGN] = "MINUS_ASSIGN",
                                 [TOK_POW] = "POW",
                                 [TOK_ASSIGN] = "ASSIGN",
                                 [TOK_PLUS] = "PLUS",
                                 [TOK_MINUS] = "MINUS",
                                 [TOK_STAR] = "STAR",
                                 [TOK_SLASH] = "SLASH",
                                 [TOK_MOD] = "MOD",
                                 [TOK_LT] = "LT",
                                 [TOK_GT] = "GT",
                                 [TOK_LPAREN] = "LPAREN",
                                 [TOK_RPAREN] = "RPAREN",
                                 [TOK_LBRACKET] = "LBRACKET",
                                 [TOK_RBRACKET] = "RBRACKET",
                                 [TOK_LBRACE] = "LBRACE",
                                 [TOK_RBRACE] = "RBRACE",
                                 [TOK_COLON] = "COLON",
                                 [TOK_COMMA] = "COMMA",
                                 [TOK_DOT] = "DOT",
                                 [TOK_NEWLINE] = "NEWLINE",
                                 [TOK_INDENT] = "INDENT",
                                 [TOK_DEDENT] = "DEDENT",
                                 [TOK_EOF] = "EOF",
                                 [TOK_ERROR] = "ERROR"};

#define TOKNAME(k)                                                             \
  (((k) < (int)(sizeof(TOK_NAME) / sizeof(*TOK_NAME)) && TOK_NAME[k])          \
       ? TOK_NAME[k]                                                           \
       : "?")

/* ==============================================================
 *  SECCION 2 – LEXER  (implementa reglas de hardtest.yal)
 * ============================================================== */

#define MAX_LEX 512
#define IND_MAX 64
#define QUE_MAX 64

typedef struct {
  TokKind kind;
  char lex[MAX_LEX];
  int line, col;
} Token;

typedef struct {
  const char *src;
  int pos, line, col;
  /* Pila de niveles de indentacion */
  int istack[IND_MAX];
  int itop;
  /* Cola de tokens pendientes (INDENT/DEDENT pueden ser multiples) */
  Token queue[QUE_MAX];
  int qh, qt; /* head, tail */
  /* Bandera: inicio de linea logica */
  int bol; /* beginning-of-line */
  /* Profundidad de parentesis/corchetes (suprime INDENT/DEDENT) */
  int pdepth;
} Lexer;

/* ---------- helpers del lexer ---------- */

static void lex_init(Lexer *l, const char *src) {
  memset(l, 0, sizeof(*l));
  l->src = src;
  l->line = 1;
  l->col = 1;
  l->istack[0] = 0;
  l->itop = 0;
  l->bol = 1;
}

static char lpeek(Lexer *l) { return l->src[l->pos]; }
static char lpeek2(Lexer *l) { return l->src[l->pos + 1]; }
static char lpeek3(Lexer *l) { return l->src[l->pos + 2]; }

static char ladv(Lexer *l) {
  char c = l->src[l->pos++];
  if (c == '\n') {
    l->line++;
    l->col = 1;
  } else
    l->col++;
  return c;
}

static Token mktok(TokKind k, const char *lex, int line, int col) {
  Token t;
  t.kind = k;
  strncpy(t.lex, lex, MAX_LEX - 1);
  t.lex[MAX_LEX - 1] = '\0';
  t.line = line;
  t.col = col;
  return t;
}

static void qpush(Lexer *l, Token t) {
  l->queue[l->qt % QUE_MAX] = t;
  l->qt++;
}
static Token qpop(Lexer *l) {
  Token t = l->queue[l->qh % QUE_MAX];
  l->qh++;
  return t;
}
static int qempty(Lexer *l) { return l->qh == l->qt; }

/* Tabla de palabras reservadas */
typedef struct {
  const char *w;
  TokKind k;
} KwEntry;
static KwEntry KEYWORDS[] = {{"def", TOK_DEF},
                             {"if", TOK_IF},
                             {"else", TOK_ELSE},
                             {"elif", TOK_ELIF},
                             {"while", TOK_WHILE},
                             {"for", TOK_FOR},
                             {"in", TOK_IN},
                             {"return", TOK_RETURN},
                             {"print", TOK_PRINT},
                             {"True", TOK_TRUE},
                             {"False", TOK_FALSE},
                             {"None", TOK_NONE},
                             {"and", TOK_AND},
                             {"or", TOK_OR},
                             {"not", TOK_NOT},
                             {"class", TOK_CLASS},
                             {"import", TOK_IMPORT},
                             {"from", TOK_FROM},
                             {"pass", TOK_PASS},
                             {"break", TOK_BREAK},
                             {"continue", TOK_CONTINUE},
                             {NULL, 0}};

static TokKind kw_lookup(const char *w) {
  for (int i = 0; KEYWORDS[i].w; i++)
    if (strcmp(KEYWORDS[i].w, w) == 0)
      return KEYWORDS[i].k;
  return TOK_ID;
}

/*
 * handle_indent – llamada al inicio de cada linea logica.
 * Cuenta espacios/tabs, luego genera INDENT o DEDENT(s) segun
 * el cambio respecto al nivel previo.
 * Ignora lineas en blanco y lineas de solo comentario.
 */
static void handle_indent(Lexer *l) {
  int ind = 0;
  int saved = l->pos;

  while (lpeek(l) == ' ' || lpeek(l) == '\t') {
    ind += (lpeek(l) == '\t') ? 4 : 1;
    ladv(l);
  }

  char c = lpeek(l);
  /* Linea en blanco o comentario: no cambiar indentacion */
  if (c == '\n' || c == '\r' || c == '#' || c == '\0') {
    (void)saved;
    return;
  }

  /* No generar INDENT/DEDENT dentro de parentesis */
  if (l->pdepth > 0)
    return;

  int cur = l->istack[l->itop];

  if (ind > cur) {
    l->istack[++l->itop] = ind;
    qpush(l, mktok(TOK_INDENT, "INDENT", l->line, 1));
  } else if (ind < cur) {
    while (l->itop > 0 && l->istack[l->itop] > ind) {
      l->itop--;
      qpush(l, mktok(TOK_DEDENT, "DEDENT", l->line, 1));
    }
  }
}

/*
 * lex_raw – devuelve el siguiente token sin filtrar.
 * Incluye COMMENT, DOCSTRING, todos los NEWLINE.
 */
static Token lex_raw(Lexer *l) {
  /* Cola de tokens pendientes (INDENT/DEDENT) */
  if (!qempty(l))
    return qpop(l);

  /* Inicio de linea: procesar indentacion */
  if (l->bol) {
    l->bol = 0;
    handle_indent(l);
    if (!qempty(l))
      return qpop(l);
  }

  /* Saltar blancos inline */
  while (lpeek(l) == ' ' || lpeek(l) == '\t')
    ladv(l);

  int line = l->line, col = l->col;
  char c = lpeek(l);

  /* EOF: cerrar todos los bloques abiertos */
  if (c == '\0') {
    if (l->itop > 0) {
      l->itop--;
      Token eof = mktok(TOK_EOF, "", line, col);
      qpush(l, eof);
      return mktok(TOK_DEDENT, "DEDENT", line, col);
    }
    return mktok(TOK_EOF, "", line, col);
  }

  /* Salto de linea */
  if (c == '\n' || c == '\r') {
    ladv(l);
    if (c == '\r' && lpeek(l) == '\n')
      ladv(l);
    l->bol = 1;
    return mktok(TOK_NEWLINE, "\\n", line, col);
  }

  /* Comentario de linea */
  if (c == '#') {
    char buf[MAX_LEX];
    int i = 0;
    ladv(l);
    while (lpeek(l) != '\n' && lpeek(l) != '\r' && lpeek(l) != '\0')
      if (i < MAX_LEX - 1)
        buf[i++] = ladv(l);
      else
        ladv(l);
    buf[i] = '\0';
    return mktok(TOK_COMMENT, buf, line, col);
  }

  /* Docstring / triple comilla doble """...""" */
  if (c == '"' && lpeek2(l) == '"' && lpeek3(l) == '"') {
    char buf[MAX_LEX];
    int i = 0;
    ladv(l);
    ladv(l);
    ladv(l); /* skip """ */
    while (!(lpeek(l) == '"' && lpeek2(l) == '"' && lpeek3(l) == '"') &&
           lpeek(l) != '\0')
      if (i < MAX_LEX - 1)
        buf[i++] = ladv(l);
      else
        ladv(l);
    if (lpeek(l) == '"') {
      ladv(l);
      ladv(l);
      ladv(l);
    }
    buf[i] = '\0';
    return mktok(TOK_DOCSTRING, buf, line, col);
  }

  /* String entre comillas dobles "..." */
  if (c == '"') {
    char buf[MAX_LEX];
    int i = 0;
    ladv(l);
    while (lpeek(l) != '"' && lpeek(l) != '\n' && lpeek(l) != '\0') {
      char ch = ladv(l);
      if (ch == '\\' && lpeek(l) == '"') {
        if (i < MAX_LEX - 1)
          buf[i++] = '"';
        ladv(l);
      } else {
        if (i < MAX_LEX - 1)
          buf[i++] = ch;
      }
    }
    if (lpeek(l) == '"')
      ladv(l);
    buf[i] = '\0';
    return mktok(TOK_STRING, buf, line, col);
  }

  /* String entre comillas simples '...' */
  if (c == '\'') {
    char buf[MAX_LEX];
    int i = 0;
    ladv(l);
    while (lpeek(l) != '\'' && lpeek(l) != '\n' && lpeek(l) != '\0') {
      char ch = ladv(l);
      if (ch == '\\' && lpeek(l) == '\'') {
        if (i < MAX_LEX - 1)
          buf[i++] = '\'';
        ladv(l);
      } else {
        if (i < MAX_LEX - 1)
          buf[i++] = ch;
      }
    }
    if (lpeek(l) == '\'')
      ladv(l);
    buf[i] = '\0';
    return mktok(TOK_STRING, buf, line, col);
  }

  /* Numeros */
  if (isdigit((unsigned char)c)) {
    char buf[MAX_LEX];
    int i = 0;
    int is_f = 0;
    while (isdigit((unsigned char)lpeek(l)))
      buf[i++] = ladv(l);
    if (lpeek(l) == '.' && isdigit((unsigned char)lpeek2(l))) {
      is_f = 1;
      buf[i++] = ladv(l);
      while (isdigit((unsigned char)lpeek(l)))
        buf[i++] = ladv(l);
    }
    if (lpeek(l) == 'e' || lpeek(l) == 'E') {
      is_f = 1;
      buf[i++] = ladv(l);
      if (lpeek(l) == '+' || lpeek(l) == '-')
        buf[i++] = ladv(l);
      while (isdigit((unsigned char)lpeek(l)))
        buf[i++] = ladv(l);
    }
    buf[i] = '\0';
    return mktok(is_f ? TOK_FLOAT : TOK_INTEGER, buf, line, col);
  }

  /* Identificadores y palabras reservadas */
  if (isalpha((unsigned char)c) || c == '_') {
    char buf[MAX_LEX];
    int i = 0;
    while (isalnum((unsigned char)lpeek(l)) || lpeek(l) == '_')
      if (i < MAX_LEX - 1)
        buf[i++] = ladv(l);
      else
        ladv(l);
    buf[i] = '\0';
    return mktok(kw_lookup(buf), buf, line, col);
  }

  /* Operadores y delimitadores */
  ladv(l);
  char c2 = lpeek(l);

  if (c == '=' && c2 == '=') {
    ladv(l);
    return mktok(TOK_EQ, "==", line, col);
  }
  if (c == '!' && c2 == '=') {
    ladv(l);
    return mktok(TOK_NEQ, "!=", line, col);
  }
  if (c == '<' && c2 == '=') {
    ladv(l);
    return mktok(TOK_LTE, "<=", line, col);
  }
  if (c == '>' && c2 == '=') {
    ladv(l);
    return mktok(TOK_GTE, ">=", line, col);
  }
  if (c == '+' && c2 == '=') {
    ladv(l);
    return mktok(TOK_PLUS_ASSIGN, "+=", line, col);
  }
  if (c == '-' && c2 == '=') {
    ladv(l);
    return mktok(TOK_MINUS_ASSIGN, "-=", line, col);
  }
  if (c == '*' && c2 == '*') {
    ladv(l);
    return mktok(TOK_POW, "**", line, col);
  }

  switch (c) {
  case '=':
    return mktok(TOK_ASSIGN, "=", line, col);
  case '+':
    return mktok(TOK_PLUS, "+", line, col);
  case '-':
    return mktok(TOK_MINUS, "-", line, col);
  case '*':
    return mktok(TOK_STAR, "*", line, col);
  case '/':
    return mktok(TOK_SLASH, "/", line, col);
  case '%':
    return mktok(TOK_MOD, "%", line, col);
  case '<':
    return mktok(TOK_LT, "<", line, col);
  case '>':
    return mktok(TOK_GT, ">", line, col);
  case '(':
    return mktok(TOK_LPAREN, "(", line, col);
  case ')':
    return mktok(TOK_RPAREN, ")", line, col);
  case '[':
    return mktok(TOK_LBRACKET, "[", line, col);
  case ']':
    return mktok(TOK_RBRACKET, "]", line, col);
  case '{':
    return mktok(TOK_LBRACE, "{", line, col);
  case '}':
    return mktok(TOK_RBRACE, "}", line, col);
  case ':':
    return mktok(TOK_COLON, ":", line, col);
  case ',':
    return mktok(TOK_COMMA, ",", line, col);
  case '.':
    return mktok(TOK_DOT, ".", line, col);
  }

  char err[4] = {c, '\0'};
  return mktok(TOK_ERROR, err, line, col);
}

/*
 * lex_next – lexer publico para el parser.
 * Filtra COMMENT y DOCSTRING.
 * Suprime NEWLINE/INDENT/DEDENT dentro de parentesis.
 */
static Token lex_next(Lexer *l) {
  for (;;) {
    Token t = lex_raw(l);
    if (t.kind == TOK_COMMENT || t.kind == TOK_DOCSTRING)
      continue;

    if (t.kind == TOK_LPAREN || t.kind == TOK_LBRACKET || t.kind == TOK_LBRACE)
      l->pdepth++;
    if (t.kind == TOK_RPAREN || t.kind == TOK_RBRACKET || t.kind == TOK_RBRACE)
      if (l->pdepth > 0)
        l->pdepth--;

    if (l->pdepth > 0 &&
        (t.kind == TOK_NEWLINE || t.kind == TOK_INDENT || t.kind == TOK_DEDENT))
      continue;

    return t;
  }
}

/* ==============================================================
 *  SECCION 3 – PARSER  (descenso recursivo; implementa hardtest.yalp)
 * ============================================================== */

#define MAX_ERRORS 30

typedef struct {
  Lexer *lex;
  Token cur;
  Token prev;
  Token look; /* lookahead de 1 token */
  int has_look;
  int depth; /* profundidad del arbol para indentacion visual */
  int errors;
} Parser;

/* ---- utilidades de impresion del arbol ---- */
static void pindent(int d) {
  for (int i = 0; i < d; i++)
    printf("  ");
}

#define PNODE(p, fmt, ...)                                                     \
  do {                                                                         \
    pindent((p)->depth);                                                       \
    printf(fmt "\n", ##__VA_ARGS__);                                           \
  } while (0)

/* ---- avance y consulta de tokens ---- */
static void padv(Parser *p) {
  p->prev = p->cur;
  if (p->has_look) {
    p->cur = p->look;
    p->has_look = 0;
  } else {
    p->cur = lex_next(p->lex);
  }
}

static int pcheck(Parser *p, TokKind k) { return p->cur.kind == k; }

static int pmatch(Parser *p, TokKind k) {
  if (p->cur.kind == k) {
    padv(p);
    return 1;
  }
  return 0;
}

static void pexpect(Parser *p, TokKind k) {
  if (p->cur.kind == k) {
    padv(p);
  } else {
    p->errors++;
    fprintf(stderr,
            "\033[31m[ERROR Linea %d:%d] Esperaba %s, se encontro %s "
            "('%s')\033[0m\n",
            p->cur.line, p->cur.col, TOKNAME(k), TOKNAME(p->cur.kind),
            p->cur.lex);
    if (p->errors >= MAX_ERRORS) {
      fprintf(stderr, "Demasiados errores.\n");
      exit(1);
    }
  }
}

/* Declaraciones anticipadas */
static void parse_stmt_list(Parser *p);
static void parse_stmt(Parser *p);
static void parse_expr(Parser *p);

/* ---- suite: NEWLINE INDENT stmt_list DEDENT ---- */
static void parse_suite(Parser *p) {
  /* Saltar todos los NEWLINEs (incluyendo los generados por
     comentarios/docstrings ya filtrados por lex_next). */
  while (pcheck(p, TOK_NEWLINE))
    padv(p);
  if (pcheck(p, TOK_INDENT)) {
    padv(p);
    PNODE(p, "[BLOQUE_INICIO]");
    p->depth++;
    parse_stmt_list(p);
    p->depth--;
    if (pcheck(p, TOK_DEDENT))
      padv(p);
    PNODE(p, "[BLOQUE_FIN]");
  } else {
    /* Suite de una sola sentencia (raro pero posible con error de indentacion)
     */
    parse_stmt(p);
  }
}

/* ---- atom ---- */
static void parse_atom(Parser *p) {
  if (pcheck(p, TOK_ID) || pcheck(p, TOK_INTEGER) || pcheck(p, TOK_FLOAT) ||
      pcheck(p, TOK_STRING) || pcheck(p, TOK_TRUE) || pcheck(p, TOK_FALSE) ||
      pcheck(p, TOK_NONE)) {
    PNODE(p, "[ATOM %s \"%s\"]", TOKNAME(p->cur.kind), p->cur.lex);
    padv(p);
  } else if (pcheck(p, TOK_LPAREN)) {
    padv(p);
    PNODE(p, "[GRUPO (]");
    p->depth++;
    parse_expr(p);
    p->depth--;
    pexpect(p, TOK_RPAREN);
    PNODE(p, "[GRUPO )]");
  } else if (pcheck(p, TOK_LBRACKET)) {
    padv(p);
    PNODE(p, "[LISTA_LITERAL");
    p->depth++;
    if (!pcheck(p, TOK_RBRACKET)) {
      parse_expr(p);
      while (pmatch(p, TOK_COMMA) && !pcheck(p, TOK_RBRACKET))
        parse_expr(p);
    }
    p->depth--;
    pexpect(p, TOK_RBRACKET);
    PNODE(p, "]");
  } else if (pcheck(p, TOK_LBRACE)) {
    padv(p);
    PNODE(p, "[DICT_LITERAL {");
    p->depth++;
    if (!pcheck(p, TOK_RBRACE)) {
      parse_expr(p);
      pexpect(p, TOK_COLON);
      parse_expr(p);
      while (pmatch(p, TOK_COMMA) && !pcheck(p, TOK_RBRACE)) {
        parse_expr(p);
        pexpect(p, TOK_COLON);
        parse_expr(p);
      }
    }
    p->depth--;
    pexpect(p, TOK_RBRACE);
    PNODE(p, "}]");
  } else {
    p->errors++;
    PNODE(p, "[ERROR: expresion esperada, se encontro %s '%s']",
          TOKNAME(p->cur.kind), p->cur.lex);
    padv(p);
  }
}

/* ---- primary: atom ( '(' args ')' | '[' expr ']' | '.' ID )* ---- */
static void parse_primary(Parser *p) {
  parse_atom(p);
  for (;;) {
    if (pcheck(p, TOK_LPAREN)) {
      padv(p);
      PNODE(p, "[LLAMADA(");
      p->depth++;
      if (!pcheck(p, TOK_RPAREN)) {
        parse_expr(p);
        while (pmatch(p, TOK_COMMA) && !pcheck(p, TOK_RPAREN))
          parse_expr(p);
      }
      p->depth--;
      pexpect(p, TOK_RPAREN);
      PNODE(p, ")]");
    } else if (pcheck(p, TOK_LBRACKET)) {
      padv(p);
      PNODE(p, "[INDICE[");
      p->depth++;
      parse_expr(p);
      p->depth--;
      pexpect(p, TOK_RBRACKET);
      PNODE(p, "]]");
    } else if (pcheck(p, TOK_DOT)) {
      padv(p);
      PNODE(p, "[MIEMBRO .%s]", p->cur.lex);
      pexpect(p, TOK_ID);
    } else {
      break;
    }
  }
}

/* ---- unary: ('-' | 'not') unary | primary ---- */
static void parse_unary(Parser *p) {
  if (pcheck(p, TOK_MINUS)) {
    PNODE(p, "[NEG_UNARIA");
    p->depth++;
    padv(p);
    parse_unary(p);
    p->depth--;
    PNODE(p, "]");
  } else if (pcheck(p, TOK_NOT)) {
    PNODE(p, "[NOT_UNARIO");
    p->depth++;
    padv(p);
    parse_unary(p);
    p->depth--;
    PNODE(p, "]");
  } else {
    parse_primary(p);
  }
}

/* ---- power: unary ('**' power)? ---- */
static void parse_power(Parser *p) {
  parse_unary(p);
  if (pcheck(p, TOK_POW)) {
    padv(p);
    PNODE(p, "[POW **");
    p->depth++;
    parse_power(p); /* derecho-asociativo */
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- term: power (('*'|'/'|'%') power)* ---- */
static void parse_term(Parser *p) {
  parse_power(p);
  while (pcheck(p, TOK_STAR) || pcheck(p, TOK_SLASH) || pcheck(p, TOK_MOD)) {
    const char *op = TOKNAME(p->cur.kind);
    padv(p);
    PNODE(p, "[OP_MUL %s", op);
    p->depth++;
    parse_power(p);
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- arith_expr: term (('+' | '-') term)* ---- */
static void parse_arith_expr(Parser *p) {
  parse_term(p);
  while (pcheck(p, TOK_PLUS) || pcheck(p, TOK_MINUS)) {
    const char *op = TOKNAME(p->cur.kind);
    padv(p);
    PNODE(p, "[OP_ADD %s", op);
    p->depth++;
    parse_term(p);
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- comparison: arith_expr (CMP arith_expr)* ---- */
static void parse_comparison(Parser *p) {
  parse_arith_expr(p);
  while (pcheck(p, TOK_EQ) || pcheck(p, TOK_NEQ) || pcheck(p, TOK_LT) ||
         pcheck(p, TOK_GT) || pcheck(p, TOK_LTE) || pcheck(p, TOK_GTE)) {
    const char *op = TOKNAME(p->cur.kind);
    padv(p);
    PNODE(p, "[CMP %s", op);
    p->depth++;
    parse_arith_expr(p);
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- not_expr: 'not' not_expr | comparison ---- */
static void parse_not_expr(Parser *p) {
  if (pcheck(p, TOK_NOT)) {
    padv(p);
    PNODE(p, "[NOT");
    p->depth++;
    parse_not_expr(p);
    p->depth--;
    PNODE(p, "]");
  } else {
    parse_comparison(p);
  }
}

/* ---- and_expr: not_expr ('and' not_expr)* ---- */
static void parse_and_expr(Parser *p) {
  parse_not_expr(p);
  while (pcheck(p, TOK_AND)) {
    padv(p);
    PNODE(p, "[AND");
    p->depth++;
    parse_not_expr(p);
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- or_expr: and_expr ('or' and_expr)* ---- */
static void parse_or_expr(Parser *p) {
  parse_and_expr(p);
  while (pcheck(p, TOK_OR)) {
    padv(p);
    PNODE(p, "[OR");
    p->depth++;
    parse_and_expr(p);
    p->depth--;
    PNODE(p, "]");
  }
}

static void parse_expr(Parser *p) { parse_or_expr(p); }

/* ---- asignacion / expresion como sentencia ---- */
static void parse_assign_or_expr(Parser *p) {
  /* Intentamos leer la parte izquierda como expresion.
     Si luego viene '=' / '+=' / '-=', es asignacion. */
  parse_expr(p);
  if (pcheck(p, TOK_ASSIGN)) {
    const char *op = TOKNAME(p->cur.kind);
    padv(p);
    PNODE(p, "[ASIGNACION %s", op);
    p->depth++;
    /* Permitir asignacion encadenada: a = b = c */
    parse_expr(p);
    while (pcheck(p, TOK_ASSIGN)) {
      padv(p);
      parse_expr(p);
    }
    p->depth--;
    PNODE(p, "]");
  } else if (pcheck(p, TOK_PLUS_ASSIGN) || pcheck(p, TOK_MINUS_ASSIGN)) {
    const char *op = TOKNAME(p->cur.kind);
    padv(p);
    PNODE(p, "[AUG_ASSIGN %s", op);
    p->depth++;
    parse_expr(p);
    p->depth--;
    PNODE(p, "]");
  }
}

/* ---- return ---- */
static void parse_return(Parser *p) {
  pexpect(p, TOK_RETURN);
  PNODE(p, "[RETURN");
  if (!pcheck(p, TOK_NEWLINE) && !pcheck(p, TOK_DEDENT) &&
      !pcheck(p, TOK_EOF)) {
    p->depth++;
    parse_expr(p);
    p->depth--;
  }
  PNODE(p, "]");
}

/* ---- print ---- */
static void parse_print(Parser *p) {
  pexpect(p, TOK_PRINT);
  pexpect(p, TOK_LPAREN);
  PNODE(p, "[PRINT(");
  p->depth++;
  if (!pcheck(p, TOK_RPAREN)) {
    parse_expr(p);
    while (pmatch(p, TOK_COMMA) && !pcheck(p, TOK_RPAREN))
      parse_expr(p);
  }
  p->depth--;
  pexpect(p, TOK_RPAREN);
  PNODE(p, ")]");
}

/* ---- if / elif / else ---- */
static void parse_if(Parser *p) {
  pexpect(p, TOK_IF);
  PNODE(p, "[IF");
  p->depth++;
  parse_expr(p);
  pexpect(p, TOK_COLON);
  parse_suite(p);
  /* elif chain */
  while (pcheck(p, TOK_ELIF)) {
    padv(p);
    PNODE(p, "[ELIF");
    parse_expr(p);
    pexpect(p, TOK_COLON);
    parse_suite(p);
    PNODE(p, "]");
  }
  /* else */
  if (pcheck(p, TOK_ELSE)) {
    padv(p);
    PNODE(p, "[ELSE");
    pexpect(p, TOK_COLON);
    parse_suite(p);
    PNODE(p, "]");
  }
  p->depth--;
  PNODE(p, "]");
}

/* ---- while ---- */
static void parse_while(Parser *p) {
  pexpect(p, TOK_WHILE);
  PNODE(p, "[WHILE");
  p->depth++;
  parse_expr(p);
  pexpect(p, TOK_COLON);
  parse_suite(p);
  p->depth--;
  PNODE(p, "]");
}

/* ---- for ---- */
static void parse_for(Parser *p) {
  pexpect(p, TOK_FOR);
  char target[MAX_LEX];
  strncpy(target, p->cur.lex, MAX_LEX - 1);
  pexpect(p, TOK_ID);
  pexpect(p, TOK_IN);
  PNODE(p, "[FOR %s IN", target);
  p->depth++;
  parse_expr(p);
  pexpect(p, TOK_COLON);
  parse_suite(p);
  p->depth--;
  PNODE(p, "]");
}

/* ---- def ---- */
static void parse_funcdef(Parser *p) {
  pexpect(p, TOK_DEF);
  char fname[MAX_LEX];
  strncpy(fname, p->cur.lex, MAX_LEX - 1);
  pexpect(p, TOK_ID);
  pexpect(p, TOK_LPAREN);
  /* Parametros */
  char params_buf[MAX_LEX * 4] = "";
  if (!pcheck(p, TOK_RPAREN)) {
    strncat(params_buf, p->cur.lex, sizeof(params_buf) - 1);
    pexpect(p, TOK_ID);
    while (pcheck(p, TOK_COMMA)) {
      padv(p);
      strncat(params_buf, ", ", sizeof(params_buf) - 1);
      strncat(params_buf, p->cur.lex, sizeof(params_buf) - 1);
      if (pcheck(p, TOK_ID))
        padv(p);
    }
  }
  pexpect(p, TOK_RPAREN);
  pexpect(p, TOK_COLON);
  PNODE(p, "[DEF %s(%s)", fname, params_buf);
  p->depth++;
  parse_suite(p);
  p->depth--;
  PNODE(p, "]");
}

/* ---- class ---- */
static void parse_classdef(Parser *p) {
  pexpect(p, TOK_CLASS);
  char cname[MAX_LEX];
  strncpy(cname, p->cur.lex, MAX_LEX - 1);
  pexpect(p, TOK_ID);
  char base[MAX_LEX] = "";
  if (pcheck(p, TOK_LPAREN)) {
    padv(p);
    strncpy(base, p->cur.lex, MAX_LEX - 1);
    pexpect(p, TOK_ID);
    pexpect(p, TOK_RPAREN);
  }
  pexpect(p, TOK_COLON);
  if (base[0])
    PNODE(p, "[CLASS %s(%s)", cname, base);
  else
    PNODE(p, "[CLASS %s", cname);
  p->depth++;
  parse_suite(p);
  p->depth--;
  PNODE(p, "]");
}

/* ---- sentencia individual ---- */
static void parse_stmt(Parser *p) {
  /* Saltar NEWLINEs sueltos (lineas en blanco) */
  while (pcheck(p, TOK_NEWLINE))
    padv(p);

  switch (p->cur.kind) {
  case TOK_DEF:
    parse_funcdef(p);
    break;
  case TOK_IF:
    parse_if(p);
    break;
  case TOK_WHILE:
    parse_while(p);
    break;
  case TOK_FOR:
    parse_for(p);
    break;
  case TOK_CLASS:
    parse_classdef(p);
    break;
  case TOK_RETURN:
    parse_return(p);
    pmatch(p, TOK_NEWLINE);
    break;
  case TOK_PRINT:
    parse_print(p);
    pmatch(p, TOK_NEWLINE);
    break;
  case TOK_PASS:
    PNODE(p, "[PASS]");
    padv(p);
    pmatch(p, TOK_NEWLINE);
    break;
  case TOK_BREAK:
    PNODE(p, "[BREAK]");
    padv(p);
    pmatch(p, TOK_NEWLINE);
    break;
  case TOK_CONTINUE:
    PNODE(p, "[CONTINUE]");
    padv(p);
    pmatch(p, TOK_NEWLINE);
    break;
  case TOK_IMPORT: {
    padv(p);
    PNODE(p, "[IMPORT %s]", p->cur.lex);
    while (!pcheck(p, TOK_NEWLINE) && !pcheck(p, TOK_EOF))
      padv(p);
    pmatch(p, TOK_NEWLINE);
    break;
  }
  case TOK_FROM: {
    padv(p);
    PNODE(p, "[FROM %s IMPORT ...]", p->cur.lex);
    while (!pcheck(p, TOK_NEWLINE) && !pcheck(p, TOK_EOF))
      padv(p);
    pmatch(p, TOK_NEWLINE);
    break;
  }
  case TOK_EOF:
    return;
  case TOK_DEDENT:
    return;
  default:
    parse_assign_or_expr(p);
    pmatch(p, TOK_NEWLINE);
    break;
  }
}

static void parse_stmt_list(Parser *p) {
  while (pcheck(p, TOK_NEWLINE))
    padv(p);
  while (!pcheck(p, TOK_EOF) && !pcheck(p, TOK_DEDENT))
    parse_stmt(p);
}

/* ==============================================================
 *  SECCION 4 – FUENTE EMBEBIDA  (= contenido de hardtest.py)
 * ============================================================== */

static const char *HARDTEST_PY =
    "def calcularSuma(n1, n2):\n"
    "    # Esta funcion recibe dos numeros y devuelve su suma\n"
    "    \"\"\"\n"
    "        ESTO ES UN COMENTARIO MULTILINEA!\n"
    "    \"\"\"\n"
    "    # Que hago aqui ?\n"
    "    x = n1 + n2\n"
    "    return x\n"
    "\n"
    "def calcularResta(n1, n2):\n"
    "    # Esta funcion recibe dos numeros y devuelve su resta\n"
    "\n"
    "    x = n1 - n2\n"
    "    return x\n"
    "\n"
    "x = 5\n"
    "y = 3\n"
    "suma = calcularSuma(x, y)\n"
    "resta = calcularResta(x, y)\n"
    "print(\"La suma de \",x,\" y \",y,\" es \",suma)\n"
    "\n"
    "if suma > 10:\n"
    "    print(\"La suma es mayor que 10\")\n"
    "else:\n"
    "    while (resta > 0):\n"
    "        print(\"La resta es \",resta)\n"
    "        resta = resta - 1\n"
    "print(\"Fin del programa\")\n"
    "adios = \"adios\"\n"
    "for i in adios:\n"
    "    print(i)\n";

/* ==============================================================
 *  SECCION 5 – MAIN
 * ============================================================== */

int main(void) {

  /* ---- FASE 1: ANALISIS LEXICO ---- */
  printf("╔══════════════════════════════════════════════════════════╗\n");
  printf("║  FASE 1 — ANALISIS LEXICO   (hardtest.yal)               ║\n");
  printf("╚══════════════════════════════════════════════════════════╝\n\n");
  printf("Archivo fuente: hardtest.py\n\n");

  /* Imprimimos TODOS los tokens incluyendo COMMENT/DOCSTRING */
  {
    Lexer raw;
    lex_init(&raw, HARDTEST_PY);

    printf("  %-5s %-5s  %-22s  %s\n", "LIN", "COL", "TOKEN", "LEXEMA");
    printf("  %-5s %-5s  %-22s  %s\n", "---", "---", "-----", "------");

    int total = 0;
    for (;;) {
      Token t = lex_raw(&raw);
      /* Truncar lexemas largos */
      char trunc[38];
      size_t llen = strlen(t.lex);
      if (llen > 35) {
        strncpy(trunc, t.lex, 32);
        trunc[32] = '.';
        trunc[33] = '.';
        trunc[34] = '.';
        trunc[35] = '\0';
      } else {
        strncpy(trunc, t.lex, 37);
        trunc[37] = '\0';
      }
      printf("  %-5d %-5d  %-22s  %s\n", t.line, t.col, TOKNAME(t.kind), trunc);
      total++;
      if (t.kind == TOK_EOF)
        break;
    }
    printf("\nTotal de tokens (con COMMENT/DOCSTRING): %d\n", total);
  }

  /* ---- FASE 2: ANALISIS SINTACTICO ---- */
  printf("\n╔══════════════════════════════════════════════════════════╗\n");
  printf("║  FASE 2 — ANALISIS SINTACTICO  (hardtest.yalp)           ║\n");
  printf("╚══════════════════════════════════════════════════════════╝\n\n");
  printf("Gramatica: LALR  (simulado con descenso recursivo)\n");
  printf("INDENT/DEDENT generados por el lexer (cambios de nivel)\n");
  printf("COMMENT y DOCSTRING ignorados por el parser\n\n");
  printf("[PROGRAMA]\n");

  Lexer lex;
  lex_init(&lex, HARDTEST_PY);

  Parser parser;
  memset(&parser, 0, sizeof(parser));
  parser.lex = &lex;
  parser.depth = 1;
  padv(&parser); /* cargar primer token */

  parse_stmt_list(&parser);

  if (!pcheck(&parser, TOK_EOF))
    fprintf(stderr,
            "\n[ADVERTENCIA] Tokens sin consumir. Ultimo visto: %s '%s'\n",
            TOKNAME(parser.cur.kind), parser.cur.lex);

  /* ---- FASE 3: RESUMEN ---- */
  printf("\n╔══════════════════════════════════════════════════════════╗\n");
  printf("║  FASE 3 — RESUMEN DEL ANALISIS                           ║\n");
  printf("╚══════════════════════════════════════════════════════════╝\n");
  printf("\n  Archivo fuente  : hardtest.py\n");
  printf("  Lexer definido  : hardtest.yal\n");
  printf("  Gramatica       : hardtest.yalp\n");
  if (parser.errors == 0)
    printf("  Estado          : \033[32mOK — programa valido\033[0m\n\n");
  else
    printf("  Estado          : \033[31mERROR — %d error(es) "
           "encontrado(s)\033[0m\n\n",
           parser.errors);

  return parser.errors > 0 ? 1 : 0;
}
