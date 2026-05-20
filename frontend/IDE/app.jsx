/* eslint-disable */
const { useState, useEffect, useRef } = React;
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

function FileTree({ active, onPick }){
  const Row = ({depth=0, kind, icon, label, file, badge}) => (
    <div className={"tree-row " + (kind||"file") + (kind!=="folder"&&kind!=="folder-open" ? " "+(D.FILES[file]?.kind||"") : "") + (active===file?" active":"")}
         onClick={()=> file && onPick(file)}>
      {Array.from({length:depth}).map((_,i)=><span key={i} className="indent"/>)}
      <span className="icn"/>
      <span>{label}</span>
      {badge && <span className="badge">{badge}</span>}
    </div>
  );
  return (
    <div className="filetree">
      <div className="h">▍ FILES · syntra</div>
      <Row kind="folder" label="src"/>
      <Row depth={1} file="yal"  label="lexer.yal" badge="!"/>
      <Row depth={1} file="yalp" label="parser.yalp"/>
      <div className="tree-row folder"><span className="icn"/>build</div>
      <div className="tree-row file" style={{opacity:.55}}>
        <span className="indent"/><span className="icn" style={{background:"var(--tx-mute)"}}/>lexer.rs
      </div>
      <div className="tree-row file" style={{opacity:.55}}>
        <span className="indent"/><span className="icn" style={{background:"var(--tx-mute)"}}/>parser.rs
      </div>
      <Row kind="folder" label="tests"/>
      <Row depth={1} file="test" label="input.txt"/>
      <div style={{height:8}}/>
      <div className="tree-row file"><span className="icn" style={{background:"var(--yellow)"}}/>Cargo.toml</div>
      <div className="tree-row file"><span className="icn" style={{background:"var(--tx-mute)"}}/>README.md</div>

      <div className="h" style={{marginTop:24}}>▍ ANALYSIS</div>
      <div className="tree-row file"><span className="icn" style={{background:"var(--cyan)"}}/>NFA
        <span className="badge">12 q</span>
      </div>
      <div className="tree-row file"><span className="icn" style={{background:"var(--cyan)"}}/>DFA
        <span className="badge">7 q</span>
      </div>
      <div className="tree-row file"><span className="icn" style={{background:"var(--coral)"}}/>LR(1)
        <span className="badge">10 I</span>
      </div>
    </div>
  );
}

/* ============================== Editor ============================== */

function Editor({ file }){
  const f = D.FILES[file];
  const total = f.src.length;
  // problems by line (mock per file)
  const warns = file==="yal" ? new Set([10]) : new Set();
  const errs  = file==="yal" ? new Set([1])  : new Set();

  return (
    <>
      <div className="bread">
        <span className="b">src</span><span className="sep">›</span>
        <span className="b">{f.name}</span>
        <div className="right">
          <span className="pill">{file==="yal"?"YALex":file==="yalp"?"YACC":"text"}</span>
          <span>● modificado</span>
          <span>UTF-8</span>
        </div>
      </div>
      <div id="editor" className="panel cyan">
        <div className="gutter">
          {f.src.map((_,i)=>{
            const n = i+1;
            const c = [
              "ln",
              n===f.current+1 ? "cur" : "",
              errs.has(n) ? "err" : "",
              warns.has(n) ? "warn" : "",
            ].join(" ");
            return <div key={i} className={c}>{String(n).padStart(2,"0")}</div>;
          })}
        </div>
        <div className="code">
          {f.src.map((row,i)=>
            <CodeLine key={i} row={row}
                      cur={i===f.current}
                      errFlag={errs.has(i+1)}
                      warnFlag={warns.has(i+1)}/>
          )}
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
            <span className="mute" style={{marginLeft:"auto", fontSize:15}}>{s.items.length} ítems</span>
          </div>
          <div className="items">
            {s.items.map((it,i)=> <div key={i}>{it.replace("·",  /·/.test(it)?"·":"·")}</div> )}
          </div>
        </div>
      )}
    </div>
  );
}

function ActionGotoTable({ stepIdx }){
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

function DfaGraph(){
  // Mini DFA: states 0..4, transitions for example lexer
  const N = [
    {id:0, x:60,  y:80, accept:false, label:"q0"},
    {id:1, x:200, y:40, accept:true,  label:"q1·ID"},
    {id:2, x:200, y:140,accept:true,  label:"q2·NUM"},
    {id:3, x:360, y:40, accept:true,  label:"q3·ID"},
    {id:4, x:360, y:140,accept:true,  label:"q4·NUM"},
  ];
  const E = [
    {a:0,b:1, l:"[a-z]"},
    {a:0,b:2, l:"[0-9]"},
    {a:1,b:3, l:"[a-z0-9]"},
    {a:3,b:3, l:"[a-z0-9]", loop:true},
    {a:2,b:4, l:"[0-9]"},
    {a:4,b:4, l:"[0-9]", loop:true},
  ];
  const find = id => N.find(n=>n.id===id);
  const r = 22;

  return (
    <div className="dfa-wrap">
      <div className="h-pixel" style={{color:"var(--pink)", marginBottom:8}}>▍ DFA · lexer · 5 estados · 6 transiciones</div>
      <div className="dfa-legend">
        <span><i style={{background:"var(--cyan)"}}/>start</span>
        <span><i style={{background:"var(--green)",borderRadius:0}}/>accept</span>
        <span><i style={{background:"var(--magenta)"}}/>transition</span>
      </div>
      <div className="dfa" style={{border:"1px solid var(--line)", background:"#080410", padding:8}}>
        <svg viewBox="0 0 440 200" style={{maxHeight:280}}>
          <defs>
            <marker id="ah" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
              <path d="M0,0 L10,5 L0,10 z" fill="#c026d3"/>
            </marker>
          </defs>
          {E.map((e,i)=>{
            const a = find(e.a), b = find(e.b);
            if (e.loop){
              return (
                <g key={i}>
                  <path d={`M ${a.x+12} ${a.y-r} q 18 -30 36 0`} fill="none" stroke="#c026d3" strokeWidth="1.5" markerEnd="url(#ah)"/>
                  <text x={a.x+30} y={a.y-r-18} fill="#f9a8d4" fontFamily="VT323" fontSize="14" textAnchor="middle">{e.l}</text>
                </g>
              );
            }
            // shorten endpoints
            const dx = b.x-a.x, dy=b.y-a.y, L = Math.hypot(dx,dy);
            const ux=dx/L, uy=dy/L;
            const x1 = a.x + ux*r, y1=a.y+uy*r;
            const x2 = b.x - ux*r, y2=b.y-uy*r;
            return (
              <g key={i}>
                <line x1={x1} y1={y1} x2={x2} y2={y2} stroke="#c026d3" strokeWidth="1.5" markerEnd="url(#ah)"/>
                <text x={(x1+x2)/2} y={(y1+y2)/2 - 6} fill="#f9a8d4" fontFamily="VT323" fontSize="14" textAnchor="middle">{e.l}</text>
              </g>
            );
          })}
          {N.map(n=>(
            <g key={n.id}>
              {n.accept && <rect x={n.x-r-3} y={n.y-r-3} width={(r+3)*2} height={(r+3)*2} fill="none" stroke="#4ade80" strokeDasharray="3 2"/>}
              <rect x={n.x-r} y={n.y-r} width={r*2} height={r*2}
                    fill={n.id===0?"#0a2530":"#1a0f24"}
                    stroke={n.id===0?"#22d3ee":(n.accept?"#4ade80":"#c026d3")}
                    strokeWidth="2"/>
              <text x={n.x} y={n.y+5} fontFamily="VT323" fontSize="16" fill="#e8d6f0" textAnchor="middle">{n.label}</text>
            </g>
          ))}
        </svg>
      </div>
      <div className="dim" style={{fontSize:16, marginTop:8}}>
        ▍ Subconjuntos calculados desde NFA · {' '}
        <span style={{color:"var(--cyan)"}}>q0</span> inicial ·{' '}
        <span style={{color:"var(--green)"}}>q1..q4</span> de aceptación
      </div>
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

function ParseConsole({ stepIdx, setStep, onParse }){
  const [input, setInput] = useState("c c d c d");
  const cur = D.TRACE[stepIdx] || D.TRACE[0];
  const isAccepted = cur && cur.action === "acc";

  return (
    <div className="panel">
      <div className="panel-title" style={{position:"absolute", top:-14, left:14}}>
        <span className="swatch"/>PARSE CONSOLE
        <span style={{color:"var(--tx-mute)", marginLeft:10}}>LR(1) · step {stepIdx+1}/{D.TRACE.length}</span>
      </div>
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
      <div className="trace-wrap">
        <div className="trace-col left">
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
        <div className="trace-col">
          <h4>▍ ESTADO ACTUAL · paso {stepIdx+1}</h4>
          <StackView step={cur}/>
          {isAccepted &&
            <div className="accept-banner">
              <span>✓</span><span>CADENA ACEPTADA</span>
              <span className="dim" style={{fontFamily:"VT323", fontSize:14}}>· 12 pasos · 0 errores</span>
            </div>
          }
        </div>
      </div>
    </div>
  );
}

/* ============================== Right results panel ============================== */

function ResultsPanel({ stepIdx, activeTab, setActiveTab, activeState, setActiveState }){
  const TABS = [
    {id:"grammar",   label:"GRAMÁTICA"},
    {id:"first",     label:"FIRST"},
    {id:"follow",    label:"FOLLOW"},
    {id:"states",    label:"ESTADOS", badge: D.STATES.length},
    {id:"action",    label:"ACTION/GOTO"},
    {id:"tokens",    label:"TOKENS", badge: D.TOKENS.length},
    {id:"dfa",       label:"DFA"},
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
          {activeTab==="action"   && <ActionGotoTable stepIdx={stepIdx}/>}
          {activeTab==="tokens"   && <TokensView/>}
          {activeTab==="dfa"      && <DfaGraph/>}
          {activeTab==="gen"      && <GeneratedCode/>}
          {activeTab==="problems" && <ProblemsList/>}
        </div>
      </div>
    </>
  );
}

/* ============================== Header ============================== */

function Header({ activeFile, setFile, onRun, loading }){
  const tabs = [
    { id:"yal",  label:"lexer.yal",   dirty:true },
    { id:"yalp", label:"parser.yalp", dirty:false },
    { id:"test", label:"input.txt",   dirty:false },
  ];
  return (
    <header data-screen-label="IDE">
      <div className="brand">
        <div className="logo"/>
        <div className="name">SYNTRA</div>
      </div>
      <div className="menu">
        <span>FILE</span><span>EDIT</span><span>VIEW</span><span className="active">BUILD</span><span>RUN</span><span>?</span>
      </div>
      <div className="filetabs">
        {tabs.map(t=>
          <div key={t.id}
               className={"ftab " + (activeFile===t.id?"active":"") + (t.dirty?" dirty":"")}
               onClick={()=>setFile(t.id)}>
            <span className="dot"/>
            <span>{t.label}</span>
            <span className="x">×</span>
          </div>
        )}
      </div>
      <div className="actions">
        <button className="runbtn" onClick={onRun} disabled={loading} style={{opacity:loading?.5:1}}>
          {loading ? "..." : <><span className="play"/>RUN</>}
        </button>
        <button className="runbtn stepbtn" onClick={onRun} disabled={loading}><span className="play"/>STEP</button>
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

function StatusBar({ activeFile, stepIdx }){
  const f = D.FILES[activeFile];
  return (
    <div id="status">
      <div className="sg"><span className="sw" style={{background:"var(--pink)"}}/><span className="pink">SYNTRA</span></div>
      <div className="sg"><span className="sw" style={{background:"var(--yellow)"}}/>3</div>
      <div className="sg"><span className="sw" style={{background:"var(--coral)"}}/>1</div>
      <div className="sg dim">main ●</div>
      <div className="sg dim">build · ok</div>
      <div className="sg"><span className="grm">LR(1)</span> · sin conflictos</div>
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
  const [activeFile, setFile]   = useState("yalp");
  const [activeTab,  setTab]    = useState("action");
  const [activeState, setState] = useState(3);
  const [stepIdx,    setStep]   = useState(3);
  const [loading,    setLoading]= useState(false);
  const [,           bump]      = useState(0); // fuerza re-render cuando D muta

  const rerender = () => bump(n => n + 1);

  // sincroniza el estado resaltado en la tabla cuando cambia el paso
  useEffect(()=>{
    const cur = D.TRACE[stepIdx];
    if (!cur) return;
    const top = cur.stack[cur.stack.length - 1];
    if (typeof top === "number") setState(top);
  }, [stepIdx]);

  // ── RUN: compila la gramática y actualiza TODA la data ──────────────────────
  const handleRun = async () => {
    setLoading(true);
    try {
      const res = await fetch(`${API}/api/parser/compile`, {
        method:  "POST",
        headers: { "Content-Type": "application/json" },
        body:    JSON.stringify({ content: D.FILES.yalp.rawContent }),
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
      });
      setStep(0);
      rerender();
    } catch(e) {
      console.error("API /compile:", e);
      // Agrega el error a PROBLEMS para que aparezca en el tab
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
        body:    JSON.stringify({ content: D.FILES.yalp.rawContent, tokens }),
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
      <Header activeFile={activeFile} setFile={setFile} onRun={handleRun} loading={loading}/>

      <div id="files" className="panel" data-screen-label="files">
        <div className="panel-title">
          <span className="swatch"/>EXPLORER
        </div>
        <FileTree active={activeFile} onPick={setFile}/>
      </div>

      <div id="editor-wrap" data-screen-label="editor">
        <Editor file={activeFile}/>
      </div>

      <div id="results" data-screen-label="results">
        <ResultsPanel
          stepIdx={stepIdx}
          activeTab={activeTab}
          setActiveTab={setTab}
          activeState={activeState}
          setActiveState={setState}/>
      </div>

      <div id="console-area" data-screen-label="console">
        <ParseConsole stepIdx={stepIdx} setStep={setStep} onParse={handleParse}/>
      </div>

      <StatusBar activeFile={activeFile} stepIdx={stepIdx}/>
    </div>
  );
}

ReactDOM.createRoot(document.getElementById("root")).render(<App/>);
