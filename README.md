# Proyecto Lexer Generator

Compilador educativo escrito en Rust que implementa el flujo completo desde
una especificación léxica y sintáctica hasta código intermedio y código objetivo
MIPS. El proyecto incluye una CLI, una API HTTP y un IDE web para explorar las
distintas fases de compilación.

El lenguaje principal de las pruebas de extremo a extremo es **CompiScript**.
También se incluyen gramáticas y fuentes de ejemplo para expresiones, C,
Python y Pascalito.

## Demostración

[Ver el video de demostración: generación y ejecución de código ensamblador](https://uvggt-my.sharepoint.com/:v:/r/personal/cas23201_uvg_edu_gt/_layouts/15/stream.aspx?id=%2Fpersonal%2Fcas23201_uvg_edu_gt%2FDocuments%2F2026_S2%2FCompis%2FViedo_Codigo_Asm.mp4&share=cQo2uBzm0AnqTYr2mVBe54PREgUCESTBC_BIG_vT4RxA3aHrNg&nav=eyJyZWZlcnJhbEluZm8iOnsicmVmZXJyYWxBcHAiOiJPbmVEcml2ZUZvckJ1c2luZXNzIiwicmVmZXJyYWxBcHBQbGF0Zm9ybSI6IldlYiIsInJlZmVycmFsTW9kZSI6InZpZXciLCJyZWZlcnJhbFZpZXciOiJNeUZpbGVzTGlua0NvcHkifX0)

## Flujo del compilador

```text
.yal + .yalp/.yapar + .cps
             │
             ▼
  análisis léxico (AFN → AFD → AFD mínimo)
             │
             ▼
  análisis sintáctico (LL(1), SLR, LR(1), LALR)
             │
             ▼
  análisis semántico y tabla de símbolos
             │
             ▼
  código de tres direcciones (TAC) + VM
             │
             ▼
  ensamblador MIPS para MARS 4.5
```

### Componentes principales

- **Lexer YALex:** definiciones regulares, construcción de AFN por Thompson,
  conversión a AFD, minimización, *maximal munch*, simulación y generación de
  un lexer standalone en Rust.
- **Parser YAPar:** cálculo de `FIRST`/`FOLLOW`, tablas LL(1), SLR, LR(1) y
  LALR, detección de conflictos y árbol de derivación exportable a DOT.
- **Semántica:** ámbitos anidados, tipos, funciones, cierres, clases,
  herencia, colecciones, flujo de control, duplicados, código muerto y
  diagnósticos con línea y columna.
- **Código intermedio:** TAC organizado por funciones, registros de
  activación, enlaces de control y acceso, vtables, listas, manejo de errores
  y una máquina virtual usada como oráculo de ejecución.
- **Código objetivo MIPS:** secciones `.data`/`.text`, llamadas directas y
  virtuales, prólogos y epílogos, runtime, `try`/`catch`, análisis de vida,
  asignación `getReg()` sobre registros `$t`/`$s` y derrames a pila.
- **Herramientas de uso:** CLI `cpsc`, API con Axum, IDE web y despliegue con
  Docker Compose.

## Estado de la entrega MIPS

Las fases 0–4 de [PLAN_MIPS.md](PLAN_MIPS.md) están implementadas en
`src/codigo_objetivo/`:

| Parte | Implementación |
|---|---|
| Emisión del programa | `asm.rs`, `data.rs`, `names.rs`, `mod.rs` |
| Expresiones, control, listas y campos | `instr.rs` |
| Llamadas, retorno y registros de activación | `calls.rs` |
| Rutinas de apoyo y excepciones | `runtime.asm`, `runtime.rs` |
| Vida de temporales y `getReg()` | `liveness.rs`, `regs.rs` |

El ensamblador producido está diseñado para **MARS 4.5** y las pruebas de las
fases 0–4 lo ejecutan cuando Java y `tools/Mars4_5.jar` están disponibles. El
JAR no está versionado en este repositorio.

> **Estado actual de `main`:** la integración final de la Fase 5 descrita en
> `PLAN_MIPS.md` —`sim.rs`, `cpsc --mips/--sim` y las vistas MIPS de la API y
> el IDE— todavía no forma parte del código. La generación MIPS se consume hoy
> mediante `codigo_objetivo::generate` y se valida desde las pruebas.

### Probar la generación con MARS

Requisitos:

- Rust estable y Cargo.
- Java 8 o posterior.
- MARS 4.5 guardado localmente como `tools/Mars4_5.jar`.

```powershell
# Ejecuta únicamente las pruebas de las fases MIPS.
cargo test --test mips_fase0_tests
cargo test --test mips_fase1_tests
cargo test --test mips_fase2_tests
cargo test --test mips_fase3_tests
cargo test --test mips_fase4_tests

# Obliga a que las pruebas fallen si Java o el JAR de MARS no están presentes.
$env:MIPS_REQUIRED = "1"
cargo test --test mips_fase3_tests
```

Sin `MIPS_REQUIRED=1`, las comprobaciones puramente estructurales siempre se
ejecutan y los casos que necesitan MARS se omiten con un aviso cuando falta el
simulador.

Para abrir y ejecutar manualmente un `.asm` ya generado:

```powershell
java -Dfile.encoding=ISO-8859-1 -jar tools/Mars4_5.jar nc sm se2 ae3 output/programa.asm
```

Las opciones `nc sm se2 ae3` evitan la interfaz gráfica, ejecutan el programa
y hacen que MARS propague el código de salida del programa simulado.

## Inicio rápido

### Compilar y ejecutar pruebas

```bash
cargo build
cargo test
```

### Compilar CompiScript a TAC

La CLI toma por defecto `workspace/compiscript.yal` y
`workspace/compiscript.yalp`:

```bash
# Muestra TAC y la tabla de símbolos extendida.
cargo run --bin cpsc -- workspace/rubrica.cps

# Además ejecuta el TAC en la máquina virtual.
cargo run --bin cpsc -- workspace/rubrica.cps --run

# Usa la tabla SLR en lugar de LALR.
cargo run --bin cpsc -- workspace/rubrica.cps --slr
```

### Generar un lexer standalone

```bash
cargo run -- examples/lexer/ejemplo_c.yal examples/source/input_test2.txt
```

El lexer generado queda en `generated/src/lexer.rs`. El directorio
`generated/` es un crate independiente que puede compilarse y ejecutarse con
`cargo run`.

## IDE y API

La forma más sencilla de levantar ambos servicios es Docker Compose:

```bash
docker compose up --build
```

| Servicio | Dirección |
|---|---|
| IDE web | [http://localhost:4000](http://localhost:4000) |
| API Rust | [http://localhost:8080](http://localhost:8080) |
| Health check | [http://localhost:8080/health](http://localhost:8080/health) |

También se pueden iniciar por separado:

```bash
# Terminal 1
cargo run --bin api

# Terminal 2
python3 -m http.server 5500 --directory frontend/IDE-lite
```

El IDE permite editar archivos del `workspace`, analizar una fuente y revisar
tokens, traza del parser, árbol sintáctico anotado, problemas, tablas de
símbolos, tipos, layouts y TAC. Los endpoints y formatos completos están en
[GUIA_USO.md](GUIA_USO.md) y [API_REFERENCE.md](API_REFERENCE.md).

## Estructura del repositorio

```text
Proyecto_Lexer_Gen/
├── src/
│   ├── lexico/              # especificaciones, regex, autómatas y runtime
│   ├── sintactico/          # gramáticas, autómatas LR/LL y tablas
│   ├── semantico/           # símbolos, tipos, ámbitos y layouts
│   ├── intermedio/          # generación TAC y máquina virtual
│   ├── codigo_objetivo/     # generación de ensamblador MIPS
│   ├── api/                 # lógica compartida por HTTP y pruebas
│   └── bin/
│       ├── api.rs           # servidor Axum
│       └── cpsc.rs          # compilador de CompiScript por consola
├── frontend/IDE-lite/       # IDE web en React
├── examples/                # lexers, gramáticas, fuentes y casos completos
├── workspace/               # archivos editables y casos de la rúbrica
├── tests/                   # pruebas unitarias y de integración
├── generated/               # crate del lexer generado
├── output/                  # salidas locales no versionadas
├── PLAN_MIPS.md             # plan y contrato de la entrega MIPS
├── CODIGO_INTERMEDIO.md     # diseño y contrato del TAC
├── ARQUITECTURA.md          # arquitectura general
└── GUIA_USO.md              # guía del IDE y la API
```

## Documentación

- [ARQUITECTURA.md](ARQUITECTURA.md): diseño del compilador y ubicación de
  los entregables.
- [CODIGO_INTERMEDIO.md](CODIGO_INTERMEDIO.md): instrucciones TAC, VM y
  contrato TAC → MIPS.
- [PLAN_MIPS.md](PLAN_MIPS.md): fases, decisiones y pruebas del backend MIPS.
- [GUIA_USO.md](GUIA_USO.md): uso del IDE, API y Docker.
- [API_REFERENCE.md](API_REFERENCE.md): referencia de funciones y endpoints.
- [ORGANIZACION.md](ORGANIZACION.md): organización histórica de las fases.

## Pruebas

El proyecto contiene pruebas unitarias dentro de los módulos y pruebas de
integración en `tests/`, que cubren el lexer, los parsers, la semántica, TAC,
la VM, la API y las fases MIPS.

```bash
# Batería completa
cargo test

# Algunos grupos útiles durante el desarrollo
cargo test --test semantic_analysis_tests
cargo test --test tac_battery_tests
cargo test --test api_pipeline_tests
```
