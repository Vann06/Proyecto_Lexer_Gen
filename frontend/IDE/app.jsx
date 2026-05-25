/* eslint-disable */
const { useState, useEffect, useRef, useCallback } = React;
const D = window.IDE_DATA;

const API = "http://localhost:8080";

/* ============================== Helpers ============================== */

function CodeLine({ row, cur, errFlag, warnFlag }){
  if (row.t === "blank") return <div className={"row " + (cur?"cur":"")}>&nbsp;</div>;
  if (row.t === "com")   return <div className={"row " + (cur?"cur":"")}><span className="com">{row.v}</span></div>;
  return (
    <div className={"row " + (cur?"cur ":"") + (errFlag?"err ":"") + (warnFlag?"warn":"")}>
      {row.v.map((p,i)=> Array.isArray(p)
        ? <span key={i} className={p[0]}>{p[1]}</span>
        : <span key={i}>{p}</span>)}
      {cur && <span className="caret"/>}
    </div>
  );
}

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
          ↑ input.txt
          <input type="file" accept=".txt,text/plain" hidden onChange={e => e.target.files[0] && onLoadFile("test", e.target.files[0])}/>
        </label>
      </div>

      <div className="h">▍ WORKSPACE</div>
      {["yal","yalp","test"].map(id => (
        <div key={id}
             className={"tree-row file " + D.FILES[id].kind + (active===id?" active":"")}
             onClick={() => onPick(id)}>
          <span className="icn"/>
          <span>{D.FILES[id].name}</span>
          {D.FILES[id].dirty && <span className="badge">●</span>}
        </div>
      ))}


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
    { re: /\(\*[\s\S]*?\*\)/y, cls: "com" },
    { re: /%%/y, cls: "kw" },
    { re: /%(token|start|left|right|nonassoc)\b/y, cls: "kw" },
    { re: /[A-Z][A-Z0-9_]*/y, cls: "nonterm" },
    { re: /[a-z_][a-z0-9_]*/y, cls: "term" },
    { re: /[|:;]/y, cls: "op" },
  ],
  txt: [],
};

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
  const lang = file === "yal" ? "yal" : file === "yalp" ? "yalp" : "txt";
  const [lineCount,   setLineCount]   = useState(() => f.rawContent.split('\n').length);
  const [highlighted, setHighlighted] = useState(() => tokenize(f.rawContent, lang));

  useEffect(() => {
    if (taRef.current) taRef.current.value = f.rawContent;
    setLineCount(f.rawContent.split('\n').length);
    setHighlighted(tokenize(f.rawContent, lang));
  }, [file, contentVersion]);

  const errs  = new Set(D.PROBLEMS
    .filter(p => p.level==="err"  && p.loc && p.loc.includes(f.name))
    .map(p => parseInt(p.loc.split(":")[1])).filter(n => !isNaN(n)));
  const warns = new Set(D.PROBLEMS
    .filter(p => p.level==="warn" && p.loc && p.loc.includes(f.name))
    .map(p => parseInt(p.loc.split(":")[1])).filter(n => !isNaN(n)));

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
          <span className="pill">{file==="yal"?"YALex":file==="yalp"?"YACC":"text"}</span>
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

function GrammarView(){
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ PRODUCCIONES</div>
      <table className="t">
        <thead><tr><th>#</th><th>LHS</th><th></th><th>RHS</th></tr></thead>
        <tbody>
          {D.PRODS.map(p=>
            <tr key={p.n}>
              <td className="row-h">{p.n}</td>
              <td><span className="nonterm">{p.lhs}</span></td>
              <td className="dim">→</td>
              <td style={{textAlign:"left", paddingLeft:14}}>
                {p.rhs.map((s,i)=>
                  <span key={i} className={/[A-Z]/.test(s)?"nonterm":"term"} style={{marginRight:6}}>{s}</span>
                )}
              </td>
            </tr>
          )}
        </tbody>
      </table>
      <div style={{height:14}}/>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ SÍMBOLOS</div>
      <div className="dim" style={{fontSize:17}}>
        <div>terminales: {D.TERMINALS.map(t=><span key={t} className="term" style={{marginRight:6}}>{t}</span>)}</div>
        <div>no terminales: {D.NONTERMINALS.map(t=><span key={t} className="nonterm" style={{marginRight:6}}>{t}</span>)}</div>
        <div>inicio: <span className="nonterm">{D.NONTERMINALS[0]||"S"}</span></div>
      </div>
    </div>
  );
}

function FirstFollow({ which }){
  const data = which==="first" ? D.FIRST : D.FOLLOW;
  return (
    <div>
      <div className="ff">
        <div className="h">NT</div>
        <div className="h">{which==="first"?"FIRST":"FOLLOW"}</div>
        <div className="h">| cardinalidad</div>
        {Object.entries(data).map(([k,v])=>
          <React.Fragment key={k}>
            <div className="nt">{k}</div>
            <div>{ "{ " + v.map(x=>`'${x}'`).join(", ") + " }"}</div>
            <div className="dim">{v.length}</div>
          </React.Fragment>
        )}
      </div>
      <div style={{height:14}}/>
      <div className="dim" style={{fontSize:16}}>
        ▍ Calculado por punto fijo · {Object.keys(data).length} conjuntos · sin ε
      </div>
    </div>
  );
}

function StatesView({ active, onPick }){
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ COLECCIÓN CANÓNICA · LR(1) — {D.STATES.length} estados</div>
      {D.STATES.map(s=>
        <div key={s.id}
             className={"state " + (active===s.id?"active":"")}
             onClick={()=> onPick(s.id)}>
          <div className="head">
            <span style={{color:"var(--coral)"}}>■</span>
            <span>I{s.id}</span>
            <span className="mute" style={{marginLeft:"auto", fontSize:15}}>{s.items.length} items, {Object.keys(D.GOTO[String(s.id)] || {}).length} transiciones</span>
          </div>
          <div className="items">
            {s.items.map((it,i)=> <div key={i}>{it.replace("·",  /·/.test(it)?"·":"·")}</div> )}

            {Object.keys(D.GOTO[String(s.id)] || {}).length > 0 && (
              <div style={{borderTop: '1px solid var(--gray)', margin: '8px 0'}}></div>
            )}

            {Object.entries(D.GOTO[String(s.id)] || {}).map(([symbol, nextState]) => (
              <div key={`trans-${symbol}`} style={{display: 'flex', justifyContent: 'space-between', fontSize: '0.9em', color: 'var(--mute)'}}>
                <span style={{fontWeight: 500}}>{symbol}</span>
                <span>→ I{nextState}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function LL1TableView(){
  // Reconstruct M[NT, terminal] table from D.STATES items
  // Each state item looks like "M[S, c] → C C"
  const tableData = {};
  D.STATES.forEach(state => {
    state.items.forEach(item => {
      const m = item.match(/^M\[([^,\]]+),\s*([^\]]+)\]\s*[→>]\s*(.+)$/);
      if (m) {
        const nt = m[1].trim(), term = m[2].trim(), rhs = m[3].trim();
        if (!tableData[nt]) tableData[nt] = {};
        tableData[nt][term] = rhs;
      }
    });
  });
  const nts   = Object.keys(tableData).sort();
  const terms = [...D.TERMINALS].sort();
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ TABLA LL(1) · M[NT, Terminal]</div>
      <table className="t">
        <thead>
          <tr>
            <th>NT</th>
            {terms.map(t=> <th key={t} className="term">{t}</th>)}
          </tr>
        </thead>
        <tbody>
          {nts.map(nt=>(
            <tr key={nt}>
              <td className="row-h nonterm">{nt}</td>
              {terms.map(t=>{
                const v = tableData[nt]?.[t];
                return <td key={t} className={v?"":"empty"}>{v ? <span className="re">{v}</span> : "·"}</td>;
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div style={{height:10}}/>
      <div className="dim" style={{fontSize:16}}>
        Tabla de análisis predictivo LL(1) · celdas vacías = error
      </div>
    </div>
  );
}

function ActionGotoTable({ stepIdx, mode }){
  if (mode === "ll1") return <LL1TableView/>;

  const cur = D.TRACE[stepIdx] || D.TRACE[0];
  const curState = cur.stack[cur.stack.length-1];
  const curTok   = cur.remaining[0];
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ ACTION / GOTO ·
        <span className="dim"> estado actual </span>
        <span style={{color:"var(--cyan)"}}>I{curState}</span>
        <span className="dim"> · token </span>
        <span style={{color:"var(--coral)"}}>'{curTok}'</span>
      </div>
      <table className="t">
        <thead>
          <tr>
            <th rowSpan={2}>I</th>
            <th colSpan={D.TERMINALS.length}>ACTION</th>
            <th colSpan={D.NONTERMINALS.length}>GOTO</th>
          </tr>
          <tr>
            {D.TERMINALS.map(t=> <th key={t}>{t}</th>)}
            {D.NONTERMINALS.map(t=> <th key={t}>{t}</th>)}
          </tr>
        </thead>
        <tbody>
          {D.STATES.map(s=>{
            const id = s.id;
            const isCur = id===curState;
            return (
              <tr key={id}>
                <td className="row-h">{id}</td>
                {D.TERMINALS.map(t=>{
                  const v = D.ACTION[id]?.[t];
                  const isCell = isCur && t===curTok;
                  return (
                    <td key={t} className={(isCell?"cur ":"") + (v?"":"empty")}>
                      {v
                        ? (v.startsWith("s") ? <span className="sh">{v}</span>
                          : v.startsWith("r") ? <span className="re">{v}</span>
                          : v==="acc" ? <span className="ac">acc</span>
                          : v)
                        : "·"}
                    </td>
                  );
                })}
                {D.NONTERMINALS.map(t=>{
                  const v = D.GOTO[id]?.[t];
                  return <td key={t} className={v?"":"empty"}>{v ?? "·"}</td>;
                })}
              </tr>
            );
          })}
        </tbody>
      </table>
      <div style={{height:10}}/>
      <div className="dim" style={{fontSize:16}}>
        <span style={{color:"var(--green)"}}>s‹n›</span> shift &nbsp;·&nbsp;
        <span style={{color:"var(--cyan)"}}>r‹n›</span> reduce &nbsp;·&nbsp;
        <span style={{color:"var(--pink)"}}>acc</span> aceptar
      </div>
    </div>
  );
}

function TokensView(){
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ TOKEN STREAM · {D.TOKENS.length} tokens</div>
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

function LR0Graph({ renderKey }){
  const containerRef = useRef();

  useEffect(() => {
    const dot = D.LR0_DOT;
    if (!dot || !containerRef.current) return;
    if (!window.Viz) {
      containerRef.current.innerHTML = '<div style="color:var(--yellow);padding:12px">Cargando viz.js...</div>';
      return;
    }
    window.Viz.instance().then(viz => {
      if (!containerRef.current) return;
      try {
        const svg = viz.renderSVGElement(dot);
        svg.style.maxWidth = "100%";
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

  return (
    <div className="dfa-wrap">
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ AUTÓMATA LR(0) · {D.STATES.length} estados
        {!D.LR0_DOT && <span className="dim" style={{marginLeft:10}}>· ejecuta RUN primero</span>}
      </div>
      <div className="dfa-legend">
        <span><i style={{background:"var(--cyan)"}}/>estado inicial</span>
        <span><i style={{background:"var(--magenta)"}}/>transición</span>
      </div>
      <div ref={containerRef} className="dfa-container"/>
    </div>
  );
}

function GeneratedCode(){
  // simple highlighter
  const code = D.GEN_CODE;
  // crude split for visual sugar
  const lines = code.split("\n").map((ln,i)=>{
    const t = ln
      .replace(/(\/\/[^\n]*)/g, "§com:$1§")
      .replace(/\b(pub|fn|struct|impl|let|mut|use|return|match|crate|self|Some|None)\b/g,"§kw:$1§")
      .replace(/("[^"]*"|'[^']*')/g, "§str:$1§");
    const parts = t.split(/(§\w+:[^§]*§)/).filter(Boolean).map((p,j)=>{
      const m = /^§(\w+):([\s\S]*)§$/.exec(p);
      if (m) return <span key={j} className={m[1]}>{m[2]}</span>;
      return <span key={j}>{p}</span>;
    });
    return <div key={i}>{parts.length?parts:<>&nbsp;</>}</div>;
  });
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ CÓDIGO GENERADO · build/lexer.rs · {code.split("\n").length} líneas
      </div>
      <div className="gen">{lines}</div>
    </div>
  );
}

function ProblemsList(){
  const counts = { err:D.PROBLEMS.filter(p=>p.level==="err").length,
                   warn:D.PROBLEMS.filter(p=>p.level==="warn").length,
                   info:D.PROBLEMS.filter(p=>p.level==="info").length };
  return (
    <div>
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>
        ▍ PROBLEMAS ·
        <span className="err"> {counts.err} err</span> ·
        <span className="warn"> {counts.warn} warn</span> ·
        <span className="info"> {counts.info} info</span>
      </div>
      {D.PROBLEMS.map((p,i)=>
        <div key={i} className={"prob "+p.level}>
          <div className="tag">{p.level==="err"?"ERR":p.level==="warn"?"WRN":"INF"}</div>
          <div>
            <div className="msg">{p.msg}</div>
            <div className="loc">{p.code} · {p.loc}</div>
          </div>
        </div>
      )}
    </div>
  );
}

/* ============================== Console / Trace ============================== */

function StackView({ step }){
  const stack = step.stack;
  return (
    <div className="sv">
      <div className="row">
        <div className="lbl">STACK</div>
        <div className="cells">
          {stack.map((s,i)=>{
            const isNum = typeof s === "number";
            return <div key={i} className={"cell " + (isNum?"st":"sym")}>{s}</div>;
          })}
        </div>
      </div>
      <div className="row">
        <div className="lbl">INPUT</div>
        <div className="cells">
          {step.remaining.map((s,i)=>
            <div key={i} className={"cell " + (i===0?"next":"")}>{s}</div>
          )}
        </div>
      </div>
      <div className="row">
        <div className="lbl">ACTION</div>
        <div className="cells">
          <div className="cell" style={{
            color: step.action==="acc"?"var(--pink)":
                   step.action.startsWith("s")?"var(--green)":
                   step.action.startsWith("r")?"var(--cyan)":"var(--tx)",
            borderColor:"currentColor",
            padding:"2px 14px"
          }}>{step.action}</div>
          <div className="cell" style={{flex:1, color:"var(--tx-dim)", textAlign:"left", borderColor:"var(--line-soft)"}}>
            {step.desc}
          </div>
        </div>
      </div>
    </div>
  );
}

function ParseConsole({
  stepIdx, setStep, onParse, mode, testCases,
  testCasesPanelWidth, setTestCasesPanelWidth,
  isDraggingResize, setIsDraggingResize,
  selectedTestCaseIdx, setSelectedTestCaseIdx
}){
  const [input, setInput] = useState("c c d c d");
  const cur = D.TRACE[stepIdx] || D.TRACE[0];
  const isAccepted = cur && cur.action === "acc";
  const hasTestCases = testCases && testCases.length > 0;

  const handleTestCaseClick = (testCase, idx) => {
    setInput(testCase);
    setSelectedTestCaseIdx(idx);
    onParse(testCase);
  };

  const traceAreaWidth = hasTestCases ? (1 - testCasesPanelWidth) : 1;
  const resizeHandleLeft = `${testCasesPanelWidth * 100}%`;

  return (
    <div className="panel" style={{ display: 'flex', flexDirection: 'column' }}>
      <div className="panel-title" style={{position:"absolute", top:-14, left:14}}>
        <span className="swatch"/>PARSE CONSOLE
        <span style={{color:"var(--tx-mute)", marginLeft:10}}>{MODE_LABELS[mode]} · step {stepIdx+1}/{D.TRACE.length}</span>
      </div>

      {/* Input bar */}
      <div className="console-top" style={{marginTop:14}}>
        <div className="input-frame">
          <span className="prompt">›</span>
          <input value={input} onChange={e=>setInput(e.target.value)}
                 onKeyDown={e=>e.key==="Enter" && onParse(input)}/>
          <span className="dim" style={{fontSize:15}}>$</span>
        </div>
        <button className="cbtn green" onClick={()=>onParse(input)}>▶ PARSEAR</button>
        <button className="cbtn icon cyan" onClick={()=>setStep(0)}>⏮</button>
        <button className="cbtn icon cyan" onClick={()=>setStep(Math.max(0,stepIdx-1))}>◀ PASO</button>
        <button className="cbtn icon cyan" onClick={()=>setStep(Math.min(D.TRACE.length-1,stepIdx+1))}>PASO ▶</button>
        <button className="cbtn icon" onClick={()=>setStep(D.TRACE.length-1)}>⏭</button>
      </div>

      {/* Content area - 2 columnas si hay test cases */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden', gap: 0 }}>

        {/* Columna izquierda: Test Cases */}
        {hasTestCases && (
          <div style={{ width: `${testCasesPanelWidth * 100}%`, borderRight: '1px solid var(--line)', overflow: 'auto', padding: '12px' }}>
            <div style={{ fontSize: 11, color: 'var(--mute)', marginBottom: 12, fontWeight: 500 }}>
              TEST CASES ({testCases.length})
            </div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
              {testCases.map((testCase, idx) => (
                <div
                  key={idx}
                  onClick={() => handleTestCaseClick(testCase, idx)}
                  style={{
                    padding: '8px 10px',
                    backgroundColor: selectedTestCaseIdx === idx ? 'var(--bg-3)' : 'var(--bg-soft)',
                    border: selectedTestCaseIdx === idx ? '1px solid var(--cyan)' : '1px solid var(--line-soft)',
                    cursor: 'pointer',
                    fontSize: 12,
                    borderRadius: 2,
                    transition: 'all 0.2s',
                    color: 'var(--cyan)',
                    fontWeight: selectedTestCaseIdx === idx ? 'bold' : 'normal'
                  }}
                  onMouseEnter={(e) => { if (selectedTestCaseIdx !== idx) e.target.style.backgroundColor = 'var(--bg-3)'; }}
                  onMouseLeave={(e) => { if (selectedTestCaseIdx !== idx) e.target.style.backgroundColor = 'var(--bg-soft)'; }}
                >
                  <span style={{ color: 'var(--mute)' }}>#{idx + 1}</span> {testCase}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Resize handle */}
        {hasTestCases && (
          <div
            style={{
              width: '4px',
              backgroundColor: isDraggingResize ? 'var(--cyan)' : 'var(--line)',
              cursor: 'ew-resize',
              userSelect: 'none',
              transition: 'background-color 0.1s'
            }}
            onMouseDown={() => setIsDraggingResize(true)}
            onMouseEnter={(e) => e.target.style.backgroundColor = 'var(--cyan)'}
            onMouseLeave={(e) => { if (!isDraggingResize) e.target.style.backgroundColor = 'var(--line)'; }}
          />
        )}

        {/* Columna derecha: Trace */}
        <div style={{ width: hasTestCases ? `${traceAreaWidth * 100}%` : '100%', overflow: 'auto' }}>
          <div className="trace-wrap" style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
            <div className="trace-col left" style={{ flex: 1, overflowY: 'auto' }}>
              <h4>▍ TRAZA DE EJECUCIÓN</h4>
              {D.TRACE.map((s,i)=>{
                const a = s.action;
                const cls = a==="acc"?"a-ac":a.startsWith("s")?"a-sh":a.startsWith("r")?"a-re":"a-er";
                return (
                  <div key={i} className={"step " + (i===stepIdx?"cur":"")} onClick={()=>setStep(i)}>
                    <div className="n">{String(i+1).padStart(2,"0")}</div>
                    <div>
                      <span className="dim">I{s.stack[s.stack.length-1]}</span>{' '}
                      <span className="dim">·</span>{' '}
                      <span style={{color:"var(--coral)"}}>'{s.remaining[0]}'</span>{' '}
                      <span className="dim">→</span>{' '}
                      <span className={cls}>{a}</span>{' '}
                      <span className="dim">· {s.desc.split(" → ").slice(-1)[0]}</span>
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="trace-col" style={{ flex: 1, overflowY: 'auto', borderTop: '1px solid var(--line)' }}>
              <h4>▍ ESTADO ACTUAL · paso {stepIdx+1}</h4>
              <StackView step={cur}/>
              {isAccepted &&
                <div className="accept-banner">
                  <span>✓</span><span>CADENA ACEPTADA</span>
                  <span className="dim" style={{fontFamily:"VT323", fontSize:14}}>· {D.TRACE.length} pasos · 0 errores</span>
                </div>
              }
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ============================== Right results panel ============================== */

function ResultsPanel({ stepIdx, activeTab, setActiveTab, activeState, setActiveState, mode, renderKey }){
  const TABS = [
    {id:"grammar",   label:"GRAMÁTICA"},
    {id:"first",     label:"FIRST"},
    {id:"follow",    label:"FOLLOW"},
    {id:"states",    label:"ESTADOS", badge: D.STATES.length},
    {id:"action",    label:"ACTION/GOTO"},
    {id:"tokens",    label:"TOKENS", badge: D.TOKENS.length},
    {id:"dfa",       label:"LR(0)"},
    {id:"gen",       label:"CÓD.GEN"},
    {id:"problems",  label:"PROBLEMAS", badge: D.PROBLEMS.length},
  ];

  return (
    <>
      <div className="panel-title">
        <span className="swatch"/>RESULTS · ANALYZER
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
          {activeTab==="grammar"  && <GrammarView/>}
          {activeTab==="first"    && <FirstFollow which="first"/>}
          {activeTab==="follow"   && <FirstFollow which="follow"/>}
          {activeTab==="states"   && <StatesView active={activeState} onPick={setActiveState}/>}
          {activeTab==="action"   && <ActionGotoTable stepIdx={stepIdx} mode={mode}/>}
          {activeTab==="tokens"   && <TokensView/>}
          {activeTab==="dfa"      && <LR0Graph renderKey={renderKey}/>}
          {activeTab==="gen"      && <GeneratedCode/>}
          {activeTab==="problems" && <ProblemsList/>}
        </div>
      </div>
    </>
  );
}

/* ============================== Header ============================== */

const MODE_LABELS = { lalr:"LALR(1)", slr:"SLR(1)", ll1:"LL(1)" };

function Header({ activeFile, setFile, onRun, onSave, loading, mode, setMode }){
  const tabs = ["yal","yalp","test"];
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
          {loading ? "..." : <><span className="play"/>RUN</>}
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

function StatusBar({ activeFile, stepIdx, mode }){
  const f = D.FILES[activeFile];
  return (
    <div id="status">

      <div className="sg"><span className="sw" style={{background:"var(--yellow)"}}/>3</div>
      <div className="sg"><span className="sw" style={{background:"var(--coral)"}}/>1</div>
      <div className="sg dim">main ●</div>
      <div className="sg dim">build · ok</div>
      <div className="sg"><span className="grm">{MODE_LABELS[mode]}</span> · sin conflictos</div>
      <div className="right">
        <div className="sg">LN {f.current+1}</div>
        <div className="sg">COL 14</div>
        <div className="sg">{activeFile==="yal"?"YALex":activeFile==="yalp"?"YACC":"UTF-8"}</div>
        <div className="sg"><span className="pink">PASO</span> {stepIdx+1}/{D.TRACE.length}</div>
        <div className="sg">UTF-8</div>
      </div>
    </div>
  );
}

/* ============================== App ============================== */

function App(){
  const [activeFile,     setFile]          = useState("yalp");
  const [activeTab,      setTab]           = useState("action");
  const [activeState,    setState]         = useState(3);
  const [stepIdx,        setStep]          = useState(3);
  const [loading,        setLoading]       = useState(false);
  const [mode,           setMode]          = useState("lalr");
  const [renderKey,      bump]             = useState(0); // re-render global
  const [contentVersion, setContentVersion] = useState(0); // señal de carga externa

  const rerender = () => bump(n => n + 1);

  // ── TEST CASES desde archivo .txt ─────────────────────────────────────────
  const [testCasesPanelWidth, setTestCasesPanelWidth] = useState(() => {
    const saved = localStorage.getItem('syntra-testCasesPanelWidth');
    return saved ? parseFloat(saved) : 0.35;
  });
  const [isDraggingResize, setIsDraggingResize] = useState(false);
  const [selectedTestCaseIdx, setSelectedTestCaseIdx] = useState(() => {
    const saved = localStorage.getItem('syntra-selectedTestCaseIdx');
    return saved ? parseInt(saved) : 0;
  });

  // ── CONSOLE RESIZE (vertical entre RESULTS y CONSOLE) ─────────────────────
  const [consoleHeight, setConsoleHeight] = useState(0.50); // NO se guarda
  const [isDraggingConsoleResize, setIsDraggingConsoleResize] = useState(false);

  const testCases = React.useMemo(() => {
    if (!D.FILES.test || !D.FILES.test.rawContent) return [];
    return D.FILES.test.rawContent
      .split('\n')
      .map(line => line.trim())
      .filter(line => line.length > 0);
  }, [contentVersion]);

  // Guardar testCasesPanelWidth en localStorage
  React.useEffect(() => {
    localStorage.setItem('syntra-testCasesPanelWidth', testCasesPanelWidth.toString());
  }, [testCasesPanelWidth]);

  // Guardar selectedTestCaseIdx en localStorage
  React.useEffect(() => {
    localStorage.setItem('syntra-selectedTestCaseIdx', selectedTestCaseIdx.toString());
  }, [selectedTestCaseIdx]);

  // Resize listener (horizontal - test cases panel)
  React.useEffect(() => {
    if (!isDraggingResize) return;

    const handleMouseMove = (e) => {
      setTestCasesPanelWidth(prev => {
        const newWidth = prev + (e.movementX / window.innerWidth);
        return Math.max(0.15, Math.min(0.6, newWidth)); // Min 15%, Max 60%
      });
    };

    const handleMouseUp = () => {
      setIsDraggingResize(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDraggingResize]);

  // Resize listener (vertical - console panel)
  React.useEffect(() => {
    if (!isDraggingConsoleResize) return;

    const handleMouseMove = (e) => {
      setConsoleHeight(prev => {
        const newHeight = prev - (e.movementY / window.innerHeight);
        return Math.max(0.15, Math.min(0.6, newHeight)); // Min 15%, Max 60%
      });
    };

    const handleMouseUp = () => {
      setIsDraggingConsoleResize(false);
    };

    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
  }, [isDraggingConsoleResize]);

  // ── WORKSPACE: carga archivos del servidor al arrancar ──────────────────────
  const fetchWorkspace = async () => {
    try {
      const res = await fetch(`${API}/api/workspace`);
      if (!res.ok) return;
      const { files } = await res.json();
      for (const { name, kind } of files) {
        const slot = kind === "yal" ? "yal" : kind === "yalp" ? "yalp" : "test";
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

  // sincroniza el estado resaltado en la tabla cuando cambia el paso
  useEffect(()=>{
    const cur = D.TRACE[stepIdx];
    if (!cur) return;
    const top = cur.stack[cur.stack.length - 1];
    if (typeof top === "number") setState(top);
  }, [stepIdx]);

  // ── EDITAR: Editor ya actualizó D.FILES directamente — solo refrescamos UI ──
  const handleEdit = () => {
    rerender(); // actualiza indicadores dirty en sidebar y header
  };

  // ── CARGAR ARCHIVO: lee un File y también lo sube al workspace ──────────────
  const handleLoadFile = (fileId, file) => {
    const reader = new FileReader();
    reader.onload = async e => {
      const content = e.target.result;
      D.FILES[fileId].rawContent = content;
      D.FILES[fileId].name  = file.name;
      D.FILES[fileId].dirty = false;
      try {
        await fetch(`${API}/api/workspace/${encodeURIComponent(file.name)}`, {
          method: "PUT",
          headers: { "Content-Type": "text/plain" },
          body: content,
        });
      } catch(e) { /* funciona localmente aunque el backend falle */ }
      setFile(fileId);
      setContentVersion(v => v + 1);
      rerender();
    };
    reader.readAsText(file);
  };

  // ── GUARDAR: escribe en el workspace del servidor ───────────────────────────
  const handleSave = async () => {
    const f = D.FILES[activeFile];
    try {
      const res = await fetch(`${API}/api/workspace/${encodeURIComponent(f.name)}`, {
        method: "PUT",
        headers: { "Content-Type": "text/plain" },
        body: f.rawContent,
      });
      if (res.ok) { D.FILES[activeFile].dirty = false; rerender(); }
    } catch(e) {
      // fallback: descarga Blob si el backend no está disponible
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

  // ── RUN: compila la gramática y actualiza TODA la data ──────────────────────
  const handleRun = async () => {
    setLoading(true);
    try {
      const res = await fetch(`${API}/api/parser/compile`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({ content: D.FILES.yalp.rawContent, mode }),
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();

      // Muta D en lugar de reemplazarlo: todos los componentes leen D.X
      Object.assign(D, {
        STATES:        data.states,
        ACTION:        data.action,
        GOTO:          data.goto,
        TERMINALS:     data.terminals,
        NONTERMINALS:  data.non_terminals,
        FIRST:         data.first,
        FOLLOW:        data.follow,
        PRODS:         data.prods,
        PROBLEMS:      data.problems,
        LR0_DOT:       data.lr0_dot || "",
      });
      setStep(0);
      rerender();

      // Si hay archivo .txt cargado, auto-ejecutar el primer test case
      if (D.FILES.test && D.FILES.test.rawContent && D.FILES.test.rawContent.trim()) {
        const firstLine = D.FILES.test.rawContent
          .split('\n')
          .map(l => l.trim())
          .find(l => l.length > 0);
        if (firstLine) {
          setTimeout(() => {
            const tokens = firstLine.trim().split(/\s+/).filter(Boolean);
            if (tokens.length > 0) {
              (async () => {
                try {
                  const parseRes = await fetch(`${API}/api/parser/parse`, {
                    method:  "POST",
                    headers: { "Content-Type": "application/json" },
                    body:    JSON.stringify({ content: D.FILES.yalp.rawContent, tokens, mode }),
                  });
                  if (parseRes.ok) {
                    const parseData = await parseRes.json();
                    D.TRACE = parseData.trace;
                    setStep(0);
                    rerender();
                  }
                } catch(e) { console.error("Auto-parse failed:", e); }
              })();
            }
          }, 100);
        }
      }
    } catch(e) {
      console.error("API /compile:", e);
      D.PROBLEMS = [{ level:"err", code:"E000", msg: String(e), loc:"api" }];
      rerender();
    } finally {
      setLoading(false);
    }
  };

  // ── PARSEAR: obtiene la traza para los tokens ingresados ────────────────────
  const handleParse = async (inputStr) => {
    const tokens = inputStr.trim().split(/\s+/).filter(Boolean);
    if (!tokens.length) return;
    try {
      const res = await fetch(`${API}/api/parser/parse`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({ content: D.FILES.yalp.rawContent, tokens, mode }),
      });
      if (!res.ok) throw new Error(await res.text());
      const data = await res.json();
      D.TRACE = data.trace;
      setStep(0);
      rerender();
    } catch(e) {
      console.error("API /parse:", e);
    }
  };

  return (
    <div id="app">
      <Header activeFile={activeFile} setFile={setFile} onRun={handleRun} onSave={handleSave}
              loading={loading} mode={mode} setMode={setMode}/>

      <div id="files" className="panel" data-screen-label="files">
        <div className="panel-title">
          <span className="swatch"/>EXPLORER
        </div>
        <FileTree active={activeFile} onPick={setFile} onLoadFile={handleLoadFile}/>
      </div>

      <div id="editor-wrap" data-screen-label="editor">
        <Editor file={activeFile} onEdit={handleEdit} contentVersion={contentVersion}/>
      </div>

      <div id="results" data-screen-label="results" style={{ height: `${(1 - consoleHeight) * 100}%`, overflow: 'auto' }}>
        <ResultsPanel
          stepIdx={stepIdx}
          activeTab={activeTab}
          setActiveTab={setTab}
          activeState={activeState}
          setActiveState={setState}
          mode={mode}
          renderKey={renderKey}/>
      </div>

      {/* Resize handle vertical */}
      <div
        style={{
          height: '4px',
          backgroundColor: isDraggingConsoleResize ? 'var(--cyan)' : 'var(--line)',
          cursor: 'ns-resize',
          userSelect: 'none',
          transition: 'background-color 0.1s'
        }}
        onMouseDown={() => setIsDraggingConsoleResize(true)}
        onMouseEnter={(e) => e.target.style.backgroundColor = 'var(--cyan)'}
        onMouseLeave={(e) => { if (!isDraggingConsoleResize) e.target.style.backgroundColor = 'var(--line)'; }}
      />

      <div id="console-area" data-screen-label="console" style={{ height: `${consoleHeight * 100}%`, overflow: 'hidden' }}>
        <ParseConsole
          stepIdx={stepIdx}
          setStep={setStep}
          onParse={handleParse}
          mode={mode}
          testCases={testCases}
          testCasesPanelWidth={testCasesPanelWidth}
          setTestCasesPanelWidth={setTestCasesPanelWidth}
          isDraggingResize={isDraggingResize}
          setIsDraggingResize={setIsDraggingResize}
          selectedTestCaseIdx={selectedTestCaseIdx}
          setSelectedTestCaseIdx={setSelectedTestCaseIdx}
        />
      </div>

      <StatusBar activeFile={activeFile} stepIdx={stepIdx} mode={mode}/>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
