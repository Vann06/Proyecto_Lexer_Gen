/* eslint-disable */
// IDE-lite del DBMS: editor SQL con resaltado + sidebar de archivos + las
// vistas: tokens, árbol de derivación de ANTLR, AST y problemas (▶ ANALIZAR,
// `POST /api/sql/parse`), y resultados y catálogo (▶ EJECUTAR,
// `POST /api/sql/run` y `GET /api/catalog`).
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

/* Los dos árboles vienen del backend YA renderizados a DOT (campos de
   /api/sql/parse):
   - ÁRBOL SINTÁCTICO (`parse_tree_dot`): el árbol de derivación que construyó
     el parser de ANTLR, con cada nodo etiquetado "regla · alternativa". Con
     errores de sintaxis se dibuja igual el árbol parcial que dejó la
     recuperación de errores de ANTLR (nodos de error en rojo).
   - AST (`ast_dot`): los nodos de dbms.ast, con aristas rotuladas por campo.
     Solo existe si no hubo errores de sintaxis. */
function DotView({ dot, title, subtitle, filename, emptyMsg, renderKey }) {
  const containerRef = useRef();

  useEffect(() => {
    if (!dot) {
      if (containerRef.current)
        containerRef.current.innerHTML = `<div class="dim" style="padding:12px;font-size:17px">${emptyMsg}</div>`;
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
  }, [renderKey, dot]);

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
      a.download = filename;
      a.click();
    };
    img.src = "data:image/svg+xml;base64," + btoa(unescape(encodeURIComponent(data)));
  };

  return (
    <div className="dfa-wrap">
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8, display:"flex", alignItems:"center", gap:10}}>
        ▍ {title}
        <span className="dim" style={{fontFamily:"VT323", fontSize:16}}>· {subtitle}</span>
        {dot && (
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

/* Una tabla de datos (resultado de un SELECT o columnas del catálogo).
   NULL se distingue de la cadena vacía. */
function DataTable({ columns, rows }){
  return (
    <div className="dtable-wrap">
      <table className="dtable">
        <thead><tr>{columns.map((c,i)=><th key={i}>{c}</th>)}</tr></thead>
        <tbody>
          {rows.map((r,i)=>
            <tr key={i}>{r.map((v,j)=>
              <td key={j} className={v===null?"null":""}>{v===null ? "NULL" : String(v)}</td>)}
            </tr>)}
        </tbody>
      </table>
    </div>
  );
}

/* Una tarjeta por sentencia ejecutada (campo `statements` de /api/sql/run).
   La ejecución se detiene en la primera con error: esa queda en rojo. */
function ExecResultsView({ onJump }){
  if (D.RESULTS === null)
    return <div className="dim" style={{padding:16}}>Presiona ▶ EJECUTAR para correr el script</div>;
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ RESULTADOS · {D.RESULTS.length} sentencia{D.RESULTS.length===1?"":"s"} · base {D.DATABASE}
      </div>
      {!D.RESULTS.length && <div className="dim" style={{padding:16}}>no se ejecutó ninguna sentencia (ver PROBLEMAS)</div>}
      {D.RESULTS.map((s,i)=>{
        const r = s.result;
        return (
          <div key={i} className={"stmt-card" + (r ? "" : " failed")}>
            <div className="stmt-head clickable" onClick={() => onJump(s)} title="Ir a la línea en el editor">
              <span className="k">{s.kind}</span>
              <span className="dim">línea {s.line}</span>
              {r && r.plan && <span className="plan">plan: {r.plan}</span>}
            </div>
            <div className="stmt-sql">{s.sql}</div>
            {r && r.columns.length > 0 && <>
              <DataTable columns={r.columns} rows={r.rows}/>
              <div className="dim">{r.rows.length} fila{r.rows.length===1?"":"s"}</div>
            </>}
            {r && r.message && <div className="stmt-msg">✓ {r.message}</div>}
            {s.problems.map((p,j)=>
              <div key={j} className={p.level}>{p.code} · {p.msg}</div>)}
          </div>
        );
      })}
    </div>
  );
}

/* Esquema de la base activa (GET /api/catalog): tablas con columnas,
   restricciones y cantidad de filas, e índices. */
function CatalogView(){
  const c = D.CATALOG;
  if (!c) return <div className="dim" style={{padding:16}}>catálogo no disponible (¿la API está corriendo?)</div>;
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ CATÁLOGO · base {c.database}
        <span className="dim" style={{fontFamily:"VT323", fontSize:16}}> · bases: {c.databases.join(", ")}</span>
      </div>
      {!c.tables.length && <div className="dim" style={{padding:16}}>la base no tiene tablas</div>}
      {c.tables.map(t=>
        <div key={t.name} className="stmt-card">
          <div className="stmt-head"><span className="k">{t.name}</span><span className="dim">{t.rows} fila{t.rows===1?"":"s"}</span></div>
          <DataTable columns={["columna","tipo","not null"]}
                     rows={t.columns.map(col=>[col.name, col.type, col.not_null ? "✓" : ""])}/>
          {t.constraints.map(k=>
            <div key={k.name} className="dim">
              {k.kind} {k.name} ({k.columns.join(", ")}){k.ref_table ? ` → ${k.ref_table}(${k.ref_columns.join(", ")})` : ""}{k.check ? ` ${k.check}` : ""}
            </div>)}
        </div>
      )}
      {c.indexes.length > 0 && <>
        <div className="h-pixel" style={{color:"var(--pink)", margin:"12px 0 8px"}}>▍ ÍNDICES</div>
        <DataTable columns={["índice","tabla","columnas","único","implícito"]}
                   rows={c.indexes.map(i=>[i.name, i.table, i.columns.join(", "), i.unique ? "✓" : "", i.implicit ? "✓" : ""])}/>
      </>}
    </div>
  );
}

/* ============================== Right results panel ============================== */

function ResultsPanel({ activeTab, setActiveTab, renderKey, onToggleSize, resultsSize, onJump }){
  const problemCount = D.PROBLEMS.length;
  const TABS = [
    {id:"tokens",   label:"TOKENS", badge: D.TOKENS.length || null},
    {id:"tree",     label:"ÁRBOL SINTÁCTICO"},
    {id:"ast",      label:"AST"},
    {id:"errors",   label:"PROBLEMAS", badge: problemCount || null},
    {id:"results",  label:"RESULTADOS", badge: D.RESULTS ? D.RESULTS.length : null},
    {id:"catalog",  label:"CATÁLOGO"},
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
          {activeTab==="tree"    && <DotView key="tree" dot={D.PARSE_TREE_DOT} renderKey={renderKey}
                                             title="ÁRBOL SINTÁCTICO" subtitle="árbol de derivación de ANTLR (regla · alternativa)"
                                             filename="arbol.png" emptyMsg="Presiona ▶ ANALIZAR para ver el árbol"/>}
          {activeTab==="ast"     && <DotView key="ast" dot={D.AST_DOT} renderKey={renderKey}
                                             title="AST" subtitle="nodos de dbms.ast · aristas = campo del nodo"
                                             filename="ast.png"
                                             emptyMsg={D.PARSE_OK === false ? "Sin AST: el script tiene errores de sintaxis (ver PROBLEMAS)"
                                                                            : "Presiona ▶ ANALIZAR para ver el AST"}/>}
          {activeTab==="errors"  && <ProblemsView onJump={onJump}/>}
          {activeTab==="results" && <ExecResultsView onJump={onJump}/>}
          {activeTab==="catalog" && <CatalogView/>}
        </div>
      </div>
    </>
  );
}

/* ============================== Header ============================== */

function Header({ activeFile, setFile, onRun, onExec, onSave, loading }){
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
                title="Análisis léxico, sintáctico y semántico del script (no ejecuta nada)">
          {loading ? "..." : <><span className="play"/>ANALIZAR</>}
        </button>
        <button className="runbtn execbtn" onClick={onExec} disabled={loading} style={{opacity:loading?.5:1}}
                title="Ejecuta el script sentencia por sentencia; se detiene en el primer error">
          {loading ? "..." : <><span className="play"/>EJECUTAR</>}
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
  const semErrors = D.PROBLEMS.filter(p => p.level === "err" && (p.code || "").startsWith("SEM")).length;
  const n = D.STATEMENTS.length;
  return (
    <div id="status">
      <div className="sg"><span className="grm">SQL · ANTLR 4</span></div>
      {D.PARSE_OK !== null && (
        <div className="sg" style={{color: D.PARSE_OK ? "var(--green)" : "var(--red)"}}>
          {D.PARSE_OK
            ? `✓ SINTAXIS OK · ${n} sentencia${n===1?"":"s"}`
            : `✗ ${errors} error${errors===1?"":"es"}`}
        </div>
      )}
      {D.PARSE_OK && (
        <div className="sg" style={{color: semErrors ? "var(--red)" : "var(--green)"}}>
          {semErrors ? `✗ ${semErrors} error${semErrors===1?"":"es"} semántico${semErrors===1?"":"s"}` : "✓ SEMÁNTICA OK"}
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

  const fetchCatalog = async () => {
    try {
      const res = await fetch(`${API}/api/catalog?database=${encodeURIComponent(D.DATABASE)}`);
      D.CATALOG = res.ok ? await res.json() : null;
    } catch(e) { D.CATALOG = null; }
    rerender();
  };

  useEffect(() => { fetchWorkspace(true); fetchGrammar(); fetchCatalog(); }, []);

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
      D.AST_DOT = data.ast_dot || "";
      D.STATEMENTS = data.statements || [];
      D.PARSE_OK = !!data.ok;
      // `loc` con el nombre del archivo: así el editor marca las líneas del
      // script (problemLine filtra por archivo).
      D.PROBLEMS = (data.problems || []).map(p => ({ ...p, loc: `${f.name}:${p.line}:${p.col}` }));

      // Con cualquier error (sintáctico o semántico) se abre PROBLEMAS.
      setTab(D.PROBLEMS.some(p => p.level === "err") ? "errors" : "tree");
      rerender();
    } catch(e) {
      console.error("API /sql/parse:", e);
      D.TOKENS = [];
      D.PARSE_TREE_DOT = "";
      D.AST_DOT = "";
      D.STATEMENTS = [];
      D.PARSE_OK = null;
      D.PROBLEMS = [{ level:"err", code:"IDE001", msg:`No se pudo contactar la API: ${e}`, loc:"" }];
      setTab("errors");
      rerender();
    } finally {
      setLoading(false);
    }
  };

  // ── EJECUTAR: semántica + ejecución sentencia por sentencia (/api/sql/run).
  // Escribe en la base: después se refresca el catálogo. ─────────────────
  const handleExec = async () => {
    const f = D.FILES.sql;
    setLoading(true);
    try {
      const res = await fetch(`${API}/api/sql/run`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({ script: f.rawContent, database: D.DATABASE }),
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();
      D.RESULTS = data.statements || [];
      D.DATABASE = data.database || D.DATABASE;
      D.PROBLEMS = (data.problems || []).map(p => ({ ...p, loc: `${f.name}:${p.line}:${p.col}` }));
      // Con errores de sintaxis no se ejecutó nada: se ve mejor en PROBLEMAS.
      setTab(D.RESULTS.length ? "results" : "errors");
      await fetchCatalog();
    } catch(e) {
      console.error("API /sql/run:", e);
      D.RESULTS = null;
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
      <Header activeFile={activeFile} setFile={setFile} onRun={handleRun} onExec={handleExec} onSave={handleSave}
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
