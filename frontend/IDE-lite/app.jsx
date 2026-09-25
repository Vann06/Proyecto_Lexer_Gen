/* eslint-disable */
// IDE-lite del DBMS: editor SQL con resaltado + sidebar de archivos + las
// vistas del análisis: flujo de tokens, árbol de derivación de ANTLR y
// problemas léxicos/sintácticos. Todo sale de `POST /api/sql/parse`.
// (Fase 9: pestañas RESULTADOS y CATÁLOGO cuando exista /api/sql/run.)
const { useState, useEffect, useRef } = React;
const D = window.IDE_DATA;

const API = "http://localhost:8080";

/* ============================== Helpers ============================== */

function FileTree({ active, onPick, onLoadFile, workspace, onOpenWorkspace }){
  return (
    <div className="filetree">
      <div className="h">▍ CARGAR ARCHIVO</div>
      <div className="load-btns">
        <label className="load-btn">
          ↑ script .sql
          <input type="file" accept=".sql,.txt,text/plain" hidden onChange={e => e.target.files[0] && onLoadFile(e.target.files[0])}/>
        </label>
      </div>

      <div className="h">▍ ABIERTOS</div>
      {["sql","g4"].map(id => (
        <div key={id}
             className={"tree-row file " + D.FILES[id].kind + (active===id?" active":"")}
             onClick={() => onPick(id)}>
          <span className="icn"/>
          <span>{D.FILES[id].name}</span>
          {D.FILES[id].dirty && <span className="badge">●</span>}
        </div>
      ))}
      <div className="dim" style={{fontSize:14, padding:"6px 10px"}}>
        SQL.g4 es la gramática ANTLR real del backend (solo lectura)
      </div>

      <div className="h">▍ WORKSPACE</div>
      {!workspace.length && <div className="dim" style={{fontSize:14, padding:"6px 10px"}}>sin archivos (¿API apagada?)</div>}
      {workspace.map(({ name, kind }) => (
        <div key={name}
             className={"tree-row file " + kind + (D.FILES.sql.name===name?" active":"")}
             onClick={() => onOpenWorkspace(name)}>
          <span className="icn"/>
          <span>{name}</span>
        </div>
      ))}
    </div>
  );
}

/* ============================== Syntax highlight ============================== */

function escHtml(s){ return s.replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;"); }

const SQL_KEYWORDS = "add|alter|and|as|asc|between|by|check|column|columns|constraint|create|cross|data|database|databases|default|delete|desc|describe|distinct|drop|exists|false|foreign|from|group|having|if|in|index|inner|insert|into|is|join|key|left|like|limit|not|null|offset|on|or|order|outer|primary|references|rename|right|select|set|show|table|tables|to|true|type|unique|update|use|values|where";
const SQL_TYPES = "int|integer|float|real|double|char|varchar|date|boolean|bool";

const HL_RULES = {
  sql: [
    { re: /--[^\n]*/y, cls: "com" },
    { re: /\/\*[\s\S]*?\*\//y, cls: "com" },
    { re: /'(''|[^'])*'?/y, cls: "str" },
    { re: /"(""|[^"])*"|`[^`]*`/y, cls: "fn" },
    { re: new RegExp(`\\b(${SQL_TYPES})\\b`, "iy"), cls: "term" },
    { re: new RegExp(`\\b(${SQL_KEYWORDS})\\b`, "iy"), cls: "kw" },
    { re: /[A-Za-z_][A-Za-z0-9_]*(?=\s*\()/y, cls: "nonterm" },
    { re: /[A-Za-z_][A-Za-z0-9_]*/y, cls: "fn" },
    { re: /[0-9]+(\.[0-9]*)?([eE][+-]?[0-9]+)?/y, cls: "num" },
    { re: /[(),;.*=<>!+\-\/%|]+/y, cls: "op" },
  ],
  g4: [
    { re: /\/\/[^\n]*/y, cls: "com" },
    { re: /\/\*[\s\S]*?\*\//y, cls: "com" },
    { re: /'(\\.|[^'\\])*'/y, cls: "str" },
    { re: /\b(grammar|import|options|tokens|lexer|parser|fragment|returns|throws|catch|finally|skip)\b/y, cls: "kw" },
    { re: /#\s*[a-zA-Z_][a-zA-Z0-9_]*/y, cls: "num" },
    { re: /[A-Z][A-Za-z0-9_]*/y, cls: "term" },
    { re: /[a-z_][a-zA-Z0-9_]*/y, cls: "nonterm" },
    { re: /->|[|:;?*+~]/y, cls: "op" },
  ],
  txt: [],
};

/* Por extensión del archivo abierto, no por slot. */
function langForFile(f){
  const name = (f && f.name) || "";
  if (name.endsWith(".sql")) return "sql";
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

/* Todo el editor trabaja con saltos `\n`: el textarea ya normaliza así su
   `value`, pero un archivo de Windows llega con `\r\n`, y un `\r` suelto en
   la capa de resaltado o en el conteo de líneas la desalinea del textarea. */
function normalizeEol(text){ return text.replace(/\r\n?/g, "\n"); }

/* Resaltado listo para la capa de fondo. Un bloque `white-space:pre` no
   dibuja la última línea vacía si el texto termina en salto de línea, y el
   textarea sí: sin el `\n` extra, el cursor en esa última línea quedaría
   por debajo del texto resaltado. */
function highlightFor(text, lang){
  const html = tokenize(text, lang);
  return text.endsWith("\n") ? html + "\n" : html;
}

/* Métrica de línea del editor, la MISMA que usa el CSS (--code-pad,
   --code-lh en index.html). Se lee del CSS para no duplicar los números. */
function codeMetrics(){
  const css = getComputedStyle(document.documentElement);
  const px = (name, fallback) => parseFloat(css.getPropertyValue(name)) || fallback;
  return { pad: px("--code-pad", 12), lh: px("--code-lh", 25) };
}

/* Número de línea de un problema, si apunta a `fileName` (null = no se
   filtra por archivo). Los diagnósticos del backend traen `line`; los del
   IDE solo `loc` ("archivo:línea:col"). */
function problemLine(p, fileName){
  if (p.loc && fileName && !p.loc.startsWith(fileName + ":")) return null;
  if (p.line != null && Number.isFinite(p.line) && p.line > 0) return p.line;
  const parts = (p.loc || "").split(":");
  const n = parseInt(parts[1], 10);
  return Number.isFinite(n) && n > 0 ? n : null;
}

/* ============================== Editor ============================== */

function Editor({ file, onEdit, contentVersion, jump }){
  const f = D.FILES[file];
  const taRef  = useRef();
  const gutRef = useRef();
  const hlRef  = useRef();
  const lang = langForFile(f);
  const [lineCount,   setLineCount]   = useState(() => f.rawContent.split('\n').length);
  const [highlighted, setHighlighted] = useState(() => highlightFor(f.rawContent, lang));

  useEffect(() => {
    if (taRef.current) taRef.current.value = f.rawContent;
    setLineCount(f.rawContent.split('\n').length);
    setHighlighted(highlightFor(f.rawContent, lang));
  }, [file, contentVersion]);

  // Líneas con problemas de ESTE archivo. Un problema sin `loc` se asume
  // del script SQL.
  const collectLines = (level) => {
    const lines = new Set();
    for (const p of D.PROBLEMS) {
      if (p.level !== level) continue;
      const n = p.loc ? problemLine(p, f.name) : (file === "sql" ? problemLine(p, null) : null);
      if (n != null && n <= lineCount) lines.add(n);
    }
    return lines;
  };
  const errs  = collectLines("err");
  const warns = collectLines("warn");

  // Franjas detrás de cada línea con problema (un error gana a un warning).
  const { pad, lh } = codeMetrics();
  const marks = [...new Set([...errs, ...warns])]
    .map(n => `<div class="line-mark ${errs.has(n) ? "err" : "warn"}" style="top:${pad + (n - 1) * lh}px"></div>`)
    .join("");

  const handleChange = e => {
    const content = normalizeEol(e.target.value);
    D.FILES[file].rawContent = content;
    D.FILES[file].dirty = true;
    const newCount = content.split('\n').length;
    if (newCount !== lineCount) setLineCount(newCount);
    setHighlighted(highlightFor(content, lang));
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

  // Ir a una línea pedida desde el panel de problemas: seleccionarla, dejarla
  // a la vista y darle el foco al editor. Corre después del efecto de carga
  // de arriba, así que si el clic también cambió de archivo el textarea ya
  // tiene el contenido nuevo.
  useEffect(() => {
    if (!jump || jump.file !== file || !taRef.current) return;
    const ta = taRef.current;
    const lines = ta.value.split("\n");
    const line = Math.min(Math.max(jump.line, 1), lines.length);
    let start = 0;
    for (let i = 0; i < line - 1; i++) start += lines[i].length + 1;
    const { pad, lh } = codeMetrics();
    ta.focus();
    ta.setSelectionRange(start, start + lines[line - 1].length);
    ta.scrollTop = Math.max(0, pad + (line - 1) * lh - ta.clientHeight / 2);
    syncScroll();
  }, [jump, file]);

  return (
    <>
      <div className="bread">
        <span className="b">src</span><span className="sep">›</span>
        <span className="b">{f.name}</span>
        <div className="right">
          <span className="pill">{lang==="sql"?"SQL":lang==="g4"?"ANTLR":"text"}</span>
          {f.readonly && <span className="dim">solo lectura</span>}
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
            dangerouslySetInnerHTML={{__html: marks + highlighted}}
          />
          <textarea
            ref={taRef}
            className="code-edit"
            defaultValue={f.rawContent}
            onChange={handleChange}
            onScroll={syncScroll}
            readOnly={!!f.readonly}
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
   `parse_tree_dot` de /api/sql/parse): el árbol de derivación que construyó
   el parser de ANTLR, con cada nodo etiquetado "regla · alternativa". Con
   errores de sintaxis se dibuja igual el árbol parcial que dejó la
   recuperación de errores de ANTLR (nodos de error en rojo). */
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
        <span className="dim" style={{fontFamily:"VT323", fontSize:16}}>· árbol de derivación de ANTLR (regla · alternativa)</span>
        {D.PARSE_TREE_DOT && (
          <button className="cbtn icon cyan" style={{fontSize:12, padding:"2px 8px"}} onClick={downloadPng}>↓ PNG</button>
        )}
      </div>
      <div ref={containerRef} className="dfa-container"/>
    </div>
  );
}

/* Fase de un problema, por el prefijo de su código (ver
   backend/dbms/frontend/parse.py y, más adelante, la semántica y el ejecutor). */
const PROBLEM_PHASE = { LEX:"léxico", SYN:"sintáctico", SEM:"semántico", EXE:"ejecución", IDE:"IDE" };

/* TODOS los problemas de la última corrida, de cualquier fase, ordenados
   por posición. Clic en uno con posición: el editor salta a esa línea. */
function ProblemsView({ onJump }){
  const problems = D.PROBLEMS
    .map((p, i) => ({ p, i }))
    .sort((a, b) => ((a.p.line ?? 1e9) - (b.p.line ?? 1e9)) || ((a.p.col ?? 0) - (b.p.col ?? 0)) || (a.i - b.i))
    .map(({ p }) => p);
  const counts = { err:problems.filter(p=>p.level==="err").length,
                   warn:problems.filter(p=>p.level==="warn").length,
                   info:problems.filter(p=>p.level==="info").length };
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ PROBLEMAS ·
        <span className="err"> {counts.err} err</span> ·
        <span className="warn"> {counts.warn} warn</span> ·
        <span className="info"> {counts.info} info</span>
      </div>
      {!problems.length && <div className="dim" style={{padding:16}}>sin problemas</div>}
      {problems.map((p,i)=>{
        const hasPos = p.line != null && p.col != null && p.line > 0;
        const locLabel = hasPos ? `línea ${p.line}, col ${p.col}` : (p.loc || "");
        const phase = PROBLEM_PHASE[(p.code || "").slice(0, 3)];
        return (
          <div key={i}
               className={"prob "+p.level+(hasPos ? " clickable" : "")}
               onClick={hasPos ? () => onJump(p) : undefined}
               title={hasPos ? "Ir a la línea en el editor" : undefined}>
            <div className="tag">{p.level==="err"?"ERR":p.level==="warn"?"WRN":"INF"}</div>
            <div style={{flex:1}}>
              <div className="msg">{p.msg}</div>
              <div className="loc" style={{display:"flex", gap:8, flexWrap:"wrap", alignItems:"center"}}>
                {p.code && <span>{p.code}</span>}
                {phase && <span className="phase">{phase}</span>}
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

function ResultsPanel({ activeTab, setActiveTab, renderKey, onToggleSize, resultsSize, onJump }){
  const problemCount = D.PROBLEMS.length;
  const TABS = [
    {id:"tokens",   label:"TOKENS", badge: D.TOKENS.length || null},
    {id:"tree",     label:"ÁRBOL SINTÁCTICO"},
    {id:"errors",   label:"PROBLEMAS", badge: problemCount || null},
  ];

  return (
    <>
      <div className="panel-title">
        <span className="swatch"/>RESULTS
        <button className="results-toggle" onClick={onToggleSize} title="Cambiar tamaño del panel">
          {resultsSize==="normal"?"⟩⟩ AMPLIAR":"⟨⟨ NORMAL"}
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
          {activeTab==="errors"  && <ProblemsView onJump={onJump}/>}
        </div>
      </div>
    </>
  );
}

/* ============================== Header ============================== */

function Header({ activeFile, setFile, onRun, onSave, loading }){
  const tabs = ["sql","g4"];
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
        <button className="runbtn" onClick={onRun} disabled={loading} style={{opacity:loading?.5:1}}
                title="Análisis léxico y sintáctico del script (todavía no ejecuta nada)">
          {loading ? "..." : <><span className="play"/>ANALIZAR</>}
        </button>
        <button className="runbtn stepbtn" onClick={onSave} title="Guardar el script en el workspace">
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

function StatusBar({ activeFile }){
  const f = D.FILES[activeFile];
  const errors = D.PROBLEMS.filter(p => p.level === "err").length;
  return (
    <div id="status">
      <div className="sg"><span className="grm">SQL · ANTLR 4</span></div>
      {D.PARSE_OK !== null && (
        <div className="sg" style={{color: D.PARSE_OK ? "var(--green)" : "var(--red)"}}>
          {D.PARSE_OK
            ? `✓ SINTAXIS OK · ${D.STATEMENTS.length} sentencia${D.STATEMENTS.length===1?"":"s"}`
            : `✗ ${errors} error${errors===1?"":"es"}`}
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
  const [activeFile,     setFile]          = useState("sql");
  const [activeTab,      setTab]           = useState("tree");
  const [loading,        setLoading]       = useState(false);
  const [renderKey,      bump]             = useState(0);
  const [contentVersion, setContentVersion] = useState(0);
  const [resultsSize,    setResultsSize]   = useState("normal"); // "normal"|"wide"|"custom"
  const [layout,         setLayout]        = useState({ left: 240, right: 460 });
  const [drag,           setDrag]          = useState(null);
  const [jump,           setJump]          = useState(null);
  const [workspace,      setWorkspace]     = useState([]);
  const appRef = useRef(null);

  const rerender = () => bump(n => n + 1);

  // Clic en un problema: todos apuntan al script SQL. El `nonce` hace que
  // dos clics seguidos en el mismo problema vuelvan a saltar.
  const handleJump = (p) => {
    setFile("sql");
    setJump({ file: "sql", line: p.line, nonce: Date.now() });
  };

  const openInSqlSlot = (name, content) => {
    D.FILES.sql.rawContent = normalizeEol(content);
    D.FILES.sql.name  = name;
    D.FILES.sql.dirty = false;
    setFile("sql");
    setContentVersion(v => v + 1);
    rerender();
  };

  const openWorkspaceFile = async (name) => {
    try {
      const res = await fetch(`${API}/api/workspace/${encodeURIComponent(name)}`);
      if (res.ok) openInSqlSlot(name, await res.text());
    } catch(e) { console.warn("no se pudo abrir", name, e); }
  };

  // ── WORKSPACE: lista los .sql del servidor y, al arrancar, abre demo.sql
  // (o el primero que haya). ──────────────────────────────────────────────
  const fetchWorkspace = async (openDefault) => {
    try {
      const res = await fetch(`${API}/api/workspace`);
      if (!res.ok) return;
      const { files } = await res.json();
      setWorkspace(files);
      if (openDefault) {
        const pick = files.find(f => f.name === "demo.sql") || files.find(f => f.kind === "sql");
        if (pick) await openWorkspaceFile(pick.name);
      }
    } catch(e) { console.warn("workspace not available, using defaults", e); }
  };

  // La gramática real, para el slot SQL.g4 (solo lectura).
  const fetchGrammar = async () => {
    try {
      const res = await fetch(`${API}/api/grammar`);
      if (!res.ok) return;
      D.FILES.g4.rawContent = normalizeEol(await res.text());
    } catch(e) {
      D.FILES.g4.rawContent = "// no se pudo cargar SQL.g4: la API no responde\n";
    }
    setContentVersion(v => v + 1);
  };

  useEffect(() => { fetchWorkspace(true); fetchGrammar(); }, []);

  const handleEdit = () => { rerender(); };

  // ── CARGAR ARCHIVO: lo abre en el slot SQL y lo sube al workspace. ─────
  const handleLoadFile = (file) => {
    const reader = new FileReader();
    reader.onload = async e => {
      const content = e.target.result;
      openInSqlSlot(file.name, content);
      try {
        await fetch(`${API}/api/workspace/${encodeURIComponent(file.name)}`, {
          method: "PUT",
          headers: { "Content-Type": "text/plain" },
          body: content,
        });
        fetchWorkspace(false);
      } catch(e) { /* funciona localmente aunque el backend falle */ }
    };
    reader.readAsText(file);
  };

  // SAVE siempre guarda el script (el slot SQL.g4 es de solo lectura).
  const handleSave = async () => {
    const f = D.FILES.sql;
    try {
      const res = await fetch(`${API}/api/workspace/${encodeURIComponent(f.name)}`, {
        method: "PUT",
        headers: { "Content-Type": "text/plain" },
        body: f.rawContent,
      });
      if (res.ok) { f.dirty = false; fetchWorkspace(false); rerender(); }
    } catch(e) {
      const blob = new Blob([f.rawContent], { type: "text/plain" });
      const url  = URL.createObjectURL(blob);
      const a    = document.createElement("a");
      a.href = url; a.download = f.name;
      document.body.appendChild(a); a.click();
      document.body.removeChild(a); URL.revokeObjectURL(url);
      f.dirty = false;
      rerender();
    }
  };

  // ── ANALIZAR: análisis léxico + sintáctico del script con ANTLR. ───────
  const handleRun = async () => {
    const f = D.FILES.sql;
    setLoading(true);
    try {
      const res = await fetch(`${API}/api/sql/parse`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({ script: f.rawContent }),
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();

      D.TOKENS = (data.tokens || []).map((t, i) => ({ i: i+1, k: t.kind, lx: t.lexeme, l: t.line, c: t.col }));
      D.PARSE_TREE_DOT = data.parse_tree_dot || "";
      D.STATEMENTS = data.statements || [];
      D.PARSE_OK = !!data.ok;
      // `loc` con el nombre del archivo: así el editor marca las líneas del
      // script (problemLine filtra por archivo).
      D.PROBLEMS = (data.problems || []).map(p => ({ ...p, loc: `${f.name}:${p.line}:${p.col}` }));

      setTab(data.ok ? "tree" : "errors");
      rerender();
    } catch(e) {
      console.error("API /sql/parse:", e);
      D.TOKENS = [];
      D.PARSE_TREE_DOT = "";
      D.STATEMENTS = [];
      D.PARSE_OK = null;
      D.PROBLEMS = [{ level:"err", code:"IDE001", msg:`No se pudo contactar la API: ${e}`, loc:"" }];
      setTab("errors");
      rerender();
    } finally {
      setLoading(false);
    }
  };

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
              loading={loading}/>

      <div id="files" className="panel" data-screen-label="files">
        <div className="panel-title">
          <span className="swatch"/>EXPLORER
        </div>
        <FileTree active={activeFile} onPick={setFile} onLoadFile={handleLoadFile}
                  workspace={workspace} onOpenWorkspace={openWorkspaceFile}/>
      </div>

      <div className="grid-handle v left" onMouseDown={() => setDrag({ kind: "left" })} />

      <div id="editor-wrap" data-screen-label="editor">
        <Editor file={activeFile} onEdit={handleEdit} contentVersion={contentVersion} jump={jump}/>
      </div>

      <div className="grid-handle v right" onMouseDown={() => setDrag({ kind: "right" })} />

      <div id="results" data-screen-label="results">
        <ResultsPanel
          activeTab={activeTab}
          setActiveTab={setTab}
          renderKey={renderKey}
          onToggleSize={cycleResults}
          resultsSize={resultsSize}
          onJump={handleJump}/>
      </div>

      <StatusBar activeFile={activeFile}/>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
