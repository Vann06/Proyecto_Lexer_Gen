//! Asignación y reciclaje de temporales.
//!
//! El libro (§6.2) usa `newtemp()` como un contador que nunca repite, y deja
//! el reciclaje para después. Acá el reciclaje es parte del contrato: un
//! temporal se devuelve a la reserva apenas su valor se consume, así una
//! expresión como `a + b * c - d` usa dos temporales en vez de tres.
//!
//! El algoritmo es una pila de temporales libres ordenada:
//!
//! - `new_temp()` devuelve el número libre MÁS BAJO, o uno nuevo si no hay
//!   ninguno libre. Reusar siempre el más bajo mantiene los números chicos y
//!   hace que la cantidad máxima viva sea fácil de leer en el TAC.
//! - `release(t)` lo devuelve a la reserva. Lo llama el generador cuando una
//!   instrucción CONSUME un operando que es temporal: ese valor no se vuelve
//!   a leer, así que su ranura queda libre para el siguiente.
//!
//! Solo se reciclan temporales, nunca variables: una variable vive en su
//! propia ranura del marco y puede volver a leerse.
//!
//! Hay un `TempPool` por función. Su `max_live` es cuántas ranuras de
//! temporal necesita el registro de activación (`TacFunction::max_temps`).

use std::collections::BTreeSet;

use super::tac::Operand;

#[derive(Debug, Default, Clone)]
pub struct TempPool {
    /// Temporales devueltos y disponibles para reusar.
    free: BTreeSet<usize>,
    /// El próximo número nunca usado.
    next: usize,
    /// Cuántos están en uso ahora mismo.
    live: usize,
    /// El máximo de `live` alcanzado: las ranuras que necesita el marco.
    max_live: usize,
    /// Cuántas veces se pidió un temporal (sin reciclaje serían todos
    /// distintos): sirve para mostrar cuánto ahorra el reciclaje.
    requested: usize,
}

impl TempPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Un temporal libre: el de número más bajo disponible.
    pub fn new_temp(&mut self) -> usize {
        self.requested += 1;
        let t = match self.free.pop_first() {
            Some(t) => t,
            None => {
                let t = self.next;
                self.next += 1;
                t
            }
        };
        self.live += 1;
        self.max_live = self.max_live.max(self.live);
        t
    }

    /// `new_temp` envuelto como operando.
    pub fn new_operand(&mut self) -> Operand {
        Operand::Temp(self.new_temp())
    }

    /// Devuelve `t` a la reserva. Liberar dos veces el mismo temporal es un
    /// error del generador, no del programa: se detecta en debug.
    pub fn release(&mut self, t: usize) {
        debug_assert!(t < self.next, "t{t} nunca se repartió");
        let inserted = self.free.insert(t);
        debug_assert!(inserted, "t{t} se liberó dos veces");
        if inserted {
            self.live -= 1;
        }
    }

    /// Libera el temporal que ocupa `operand`, si ocupa uno. Es lo que llama
    /// el generador al consumir un operando: una variable o una constante no
    /// liberan nada.
    pub fn release_operand(&mut self, operand: &Operand) {
        if let Some(t) = operand.temp() {
            self.release(t);
        }
    }

    pub fn live(&self) -> usize {
        self.live
    }

    pub fn max_live(&self) -> usize {
        self.max_live
    }

    pub fn requested(&self) -> usize {
        self.requested
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reusa_el_temporal_libre_mas_bajo() {
        let mut pool = TempPool::new();
        let t0 = pool.new_temp();
        let t1 = pool.new_temp();
        let t2 = pool.new_temp();
        assert_eq!((t0, t1, t2), (0, 1, 2));
        pool.release(t2);
        pool.release(t0);
        assert_eq!(pool.new_temp(), 0, "el más bajo primero");
        assert_eq!(pool.new_temp(), 2);
        assert_eq!(pool.new_temp(), 3, "sin libres, uno nuevo");
    }

    #[test]
    fn el_maximo_vivo_es_lo_que_ocupa_el_marco() {
        // a + b * c - d:
        //   t0 = b * c        (vivos: t0)
        //   t1 = a + t0       (consume t0 → libre; vivos: t1)
        //   t0 = t1 - d       (consume t1; reusa t0)
        let mut pool = TempPool::new();
        let t0 = pool.new_temp();
        pool.release(t0);
        let t1 = pool.new_temp();
        assert_eq!(t1, 0, "al liberar t0 antes de pedir, se reusa");

        let mut pool = TempPool::new();
        let a = pool.new_temp();
        let b = pool.new_temp();
        pool.release(a);
        pool.release(b);
        let c = pool.new_temp();
        assert_eq!(c, 0);
        assert_eq!(pool.max_live(), 2);
        assert_eq!(pool.requested(), 3);
        assert_eq!(pool.live(), 1);
    }

    #[test]
    fn liberar_una_variable_o_una_constante_no_hace_nada() {
        let mut pool = TempPool::new();
        let t = pool.new_operand();
        pool.release_operand(&Operand::local("x", -8));
        pool.release_operand(&Operand::Int(3));
        assert_eq!(pool.live(), 1);
        pool.release_operand(&t);
        assert_eq!(pool.live(), 0);
    }
}
