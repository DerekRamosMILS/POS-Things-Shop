//! Aritmética de dinero en centavos enteros.
//!
//! Los importes se guardan en la base como `REAL` porque cambiar el esquema de
//! doce tablas con datos en producción es un riesgo mayor que el problema que
//! resuelve. Lo que sí se elimina aquí es la causa real de los descuadres: el
//! **acumular** en punto flotante. Todo cálculo entra a centavos enteros, opera
//! sin error posible y vuelve a pesos una sola vez, al guardar.
//!
//! Con `f64` puro, sumar 0.1 diez veces no da 1.0. Con `Cents` sí.

use std::iter::Sum;
use std::ops::{Add, Mul, Sub};

/// Un importe monetario expresado en centavos.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Cents(pub i64);

impl Cents {
    pub const ZERO: Cents = Cents(0);

    /// Convierte pesos a centavos redondeando al centavo más cercano.
    ///
    /// El medio centavo se redondea hacia arriba en magnitud (`round` de Rust),
    /// que es lo que espera cualquiera mirando un ticket.
    pub fn from_pesos(value: f64) -> Cents {
        if !value.is_finite() {
            return Cents::ZERO;
        }
        Cents((value * 100.0).round() as i64)
    }

    pub fn to_pesos(self) -> f64 {
        self.0 as f64 / 100.0
    }

    pub fn is_positive(self) -> bool {
        self.0 > 0
    }

    /// Nunca deja pasar un importe negativo: en una venta no existen.
    pub fn clamp_non_negative(self) -> Cents {
        Cents(self.0.max(0))
    }

    pub fn min(self, other: Cents) -> Cents {
        Cents(self.0.min(other.0))
    }

    pub fn abs(self) -> Cents {
        Cents(self.0.abs())
    }

    /// Multiplica por una cantidad entera de unidades.
    pub fn times(self, quantity: i64) -> Cents {
        Cents(self.0.saturating_mul(quantity))
    }

    /// Aplica un porcentaje redondeando al centavo.
    ///
    /// Se opera sobre enteros de 128 bits para que un importe grande por un
    /// porcentaje no se desborde antes de dividir.
    pub fn percent(self, percent: f64) -> Cents {
        if !percent.is_finite() || percent <= 0.0 {
            return Cents::ZERO;
        }
        let basis = (percent * 100.0).round() as i128; // centésimas de porcentaje
        let product = self.0 as i128 * basis;
        // Redondeo al centavo más cercano, no truncamiento.
        let rounded = (product + 5_000 * product.signum()) / 10_000;
        Cents(rounded as i64)
    }
}

impl Cents {
    /// Reparte proporcionalmente: `self * numerador / denominador`, redondeado
    /// al centavo. Se opera en 128 bits para que el producto intermedio no se
    /// desborde antes de dividir.
    ///
    /// Sirve para repartir el total realmente cobrado entre las partidas de una
    /// venta, que es como se calcula cuánto devolver.
    pub fn prorate(self, numerator: Cents, denominator: Cents) -> Cents {
        if denominator.0 == 0 {
            return Cents::ZERO;
        }
        let product = self.0 as i128 * numerator.0 as i128;
        let den = denominator.0 as i128;
        let half = den / 2;
        let rounded = if (product < 0) != (den < 0) {
            (product - half) / den
        } else {
            (product + half) / den
        };
        Cents(rounded as i64)
    }
}

impl Add for Cents {
    type Output = Cents;
    fn add(self, other: Cents) -> Cents {
        Cents(self.0.saturating_add(other.0))
    }
}

impl Sub for Cents {
    type Output = Cents;
    fn sub(self, other: Cents) -> Cents {
        Cents(self.0.saturating_sub(other.0))
    }
}

impl Mul<i64> for Cents {
    type Output = Cents;
    fn mul(self, quantity: i64) -> Cents {
        self.times(quantity)
    }
}

impl Sum for Cents {
    fn sum<I: Iterator<Item = Cents>>(iter: I) -> Cents {
        iter.fold(Cents::ZERO, |acc, c| acc + c)
    }
}

impl std::fmt::Display for Cents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2}", self.to_pesos())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pesos_round_trip_without_drift() {
        for value in [0.0, 0.01, 1.5, 249.0, 1234.56, 99999.99] {
            assert_eq!(Cents::from_pesos(value).to_pesos(), value, "falló con {}", value);
        }
    }

    #[test]
    fn accumulating_cents_is_exact_where_floats_are_not() {
        // El caso clásico: en f64, sumar 0.1 diez veces no da 1.0.
        let float_sum: f64 = (0..10).map(|_| 0.1).sum();
        assert_ne!(float_sum, 1.0);

        let cents: Cents = (0..10).map(|_| Cents::from_pesos(0.1)).sum();
        assert_eq!(cents, Cents::from_pesos(1.0));
        assert_eq!(cents.to_pesos(), 1.0);
    }

    #[test]
    fn a_long_ticket_never_drifts_by_a_cent() {
        // 300 partidas de $19.99 se suman exacto.
        let total: Cents = (0..300).map(|_| Cents::from_pesos(19.99)).sum();
        assert_eq!(total, Cents(599_700));
        assert_eq!(total.to_pesos(), 5997.0);
    }

    #[test]
    fn half_a_cent_rounds_up() {
        assert_eq!(Cents::from_pesos(10.005), Cents(1001));
        assert_eq!(Cents::from_pesos(0.125), Cents(13));
    }

    #[test]
    fn non_finite_amounts_become_zero_instead_of_garbage() {
        assert_eq!(Cents::from_pesos(f64::NAN), Cents::ZERO);
        assert_eq!(Cents::from_pesos(f64::INFINITY), Cents::ZERO);
    }

    #[test]
    fn percentages_round_to_the_nearest_cent() {
        assert_eq!(Cents::from_pesos(100.0).percent(16.0), Cents::from_pesos(16.0));
        assert_eq!(Cents::from_pesos(249.0).percent(10.0), Cents::from_pesos(24.90));
        // 33.33% de $10.00 = $3.333 -> $3.33
        assert_eq!(Cents::from_pesos(10.0).percent(33.33), Cents::from_pesos(3.33));
    }

    #[test]
    fn a_percentage_of_a_large_amount_does_not_overflow() {
        let big = Cents::from_pesos(9_000_000.0);
        assert_eq!(big.percent(50.0), Cents::from_pesos(4_500_000.0));
    }

    #[test]
    fn an_invalid_percentage_grants_nothing() {
        let amount = Cents::from_pesos(100.0);
        assert_eq!(amount.percent(-5.0), Cents::ZERO);
        assert_eq!(amount.percent(f64::NAN), Cents::ZERO);
        assert_eq!(amount.percent(0.0), Cents::ZERO);
    }

    #[test]
    fn quantities_multiply_exactly() {
        assert_eq!(Cents::from_pesos(19.99).times(7), Cents::from_pesos(139.93));
        assert_eq!(Cents::from_pesos(0.01).times(1000), Cents::from_pesos(10.0));
    }

    #[test]
    fn negative_amounts_can_be_clamped_away() {
        assert_eq!((Cents::from_pesos(10.0) - Cents::from_pesos(25.0)).clamp_non_negative(), Cents::ZERO);
        assert_eq!(Cents::from_pesos(10.0).clamp_non_negative(), Cents::from_pesos(10.0));
    }

    #[test]
    fn prorating_splits_a_total_across_parts() {
        let total = Cents::from_pesos(100.0);
        // Una partida que vale 30 de 120 se lleva el 25% del total cobrado.
        assert_eq!(total.prorate(Cents::from_pesos(30.0), Cents::from_pesos(120.0)),
                   Cents::from_pesos(25.0));
    }

    #[test]
    fn prorating_the_whole_gives_back_the_whole() {
        let total = Cents::from_pesos(116.0);
        let base = Cents::from_pesos(100.0);
        assert_eq!(total.prorate(base, base), total);
    }

    #[test]
    fn prorating_rounds_to_the_nearest_cent() {
        // 100 / 3 = 33.333... -> 33.33
        let total = Cents::from_pesos(100.0);
        assert_eq!(total.prorate(Cents(1), Cents(3)), Cents::from_pesos(33.33));
    }

    #[test]
    fn prorating_by_zero_yields_zero_instead_of_dividing() {
        assert_eq!(Cents::from_pesos(100.0).prorate(Cents::from_pesos(10.0), Cents::ZERO), Cents::ZERO);
    }

    #[test]
    fn prorating_a_large_total_does_not_overflow() {
        let total = Cents::from_pesos(5_000_000.0);
        assert_eq!(total.prorate(Cents::from_pesos(1_000_000.0), Cents::from_pesos(2_000_000.0)),
                   Cents::from_pesos(2_500_000.0));
    }

    #[test]
    fn amounts_print_with_two_decimals() {
        assert_eq!(Cents::from_pesos(249.0).to_string(), "249.00");
        assert_eq!(Cents(5).to_string(), "0.05");
    }
}
