/* eslint-disable */
// IDE-lite: mismo look & layout pixel/retro que frontend/IDE-full/app.jsx
// (mismo CSS, mismo editor con resaltado, misma sidebar de archivos), pero
// recortado a solo lo que hace falta para probar el análisis semántico:
// flujo de tokens, árbol sintáctico (que sale auto-anotado con el tipo de
// cada expresión en cuanto hay análisis semántico — es el "árbol de análisis
// anotado" del libro del dragón, ver src/sintactico/runtime/parse_tree.rs),
// tabla de símbolos (estado final + por entorno, incluidos bloques anónimos),
// tabla de tipos por nodo, y reporte de errores semánticos. Se sacaron
// GRAMÁTICA/FIRST/FOLLOW/ESTADOS/ACTION-GOTO/LR(0)/CÓD.GEN/CLOSURES y el
// stepper PARSE CONSOLE — con eso también se fue la necesidad de
// STATES/ACTION/GOTO/FIRST/FOLLOW/PRODS/TRACE/GEN_CODE/LR0_DOT/CLOSURES en
// `data.jsx`. Ver frontend/IDE-full/ para esas vistas.
const { useState, useEffect, useRef } = React;
const D = window.IDE_DATA;

const API = "http://localhost:8080";

/* ============================== Helpers ============================== */

function FileTree({ active, onPick, onLoadFile }){
  return (
    <div className="filetree">
      <div className="h">▍ CARGAR ARCHIVOS</div>
      <div className="load-btns">
        <label className="load-btn">
          ↑ .yal / .yalex
          <input type="file" accept=".yal,.yalex" hidden onChange={e => e.target.files[0] && onLoadFile("yal", e.target.files[0])}/>
        </label>
        <label className="load-btn">
          ↑ .yalp / .yapar
          <input type="file" accept=".yalp,.yapar" hidden onChange={e => e.target.files[0] && onLoadFile("yalp", e.target.files[0])}/>
        </label>
        <label className="load-btn">
          ↑ input.txt / .cps
          <input type="file" accept=".txt,.cps,text/plain" hidden onChange={e => e.target.files[0] && onLoadFile("test", e.target.files[0])}/>
        </label>
        <label className="load-btn">
          ↑ .g4 (referencia)
          <input type="file" accept=".g4,text/plain" hidden onChange={e => e.target.files[0] && onLoadFile("g4", e.target.files[0])}/>
        </label>
      </div>

      <div className="h">▍ WORKSPACE</div>
      {["yal","yalp","test","g4"].map(id => (
        <div key={id}
             className={"tree-row file " + D.FILES[id].kind + (active===id?" active":"")}
             onClick={() => onPick(id)}>
          <span className="icn"/>
          <span>{D.FILES[id].name}</span>
          {D.FILES[id].dirty && <span className="badge">●</span>}
        </div>
      ))}
      <div className="dim" style={{fontSize:14, padding:"6px 10px"}}>
        el .g4 es solo referencia — nunca se manda al backend, este generador no compila ANTLR
      </div>
    </div>
  );
}

/* ============================== Syntax highlight ============================== */

function escHtml(s){ return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;"); }

const HL_RULES = {
  yal: [
    { re: /\(\*[\s\S]*?\*\)/y, cls: "com" },
    { re: /\b(let|rule|return|skip)\b/y, cls: "kw" },
    { re: /\[[^\]]*\]/y, cls: "str" },
    { re: /'[^']*'/y, cls: "str" },
    { re: /[a-z_][a-zA-Z0-9_]*/y, cls: "fn" },
    { re: /[0-9]+/y, cls: "num" },
    { re: /[|*+?()\-={}]/y, cls: "op" },
  ],
  yalp: [
    { re: /\/\*[\s\S]*?\*\//y, cls: "com" },
    { re: /%%/y, cls: "kw" },
    { re: /%(token|start|left|right|nonassoc|ignore|ident|declare|scope)\b/y, cls: "kw" },
    { re: /[A-Z][A-Z0-9_]*/y, cls: "term" },
    { re: /[a-z_][a-z0-9_]*/y, cls: "nonterm" },
    { re: /[|:;]/y, cls: "op" },
  ],
  cps: [
    { re: /\/\/[^\n]*/y, cls: "com" },
    { re: /\/\*[\s\S]*?\*\//y, cls: "com" },
    { re: /"(\\.|[^"\\])*"/y, cls: "str" },
    { re: /\b(let|var|const|function|class|if|else|while|do|for|foreach|in|break|continue|return|try|catch|switch|case|default|print|new|this|null|true|false)\b/y, cls: "kw" },
    { re: /\b(boolean|integer|string)\b/y, cls: "term" },
    { re: /[A-Za-z_][A-Za-z0-9_]*/y, cls: "fn" },
    { re: /[0-9]+/y, cls: "num" },
    { re: /[{}()\[\];,.:=+\-*/%<>!&|?]+/y, cls: "op" },
  ],
  g4: [
    { re: /\/\/[^\n]*/y, cls: "com" },
    { re: /\/\*[\s\S]*?\*\//y, cls: "com" },
    { re: /'[^']*'/y, cls: "str" },
    { re: /\b(grammar|import|options|tokens|lexer|parser|fragment|returns|throws|catch|finally)\b/y, cls: "kw" },
    { re: /[A-Z][A-Za-z0-9_]*/y, cls: "term" },
    { re: /[a-z_][a-zA-Z0-9_]*/y, cls: "nonterm" },
    { re: /[|:;?*+]/y, cls: "op" },
  ],
  txt: [],
};

/* Extensión real del archivo cargado, no el slot fijo — así un .cps se
   resalta como Compiscript y un input.txt plano sigue sin resaltado. */
function langForFile(f){
  const name = (f && f.name) || "";
  if (name.endsWith(".yal") || name.endsWith(".yalex")) return "yal";
  if (name.endsWith(".yalp") || name.endsWith(".yapar")) return "yalp";
  if (name.endsWith(".cps")) return "cps";
  if (name.endsWith(".g4")) return "g4";
  return "txt";
}

function tokenize(text, lang){
  const rules = HL_RULES[lang] || [];
  if (!rules.length) return escHtml(text);
  let out = "";
  let i = 0;
  while (i < text.length) {
    let matched = false;
    for (const { re, cls } of rules) {
      re.lastIndex = i;
      const m = re.exec(text);
      if (m) {
        out += `<span class="${cls}">${escHtml(m[0])}</span>`;
        i += m[0].length;
        matched = true;
        break;
      }
    }
    if (!matched) { out += escHtml(text[i]); i++; }
  }
  return out;
}

/* ============================== Editor ============================== */

function Editor({ file, onEdit, contentVersion }){
  const f = D.FILES[file];
  const taRef  = useRef();
  const gutRef = useRef();
  const hlRef  = useRef();
  const lang = langForFile(f);
  const [lineCount,   setLineCount]   = useState(() => f.rawContent.split('\n').length);
  const [highlighted, setHighlighted] = useState(() => tokenize(f.rawContent, lang));

  useEffect(() => {
    if (taRef.current) taRef.current.value = f.rawContent;
    setLineCount(f.rawContent.split('\n').length);
    setHighlighted(tokenize(f.rawContent, lang));
  }, [file, contentVersion]);

  const lineFromLoc = (loc) => {
    if (!loc) return null;
    const parts = loc.split(":");
    if (parts.length < 2) return null;
    const n = parseInt(parts[1], 10);
    return Number.isFinite(n) ? n : null;
  };
  const collectLines = (level) => {
    const lines = new Set();
    for (const p of D.PROBLEMS) {
      if (p.level !== level) continue;
      let n = null;
      if (p.loc && p.loc.includes(f.name)) {
        n = lineFromLoc(p.loc);
      } else if (file === "test" && p.line != null) {
        n = p.line;
      }
      if (n != null && !Number.isNaN(n)) lines.add(n);
    }
    return lines;
  };
  const errs  = collectLines("err");
  const warns = collectLines("warn");

  const handleChange = e => {
    const content = e.target.value;
    D.FILES[file].rawContent = content;
    D.FILES[file].dirty = true;
    const newCount = content.split('\n').length;
    if (newCount !== lineCount) setLineCount(newCount);
    setHighlighted(tokenize(content, lang));
    onEdit(file);
  };

  const syncScroll = () => {
    if (gutRef.current && taRef.current)
      gutRef.current.scrollTop = taRef.current.scrollTop;
    if (hlRef.current && taRef.current){
      hlRef.current.scrollTop  = taRef.current.scrollTop;
      hlRef.current.scrollLeft = taRef.current.scrollLeft;
    }
  };

  return (
    <>
      <div className="bread">
        <span className="b">src</span><span className="sep">›</span>
        <span className="b">{f.name}</span>
        <div className="right">
          <span className="pill">{lang==="yal"?"YALex":lang==="yalp"?"YACC":lang==="cps"?"Compiscript":lang==="g4"?"ANTLR (ref.)":"text"}</span>
          {f.dirty && <span style={{color:"var(--yellow)"}}>● modificado</span>}
          <span>UTF-8</span>
        </div>
      </div>
      <div id="editor" className="panel cyan">
        <div className="gutter" ref={gutRef} style={{overflowY:"hidden"}}>
          {Array.from({length: lineCount}, (_, i) => {
            const n = i + 1;
            const c = ["ln", errs.has(n)?"err":"", warns.has(n)?"warn":""].join(" ");
            return <div key={i} className={c}>{String(n).padStart(2,"0")}</div>;
          })}
        </div>
        <div className="editor-body">
          <div
            ref={hlRef}
            className="highlight-layer"
            aria-hidden="true"
            dangerouslySetInnerHTML={{__html: highlighted}}
          />
          <textarea
            ref={taRef}
            className="code-edit"
            defaultValue={f.rawContent}
            onChange={handleChange}
            onScroll={syncScroll}
            spellCheck={false}
            autoComplete="off"
            autoCorrect="off"
          />
        </div>
      </div>
    </>
  );
}

/* ============================== Result panes ============================== */

function TokensView(){
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ TOKEN STREAM · {D.TOKENS.length} tokens</div>
      {!D.TOKENS.length && <div className="dim" style={{padding:16}}>ejecuta ▶ ANALIZAR primero</div>}
      <div className="tok" style={{borderBottom:"2px solid var(--magenta-d)"}}>
        <div className="mute">#</div>
        <div className="mute">KIND · LEXEMA</div>
        <div className="mute">L</div>
        <div className="mute">C</div>
        <div className="mute">OK</div>
      </div>
      {D.TOKENS.map(t=>
        <div className="tok" key={t.i}>
          <div className="i">{t.i}</div>
          <div><span className="k">{t.k}</span><span className="lx" style={{marginLeft:10}}>'{t.lx}'</span></div>
          <div className="lc">{t.l}</div>
          <div className="lc">{t.c}</div>
          <div style={{color:"var(--green)"}}>✓</div>
        </div>
      )}
    </div>
  );
}

/* El árbol viene del backend YA renderizado a DOT (D.PARSE_TREE_DOT, campo
   `parse_tree_dot` de /api/pipeline) — construido a partir del ParseNode
   real, y auto-anotado con el tipo de cada expresión en cuanto corrió el
   análisis semántico (ver src/api/pipeline.rs: el DOT se genera DESPUÉS del
   análisis para poder pasarle las anotaciones). Sin %ident en el .yalp sale
   el árbol plano de siempre. */
function ParseTreeView({ renderKey }) {
  const containerRef = useRef();

  useEffect(() => {
    const dot = D.PARSE_TREE_DOT;
    if (!dot) {
      if (containerRef.current)
        containerRef.current.innerHTML = '<div class="dim" style="padding:12px;font-size:17px">Presiona ▶ ANALIZAR para ver el árbol</div>';
      return;
    }
    if (!window.Viz) {
      containerRef.current.innerHTML = '<div style="color:var(--yellow);padding:12px">Cargando viz.js…</div>';
      return;
    }
    window.Viz.instance().then(viz => {
      if (!containerRef.current) return;
      try {
        const svg = viz.renderSVGElement(dot);
        svg.style.maxWidth = "none";
        svg.style.height   = "auto";
        containerRef.current.innerHTML = "";
        containerRef.current.appendChild(svg);
      } catch(err) {
        containerRef.current.innerHTML = `<div style="color:var(--red);padding:12px">Error al renderizar: ${err}</div>`;
      }
    }).catch(err => {
      if (containerRef.current)
        containerRef.current.innerHTML = `<div style="color:var(--red);padding:12px">${err}</div>`;
    });
  }, [renderKey]);

  const downloadPng = () => {
    const svg = containerRef.current?.querySelector("svg");
    if (!svg) return;
    const data = new XMLSerializer().serializeToString(svg);
    const canvas = document.createElement("canvas");
    const bb = svg.getBoundingClientRect();
    canvas.width = bb.width || 800; canvas.height = bb.height || 600;
    const img = new Image();
    img.onload = () => {
      canvas.getContext("2d").drawImage(img, 0, 0);
      const a = document.createElement("a");
      a.href = canvas.toDataURL("image/png");
      a.download = "arbol.png";
      a.click();
    };
    img.src = "data:image/svg+xml;base64," + btoa(unescape(encodeURIComponent(data)));
  };

  return (
    <div className="dfa-wrap">
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8, display:"flex", alignItems:"center", gap:10}}>
        ▍ ÁRBOL SINTÁCTICO
        <span className="dim" style={{fontFamily:"VT323", fontSize:16}}>· con %ident sale anotado con el tipo de cada expresión</span>
        {D.PARSE_TREE_DOT && (
          <button className="cbtn icon cyan" style={{fontSize:12, padding:"2px 8px"}} onClick={downloadPng}>↓ PNG</button>
        )}
      </div>
      <div ref={containerRef} className="dfa-container"/>
    </div>
  );
}

/* Un color por tipo de símbolo/ámbito, compartido entre las dos mitades del
   panel (el árbol de ESTADO FINAL y las tarjetas POR ENTORNO) para que se
   lean como la MISMA información vista dos veces, no como dos formatos
   distintos — eso era buena parte de lo confuso del diseño anterior. */
const SYMBOL_KIND_COLOR = {
  Class:     "var(--pink)",
  Struct:    "var(--blue)",
  Function:  "var(--cyan)",
  Parameter: "var(--green)",
  Variable:  "var(--yellow)",
};
const SCOPE_KIND_COLOR = {
  Global:   "var(--magenta)",
  Function: "var(--cyan)",
  Class:    "var(--pink)",
  Struct:   "var(--blue)",
  Block:    "var(--tx-dim)",
};
const kindColor = (kind, table) => table[kind] || "var(--tx)";

/* Convierte el texto plano de SymbolTable::dump() (indentado con espacios,
   ver src/semantico/symbols/mod.rs:dump) en un árbol de nodos, comparando
   sangría en vez de contar espacios a mano — así no importa si una línea es
   un encabezado de ámbito "[0] Global" o un símbolo "x: Variable, Int @1:1",
   ni cuántos niveles de miembros anidados (campos de una clase) trae. */
function parseSymbolDump(text){
  const root = { children: [] };
  const stack = [{ indent: -1, node: root }];
  for (const raw of text.split("\n")) {
    if (!raw.trim()) continue;
    const indent = raw.length - raw.trimStart().length;
    const content = raw.trim();
    const scopeMatch = content.match(/^\[(\d+)\]\s+(\w+)(?:\((.*)\))?$/);
    const symMatch = content.match(/^([A-Za-z_][A-Za-z0-9_]*):\s+(.+)\s+@(\d+):(\d+)$/);
    let node;
    if (scopeMatch) {
      node = { type:"scope", order:scopeMatch[1], kind:scopeMatch[2], label:scopeMatch[3]||null, children:[] };
    } else if (symMatch) {
      const parts = symMatch[2].split(", ");
      node = { type:"symbol", name:symMatch[1], kind:parts[0], ty:null, isConst:false, used:false,
                line:symMatch[3], col:symMatch[4], children:[] };
      for (const p of parts.slice(1)) {
        if (p === "const") node.isConst = true;
        else if (p === "usado") node.used = true;
        else node.ty = p;
      }
    } else {
      node = { type:"text", text:content, children:[] };
    }
    while (stack.length > 1 && indent <= stack[stack.length-1].indent) stack.pop();
    stack[stack.length-1].node.children.push(node);
    stack.push({ indent, node });
  }
  return root.children;
}

/* Aplana el árbol de parseSymbolDump() a filas de tabla: una por símbolo, con
   su "contenedor" (la ruta de ámbitos/símbolos que lo encierran, p. ej.
   "Global › Animal › constructor") en vez de sangría — más legible en una
   tabla que contar espacios en una celda. */
function flattenSymbolDump(nodes, path){
  let rows = [];
  for (const node of nodes) {
    if (node.type === "scope") {
      const label = node.kind + (node.label ? `(${node.label})` : "");
      rows = rows.concat(flattenSymbolDump(node.children, [...path, label]));
    } else if (node.type === "symbol") {
      rows.push({ ...node, container: path.join(" › ") || "—" });
      rows = rows.concat(flattenSymbolDump(node.children, [...path, node.name]));
    }
  }
  return rows;
}

function SymbolTableRows({ rows }){
  return rows.map((r, i) => (
    <tr key={i}>
      <td style={{color:"var(--yellow)", textAlign:"left"}}>{r.name}</td>
      <td style={{color: kindColor(r.kind, SYMBOL_KIND_COLOR)}}>{r.kind}</td>
      <td style={{color:"var(--cyan)"}}>{r.ty || "—"}</td>
      <td className="dim">{[r.isConst && "const", r.used && "usado"].filter(Boolean).join(", ") || "—"}</td>
      <td className="dim">{r.line}:{r.col}</td>
      <td className="dim" style={{textAlign:"left"}}>{r.container}</td>
    </tr>
  ));
}

const SYMBOL_TABLE_HEADERS = ["NOMBRE","TIPO","DATO","MOD.","POS.","CONTENEDOR"];

/* Panel de la tabla de símbolos, en dos tablas que NO son redundantes:

   1. ESTADO FINAL — SymbolTable::dump(): lo que sobrevive al terminar el
      recorrido, o sea el Global con los miembros de funciones y clases
      colgando anidados. Se parsea con parseSymbolDump()/flattenSymbolDump()
      y se dibuja como tabla, una fila por símbolo, con su ruta de
      contenedores en vez de sangría.
   2. POR ENTORNO — D.SCOPES (campo `scopes` de /api/pipeline,
      ScopeCollector::to_json()): una foto de CADA entorno en el momento en
      que se cerró, en orden de cierre. Es lo único que deja ver lo declarado
      en un ámbito ANÓNIMO — un `let` dentro de un `if` vive en un Block que
      se desapila y no aparece en la tabla de arriba por ningún lado.

   Vacío si el .yalp activo no trae la directiva %ident (sin análisis
   semántico para él). */
function SymbolTableView(){
  const dump = D.SYMBOL_TABLE;
  const scopes = D.SCOPES || [];
  if (!dump) {
    return (
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ TABLA DE SÍMBOLOS
        <span className="dim" style={{marginLeft:10}}>
          · ejecuta ANALIZAR sobre una gramática con %ident
        </span>
      </div>
    );
  }
  const finalRows = flattenSymbolDump(parseSymbolDump(dump), []);
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ TABLA DE SÍMBOLOS · ESTADO FINAL · {finalRows.length}
        <span className="dim" style={{marginLeft:10}}>
          · lo que queda vivo al terminar: el ámbito global y los miembros de
          funciones y clases
        </span>
      </div>
      <table className="t">
        <thead><tr>{SYMBOL_TABLE_HEADERS.map(h => <th key={h}>{h}</th>)}</tr></thead>
        <tbody><SymbolTableRows rows={finalRows}/></tbody>
      </table>

      <div className="h-pixel" style={{color:"var(--pink)", margin:"18px 0 8px"}}>
        ▍ TABLA DE SÍMBOLOS · POR ENTORNO (función / clase / bloque) · {scopes.length}
        <span className="dim" style={{marginLeft:10}}>
          {scopes.length
            ? "· cada entorno tal como estaba al cerrarse, en orden de cierre — incluye los bloques anónimos que no sobreviven arriba"
            : "· este programa no abrió ningún ámbito propio"}
        </span>
      </div>
      <table className="t">
        <thead>
          <tr><th>#</th><th>ENTORNO</th><th>APERTURA</th><th>PROF.</th><th>NOMBRE</th><th>TIPO</th><th>DATO</th><th>MOD.</th><th>POS.</th></tr>
        </thead>
        <tbody>
          {scopes.flatMap((sc, i) => {
            const entorno = `${sc.kind}${sc.label ? `(${sc.label})` : ""}`;
            const entornoColor = kindColor(sc.kind, SCOPE_KIND_COLOR);
            const apertura = (sc.line || sc.col) ? `${sc.line}:${sc.col}` : "—";
            if (!sc.symbols || !sc.symbols.length) {
              return (
                <tr key={i}>
                  <td className="dim">{sc.order}</td>
                  <td style={{color: entornoColor}}>{entorno}</td>
                  <td className="dim">{apertura}</td>
                  <td className="dim">{sc.depth}</td>
                  <td className="dim" colSpan={5}>(sin declaraciones propias)</td>
                </tr>
              );
            }
            return sc.symbols.map((sym, j) => (
              <tr key={`${i}-${j}`}>
                {j === 0 && (
                  <>
                    <td className="dim" rowSpan={sc.symbols.length}>{sc.order}</td>
                    <td style={{color: entornoColor}} rowSpan={sc.symbols.length}>{entorno}</td>
                    <td className="dim" rowSpan={sc.symbols.length}>{apertura}</td>
                    <td className="dim" rowSpan={sc.symbols.length}>{sc.depth}</td>
                  </>
                )}
                <td style={{color:"var(--yellow)", textAlign:"left"}}>{sym.name}</td>
                <td style={{color: kindColor(sym.kind, SYMBOL_KIND_COLOR)}}>{sym.kind}</td>
                <td style={{color:"var(--cyan)"}}>{sym.ty || "—"}</td>
                <td className="dim">{sym.mutable === false ? "const" : "—"}</td>
                <td className="dim">{sym.line}:{sym.col}</td>
              </tr>
            ));
          })}
        </tbody>
      </table>
    </div>
  );
}

// El tipo que el analizador infirió para cada nodo de expresión — el "árbol
// de análisis anotado" del libro del dragón, en forma de tabla. El mismo
// dato va dibujado sobre el árbol en la pestaña ÁRBOL SINTÁCTICO; acá se
// lista en orden de lectura para poder buscar un nodo puntual por su id.
function TypesView(){
  const types = D.TYPES;
  if (!types || !types.length) {
    return (
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ TIPOS
        <span className="dim" style={{marginLeft:10}}>
          · sin anotaciones · requiere .yalp con %ident y modo LALR/SLR (no LL(1))
        </span>
      </div>
    );
  }
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ TIPOS · {types.length} nodo{types.length===1?"":"s"} de expresión anotado{types.length===1?"":"s"}
        <span className="dim" style={{marginLeft:10}}>
          · el id coincide con el nodo del árbol
        </span>
      </div>
      <table className="t">
        <thead>
          <tr><th>id</th><th>nodo</th><th>lexema</th><th>tipo</th><th>pos</th></tr>
        </thead>
        <tbody>
          {types.map((t,i)=>(
            <tr key={i}>
              <td className="dim">{t.id}</td>
              <td style={{color:"var(--cyan)"}}>{t.symbol}</td>
              <td style={{color:"var(--yellow)"}}>{t.lexeme==null?"":t.lexeme}</td>
              <td style={{color:"var(--green)", fontWeight:600}}>{t.ty}</td>
              <td className="dim">{t.line}:{t.col}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/* D.PROBLEMS filtrado a solo diagnósticos semánticos (código S0xx — ver
   semantico::errors::ErrorCollector). Los sintácticos/léxicos (que también
   viajan en D.PROBLEMS) no se muestran acá: no están en el alcance de este
   panel. */
function SemanticErrorsView(){
  const semantic = D.PROBLEMS.filter(p => (p.code||"").startsWith("S"));
  const counts = { err:semantic.filter(p=>p.level==="err").length,
                   warn:semantic.filter(p=>p.level==="warn").length,
                   info:semantic.filter(p=>p.level==="info").length };
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ ERRORES SEMÁNTICOS ·
        <span className="err"> {counts.err} err</span> ·
        <span className="warn"> {counts.warn} warn</span> ·
        <span className="info"> {counts.info} info</span>
      </div>
      {!semantic.length && <div className="dim" style={{padding:16}}>sin errores semánticos</div>}
      {semantic.map((p,i)=>{
        const hasPos = p.line != null && p.col != null;
        const locLabel = hasPos
          ? `línea ${p.line}, col ${p.col}`
          : (p.loc || "");
        return (
          <div key={i} className={"prob "+p.level}>
            <div className="tag">{p.level==="err"?"ERR":p.level==="warn"?"WRN":"INF"}</div>
            <div style={{flex:1}}>
              <div className="msg">{p.msg}</div>
              <div className="loc" style={{display:"flex", gap:8, flexWrap:"wrap", alignItems:"center"}}>
                {p.code && <span>{p.code}</span>}
                {hasPos && (
                  <span className="prob-pos">
                    <span style={{color:"var(--cyan)"}}>↗</span>
                    {" "}{locLabel}
                  </span>
                )}
                {!hasPos && locLabel && <span>{locLabel}</span>}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ============================== Right results panel ============================== */

function ResultsPanel({ activeTab, setActiveTab, renderKey, onToggleSize, resultsSize }){
  const semCount = D.PROBLEMS.filter(p => (p.code||"").startsWith("S")).length;
  const TABS = [
    {id:"tokens",   label:"TOKENS", badge: D.TOKENS.length || null},
    {id:"tree",     label:"ÁRBOL SINTÁCTICO"},
    {id:"symbols",  label:"SÍMBOLOS", badge: (D.SCOPES && D.SCOPES.length) || null},
    {id:"types",    label:"TIPOS", badge: (D.TYPES && D.TYPES.length) || null},
    {id:"errors",   label:"ERRORES SEM.", badge: semCount || null},
  ];

  return (
    <>
      <div className="panel-title">
        <span className="swatch"/>RESULTS
        <button className="results-toggle" onClick={onToggleSize} title="Cambiar tamaño del panel">
          {resultsSize==="normal"?"⟩⟩ AMPLIAR": resultsSize==="wide"?"◁ OCULTAR": "⟨⟨ NORMAL"}
        </button>
      </div>
      <div className="panel">
        <div className="rtabs">
          {TABS.map(t=>
            <div key={t.id}
                 className={"rtab " + (activeTab===t.id?"active":"")}
                 onClick={()=>setActiveTab(t.id)}>
              {t.label}{t.badge!=null && <span className="count">·{t.badge}</span>}
            </div>
          )}
        </div>
        <div className="rbody">
          {activeTab==="tokens"  && <TokensView/>}
          {activeTab==="tree"    && <ParseTreeView renderKey={renderKey}/>}
          {activeTab==="symbols" && <SymbolTableView/>}
          {activeTab==="types"   && <TypesView/>}
          {activeTab==="errors"  && <SemanticErrorsView/>}
        </div>
      </div>
    </>
  );
}

/* ============================== Header ============================== */

const MODE_LABELS = { lalr:"LALR(1)", slr:"SLR(1)", ll1:"LL(1)" };

function Header({ activeFile, setFile, onRun, onSave, loading, mode, setMode }){
  const tabs = ["yal","yalp","test","g4"];
  return (
    <header data-screen-label="IDE">

      <div className="filetabs">
        {tabs.map(id=>{
          const f = D.FILES[id];
          return (
            <div key={id}
                 className={"ftab " + (activeFile===id?"active":"") + (f.dirty?" dirty":"")}
                 onClick={()=>setFile(id)}>
              <span className="dot"/>
              <span>{f.name}</span>
            </div>
          );
        })}
      </div>
      <div className="actions">
        <div className="modegrp">
          {Object.entries(MODE_LABELS).map(([key, label])=>
            <button key={key}
                    className={"modebtn " + (mode===key?"active":"")}
                    onClick={()=>setMode(key)}>
              {label}
            </button>
          )}
        </div>
        <button className="runbtn" onClick={onRun} disabled={loading} style={{opacity:loading?.5:1}}>
          {loading ? "..." : <><span className="play"/>ANALIZAR</>}
        </button>
        <button className="runbtn stepbtn" onClick={onSave} title="Guardar archivo activo">
          SAVE
        </button>
        <div className="winbtns" style={{marginLeft:14}}>
          <div className="wb wb-min"/>
          <div className="wb wb-max"/>
          <div className="wb wb-close"/>
        </div>
      </div>
    </header>
  );
}

/* ============================== Status bar ============================== */

function StatusBar({ activeFile, mode }){
  const f = D.FILES[activeFile];
  const hasResult = D.PARSE_ACCEPTED !== null && D.PARSE_ACCEPTED !== undefined;
  return (
    <div id="status">
      <div className="sg"><span className="grm">{MODE_LABELS[mode]}</span></div>
      {hasResult && (
        <div className="sg" style={{color: D.PARSE_ACCEPTED ? "var(--green)" : "var(--red)"}}>
          {D.PARSE_ACCEPTED ? "✓ ACEPTADA" : `✗ ${D.PARSE_ERROR || "RECHAZADA"}`}
        </div>
      )}
      <div className="right">
        <div className="sg">{f.name}</div>
        <div className="sg">UTF-8</div>
      </div>
    </div>
  );
}

/* ============================== App ============================== */

function App(){
  const [activeFile,     setFile]          = useState("test");
  const [activeTab,      setTab]           = useState("tree");
  const [loading,        setLoading]       = useState(false);
  const [mode,           setMode]          = useState("lalr");
  const [renderKey,      bump]             = useState(0);
  const [contentVersion, setContentVersion] = useState(0);
  const [resultsSize,    setResultsSize]   = useState("normal"); // "normal"|"wide"|"custom"
  const [layout,         setLayout]        = useState({ left: 240, right: 460 });
  const [drag,           setDrag]          = useState(null);
  const appRef = useRef(null);

  const rerender = () => bump(n => n + 1);

  // ── WORKSPACE: carga yal/yalp/test del servidor al arrancar (el .g4 de
  // referencia nunca vive ahí — no hay slot para él en el backend). ──────────
  const PREFERRED_WORKSPACE_NAME = { yal: "compiscript.yal", yalp: "compiscript.yalp", test: "rubrica.cps" };
  const fetchWorkspace = async () => {
    try {
      const res = await fetch(`${API}/api/workspace`);
      if (!res.ok) return;
      const { files } = await res.json();
      const pickByKind = { yal: null, yalp: null, test: null };
      for (const { name, kind } of files) {
        const slot = kind === "yal" ? "yal" : kind === "yalp" ? "yalp" : "test";
        if (!pickByKind[slot] || name === PREFERRED_WORKSPACE_NAME[slot]) {
          pickByKind[slot] = name;
        }
      }
      for (const slot of Object.keys(pickByKind)) {
        const name = pickByKind[slot];
        if (!name) continue;
        const content = await fetch(`${API}/api/workspace/${encodeURIComponent(name)}`).then(r => r.text());
        D.FILES[slot].rawContent = content;
        D.FILES[slot].name  = name;
        D.FILES[slot].dirty = false;
      }
      setContentVersion(v => v + 1);
      rerender();
    } catch(e) { console.warn("workspace not available, using defaults", e); }
  };

  useEffect(() => { fetchWorkspace(); }, []);

  const handleEdit = () => { rerender(); };

  // ── CARGAR ARCHIVO: lee un File y, salvo el .g4 (referencia, nunca se
  // persiste — sanitize_filename del backend no lo acepta), lo sube también
  // al workspace. ──────────────────────────────────────────────────────────
  const handleLoadFile = (fileId, file) => {
    const reader = new FileReader();
    reader.onload = async e => {
      const content = e.target.result;
      D.FILES[fileId].rawContent = content;
      D.FILES[fileId].name  = file.name;
      D.FILES[fileId].dirty = false;
      if (fileId !== "g4") {
        try {
          await fetch(`${API}/api/workspace/${encodeURIComponent(file.name)}`, {
            method: "PUT",
            headers: { "Content-Type": "text/plain" },
            body: content,
          });
        } catch(e) { /* funciona localmente aunque el backend falle */ }
      }
      setFile(fileId);
      setContentVersion(v => v + 1);
      rerender();
    };
    reader.readAsText(file);
  };

  const handleSave = async () => {
    const f = D.FILES[activeFile];
    if (activeFile === "g4") return; // solo referencia, no hay dónde guardarlo
    try {
      const res = await fetch(`${API}/api/workspace/${encodeURIComponent(f.name)}`, {
        method: "PUT",
        headers: { "Content-Type": "text/plain" },
        body: f.rawContent,
      });
      if (res.ok) { D.FILES[activeFile].dirty = false; rerender(); }
    } catch(e) {
      const blob = new Blob([f.rawContent], { type: "text/plain" });
      const url  = URL.createObjectURL(blob);
      const a    = document.createElement("a");
      a.href = url; a.download = f.name;
      document.body.appendChild(a); a.click();
      document.body.removeChild(a); URL.revokeObjectURL(url);
      D.FILES[activeFile].dirty = false;
      rerender();
    }
  };

  // ── ANALIZAR: pipeline completo (.yal + .yalp + fuente) en un solo paso —
  // sin stepper, así que no hace falta separar compilar-gramática de
  // parsear-una-cadena como en el IDE completo. ───────────────────────────
  const handleRun = async () => {
    if (!D.FILES.yal.rawContent.trim() || !D.FILES.yalp.rawContent.trim() || !D.FILES.test.rawContent.trim()) {
      D.PROBLEMS = [{ level:"err", code:"E000", msg:"Cargá .yal, .yalp y el fuente de prueba antes de analizar.", loc:"" }];
      setTab("errors");
      rerender();
      return;
    }
    setLoading(true);
    try {
      const res = await fetch(`${API}/api/pipeline`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({
          yal_content:  D.FILES.yal.rawContent,
          yalp_content: D.FILES.yalp.rawContent,
          source:       D.FILES.test.rawContent,
          source_name:  D.FILES.test.name,
          mode,
        }),
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();

      D.TOKENS = (data.token_map || []).map((t, i) => ({ i: i+1, k: t.kind, lx: t.lexeme, l: t.line, c: t.col }));
      D.PARSE_TREE_DOT = data.parse_tree_dot || "";
      D.SYMBOL_TABLE    = data.symbol_table || "";
      D.SCOPES          = Array.isArray(data.scopes) ? data.scopes : [];
      D.TYPES           = Array.isArray(data.types) ? data.types : [];
      D.PARSE_ACCEPTED = !!data.accepted;
      D.PARSE_ERROR = data.error || null;
      D.PROBLEMS = data.problems && data.problems.length ? data.problems : [];

      bump(n => n + 1);
      const hasErrors = !data.accepted || (data.problems && data.problems.some(p => p.level === "err"));
      setTab(hasErrors ? "errors" : "tree");
      rerender();
    } catch(e) {
      console.error("API /pipeline:", e);
      let msg = String(e);
      try { const j = JSON.parse(msg.replace(/^Error:\s*/,"")); if (j.error) msg = j.error; } catch(_){}
      D.TOKENS = [];
      D.PARSE_TREE_DOT = "";
      D.SYMBOL_TABLE = "";
      D.SCOPES = [];
      D.TYPES = [];
      D.PARSE_ACCEPTED = null;
      D.PARSE_ERROR = msg;
      D.PROBLEMS = [{ level:"err", code:"E001", msg, loc:`pipeline ${mode.toUpperCase()}` }];
      setTab("errors");
      rerender();
    } finally {
      setLoading(false);
    }
  };

  // ── Auto-reanalizar cuando el modo cambia, si ya se corrió antes ──────────
  const prevModeRef = useRef(mode);
  useEffect(() => {
    if (prevModeRef.current === mode) return;
    prevModeRef.current = mode;
    if (D.PARSE_ACCEPTED !== null) handleRun();
  }, [mode]);

  const RESULTS_WIDTHS = { normal: 460, wide: 760 };
  const cycleResults = () => setResultsSize(s => s==="normal"?"wide":"normal");

  useEffect(() => {
    if (resultsSize === "custom") return;
    setLayout(l => ({ ...l, right: RESULTS_WIDTHS[resultsSize] }));
  }, [resultsSize]);

  useEffect(() => {
    if (!drag) return;
    const handleMove = (e) => {
      const el = appRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const clamp = (val, min, max) => Math.min(max, Math.max(min, val));
      const minLeft = 160;
      const maxLeft = Math.max(minLeft, rect.width - 360);
      const minRight = 300;
      const maxRight = Math.max(minRight, rect.width - 320);

      if (drag.kind === "left") {
        const next = clamp(e.clientX - rect.left, minLeft, maxLeft);
        setLayout(l => ({ ...l, left: next }));
      }
      if (drag.kind === "right") {
        const next = clamp(rect.right - e.clientX, minRight, maxRight);
        setResultsSize("custom");
        setLayout(l => ({ ...l, right: next }));
      }
    };
    const handleUp = () => setDrag(null);
    window.addEventListener("mousemove", handleMove);
    window.addEventListener("mouseup", handleUp);
    return () => {
      window.removeEventListener("mousemove", handleMove);
      window.removeEventListener("mouseup", handleUp);
    };
  }, [drag]);

  return (
    <div
      id="app"
      ref={appRef}
      style={{
        "--col-left": `${layout.left}px`,
        "--col-right": `${layout.right}px`,
      }}
    >
      <Header activeFile={activeFile} setFile={setFile} onRun={handleRun} onSave={handleSave}
              loading={loading} mode={mode} setMode={setMode}/>

      <div id="files" className="panel" data-screen-label="files">
        <div className="panel-title">
          <span className="swatch"/>EXPLORER
        </div>
        <FileTree active={activeFile} onPick={setFile} onLoadFile={handleLoadFile}/>
      </div>

      <div className="grid-handle v left" onMouseDown={() => setDrag({ kind: "left" })} />

      <div id="editor-wrap" data-screen-label="editor">
        <Editor file={activeFile} onEdit={handleEdit} contentVersion={contentVersion}/>
      </div>

      <div className="grid-handle v right" onMouseDown={() => setDrag({ kind: "right" })} />

      <div id="results" data-screen-label="results">
        <ResultsPanel
          activeTab={activeTab}
          setActiveTab={setTab}
          renderKey={renderKey}
          onToggleSize={cycleResults}
          resultsSize={resultsSize}/>
      </div>

      <StatusBar activeFile={activeFile} mode={mode}/>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
