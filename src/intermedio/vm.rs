//! Infraestructura base del intérprete de código intermedio TAC.
//!
//! Por ahora este módulo solo define los valores, errores, resultado y estado
//! mínimo de la máquina. La ejecución de instrucciones, llamadas, heap y
//! manejadores de excepciones se agrega en pasos posteriores de la Fase 3.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use super::tac::TacProgram;

/// Valor que puede almacenar o producir una instrucción TAC.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// Dirección de memoria. Su interpretación concreta se incorpora junto
    /// con el heap y los marcos de activación.
    Pointer(usize),
    /// Dirección simbólica de un dato, por ejemplo `&vt_Perro`.
    /// Es necesaria para `Operand::Addr` y `Instr::AddrOf`.
    Address(String),
    Null,
}

/// Error producido al ejecutar TAC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeError {
    /// La ejecución alcanzó el límite configurado de instrucciones.
    StepLimitExceeded { limit: usize },
    /// Una instrucción todavía no forma parte del subconjunto ejecutable de
    /// la VM. Permite ampliar el intérprete de manera incremental.
    UnsupportedInstruction(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::StepLimitExceeded { limit } => {
                write!(f, "se excedió el límite de {limit} pasos de la VM")
            }
            RuntimeError::UnsupportedInstruction(instruction) => {
                write!(
                    f,
                    "instrucción TAC todavía no soportada por la VM: {instruction}"
                )
            }
        }
    }
}

impl Error for RuntimeError {}

/// Resultado observable de una ejecución terminada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmResult {
    pub output: String,
    pub steps: usize,
}

/// Límite predeterminado para detener programas que no terminan.
pub const DEFAULT_MAX_STEPS: usize = 100_000;

/// Máquina virtual encargada de ejecutar un programa TAC.
pub struct Vm<'a> {
    program: &'a TacProgram,
    /// Temporales de `main`. Cuando se incorporen llamadas, cada marco tendrá
    /// su propio conjunto de temporales.
    temporaries: HashMap<usize, Value>,
    /// Área estática indexada por offset en bytes, igual que `Base::Global`.
    globals: HashMap<isize, Value>,
    output: String,
    steps: usize,
    max_steps: usize,
}

impl<'a> Vm<'a> {
    /// Crea una VM con el límite de pasos predeterminado.
    pub fn new(program: &'a TacProgram) -> Self {
        Self::with_max_steps(program, DEFAULT_MAX_STEPS)
    }

    /// Crea una VM con un límite de pasos explícito.
    pub fn with_max_steps(program: &'a TacProgram, max_steps: usize) -> Self {
        Self {
            program,
            temporaries: HashMap::new(),
            globals: HashMap::new(),
            output: String::new(),
            steps: 0,
            max_steps,
        }
    }

    pub fn program(&self) -> &'a TacProgram {
        self.program
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn steps(&self) -> usize {
        self.steps
    }

    pub fn max_steps(&self) -> usize {
        self.max_steps
    }

    pub fn temporary(&self, index: usize) -> Option<&Value> {
        self.temporaries.get(&index)
    }

    pub fn global(&self, offset: isize) -> Option<&Value> {
        self.globals.get(&offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_representa_literales_punteros_y_direcciones_tac() {
        assert_eq!(Value::Int(7), Value::Int(7));
        assert_eq!(Value::Float(2.5), Value::Float(2.5));
        assert_eq!(Value::Bool(true), Value::Bool(true));
        assert_eq!(Value::Str("hola".into()), Value::Str("hola".into()));
        assert_eq!(Value::Pointer(12), Value::Pointer(12));
        assert_eq!(
            Value::Address("vt_Animal".into()),
            Value::Address("vt_Animal".into())
        );
        assert_eq!(Value::Null, Value::Null);
    }

    #[test]
    fn una_vm_nueva_conserva_el_programa_y_empieza_vacia() {
        let program = TacProgram::default();
        let vm = Vm::with_max_steps(&program, 25);

        assert!(std::ptr::eq(vm.program(), &program));
        assert_eq!(vm.output(), "");
        assert_eq!(vm.steps(), 0);
        assert_eq!(vm.max_steps(), 25);
        assert_eq!(vm.temporary(0), None);
        assert_eq!(vm.global(0), None);
    }

    #[test]
    fn runtime_error_tiene_un_mensaje_legible() {
        let error = RuntimeError::StepLimitExceeded { limit: 10 };
        assert_eq!(error.to_string(), "se excedió el límite de 10 pasos de la VM");
    }
}
