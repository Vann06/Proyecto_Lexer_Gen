//! El motor shift-reduce, una sola vez.
//!
//! Antes este bucle estaba escrito CUATRO veces —`LRParser::parse`,
//! `parse_tree`, `parse_recovering_with_pos` y `api::parse_with_trace_lr`—.
//! Los cuatro recorrían la tabla igual; lo único que cambiaba era qué
//! acumulaban al costado: una traza de pasos, un árbol, una lista de errores o
//! un JSON para el IDE. Como eran copias, un arreglo en una no llegaba a las
//! otras: el mensaje de GOTO faltante llegó a estar escrito de dos formas
//! distintas porque alguien mejoró tres copias y no vio la cuarta.
//!
//! Es el mismo patrón que `semantico::visitor`, una capa más abajo: la
//! MECÁNICA (pila de estados, avance de la entrada, modo pánico) vive acá una
//! sola vez, y cada consumidor implementa `ParseObserver` para decidir qué
//! hacer en cada evento. El driver es dueño de la pila de estados; cada
//! observador es dueño de su propia pila (de nodos, de símbolos o de JSON).

use crate::sintactico::gramatica::grammar::Symbol;
use crate::sintactico::runtime::parse_tree::ParseToken;
use crate::sintactico::tablas::{Action, LRTable};

/// Qué debe hacer el driver cuando `ACTION[estado, token]` no existe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// Cortar y devolver el error. Es lo que hacen los tres caminos que no
    /// recuperan.
    Abort,
    /// Entrar en modo pánico: descartar entrada hasta un símbolo de
    /// sincronización, desapilar hasta un estado que lo acepte y seguir.
    Recover,
}

/// Por qué se está llamando a `on_error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCause {
    /// No hay acción para este par (estado, token).
    NoAction,
    /// Se volvió a entrar en pánico en la MISMA posición sin haber consumido
    /// nada: la recuperación anterior cayó en una reducción vacía que no
    /// avanza. El driver va a forzar el avance de un token para romper el
    /// ciclo.
    LoopGuard,
}

/// Por qué terminó el driver sin aceptar.
#[derive(Debug, Clone)]
pub enum DriveError {
    /// Error sintáctico y el observador pidió abortar.
    Syntax { state: usize, ip: usize, token: ParseToken },
    /// `GOTO[top, head]` no existe: la tabla es internamente inconsistente.
    /// Sin cortar acá, el bucle seguiría con la pila ya reducida y sin avanzar.
    MissingGoto { top: usize, head: String, ip: usize, token: ParseToken },
    /// El modo pánico no encontró dónde resincronizar.
    Unrecovered,
    /// La entrada se agotó sin encontrar el `$`. No debería pasar —`$` se
    /// rechaza como nombre de token al parsear la gramática—, pero se indexa
    /// defensivamente en vez de romper el hilo.
    InputExhausted,
}

impl DriveError {
    /// Mensaje único para la tabla inconsistente. Antes estaba escrito a mano
    /// en cada copia, y una de ellas se había quedado sin la coletilla
    /// "tras reducción".
    pub fn missing_goto_msg(top: usize, head: &str) -> String {
        format!("Error interno: GOTO[I{}, {}] no definido tras reducción.", top, head)
    }

    pub fn exhausted_msg() -> String {
        "Error interno: se agotó la entrada de forma inesperada.".to_string()
    }
}

/// Qué hacer en cada evento del recorrido. Todos los métodos tienen una
/// implementación vacía: un observador solo escribe los que le importan.
pub trait ParseObserver {
    /// Antes de consultar la acción, con la foto de la pila de estados y de lo
    /// que queda por consumir. Lo necesita la traza del IDE, que muestra
    /// ambas cosas en cada paso.
    fn before_step(&mut self, _states: &[usize], _remaining: &[ParseToken]) {}

    fn on_shift(&mut self, _next_state: usize, _token: &ParseToken) {}

    /// `goto_state` es el estado al que se salta tras reducir. El observador
    /// desapila `body.len()` elementos de SU propia pila.
    fn on_reduce(&mut self, _head: &str, _body: &[Symbol], _goto_state: usize) {}

    fn on_accept(&mut self) {}

    fn on_error(
        &mut self,
        _cause: ErrorCause,
        _state: usize,
        _ip: usize,
        _token: &ParseToken,
        _table: &LRTable,
    ) -> OnError {
        OnError::Abort
    }

    /// El modo pánico desapiló un estado buscando dónde resincronizar. Quien
    /// mantenga una pila paralela tiene que desapilar en espejo.
    fn on_discard_state(&mut self) {}
}

/// Ejecuta el algoritmo shift-reduce sobre `tokens`.
///
/// `sync` solo se usa si algún `on_error` devuelve `Recover`; los caminos que
/// abortan pueden pasar `&[]`.
pub fn drive<O: ParseObserver + ?Sized>(
    table: &LRTable,
    tokens: Vec<ParseToken>,
    sync: &[&str],
    obs: &mut O,
) -> Result<(), DriveError> {
    let mut state_stack: Vec<usize> = vec![table.start_state];

    let mut input = tokens;
    input.push(ParseToken { kind: "$".to_string(), lexeme: String::new(), line: 0, col: 0 });

    let mut ip = 0usize;
    // Posición donde entramos en pánico por última vez SIN que ningún shift
    // haya consumido nada desde entonces. Ver `ErrorCause::LoopGuard`.
    let mut last_panic_ip: Option<usize> = None;

    loop {
        let s = *state_stack.last().expect("la pila nunca queda vacía: arranca con start_state");

        let current = match input.get(ip) {
            Some(t) => t.clone(),
            None => return Err(DriveError::InputExhausted),
        };

        obs.before_step(&state_stack, &input[ip..]);

        let action = table.action.get(&(s, current.kind.clone())).cloned();
        match action {
            Some(Action::Shift(t)) => {
                obs.on_shift(t, &current);
                state_stack.push(t);
                ip += 1;
                last_panic_ip = None; // progreso real: se consumió un token
            }

            Some(Action::Reduce { head, body }) => {
                for _ in 0..body.len() {
                    state_stack.pop();
                }
                let top = *state_stack.last().expect("no se desapila más allá del estado inicial");
                let next_state = match table.goto.get(&(top, head.clone())).copied() {
                    Some(n) => n,
                    None => {
                        return Err(DriveError::MissingGoto { top, head, ip, token: current })
                    }
                };
                obs.on_reduce(&head, &body, next_state);
                state_stack.push(next_state);
            }

            Some(Action::Accept) => {
                obs.on_accept();
                return Ok(());
            }

            None => {
                // Ciclo detectado: la recuperación anterior no avanzó. Se
                // fuerza el consumo de un token para garantizar progreso.
                if last_panic_ip == Some(ip) {
                    obs.on_error(ErrorCause::LoopGuard, s, ip, &current, table);
                    ip += 1;
                    last_panic_ip = None;
                    if ip >= input.len() {
                        return Err(DriveError::Unrecovered);
                    }
                    continue;
                }
                last_panic_ip = Some(ip);

                if obs.on_error(ErrorCause::NoAction, s, ip, &current, table) == OnError::Abort {
                    return Err(DriveError::Syntax { state: s, ip, token: current });
                }

                // ── Modo pánico ───────────────────────────────────────────
                // 1. Descartar entrada hasta un símbolo de sincronización.
                while ip < input.len()
                    && !sync.contains(&input[ip].kind.as_str())
                    && input[ip].kind != "$"
                {
                    ip += 1;
                }
                if ip >= input.len() || input[ip].kind == "$" {
                    return Err(DriveError::Unrecovered);
                }

                // 2. Desapilar hasta un estado que acepte ese símbolo.
                let sync_kind = input[ip].kind.clone();
                let mut recovered = false;
                while state_stack.len() > 1 {
                    let top = *state_stack.last().expect("len > 1");
                    if table.action.contains_key(&(top, sync_kind.clone())) {
                        recovered = true;
                        break;
                    }
                    state_stack.pop();
                    obs.on_discard_state();
                }
                if !recovered {
                    return Err(DriveError::Unrecovered);
                }
                // 3. Seguir desde el punto de sincronización.
            }
        }
    }
}
