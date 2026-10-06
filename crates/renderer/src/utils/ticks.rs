//! Preismarken: glatte Schritte, einheitliche Nachkommastellen, Grenzen auf den Schritt.
//!
//! Die Achse beschriftet in 1-2-5-Schritten (`1·10ᵏ`, `2·10ᵏ`, `5·10ᵏ`), das Gitter
//! liegt auf denselben Marken, und alle Beschriftungen einer Achse tragen gleich
//! viele Nachkommastellen. Vorher standen sieben gleich verteilte Werte mit festen
//! zwei Stellen am Rand — für Devisen zu grob, und das Gitter lief neben den
//! Beschriftungen her.

/// Wie viele Intervalle die Achse anstrebt.
pub const TARGET_INTERVALS: f64 = 5.0;

/// Eine Preisachse: Marken, Schritt und Nachkommastellen.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceTicks {
    pub step: f64,
    pub values: Vec<f64>,
    pub decimals: usize,
}

/// Der glatte Schritt (1, 2 oder 5 mal Zehnerpotenz) für `range / target`.
pub fn nice_step(range: f64, target_intervals: f64) -> f64 {
    if !(range.is_finite() && range > 0.0) || target_intervals <= 0.0 {
        return 1.0;
    }
    let raw = range / target_intervals;
    let magnitude = 10f64.powf(raw.log10().floor());
    let fraction = raw / magnitude;
    // Übliche Schwellen (1,5 / 3 / 7): rundet zum nächsten glatten Wert statt immer
    // nach oben. Mit „bis 5 → 5, darüber 10“ ergäbe eine Spanne von 0,033 nur drei
    // Marken, weil 5,5 auf 10 springt.
    let nice = if fraction <= 1.5 {
        1.0
    } else if fraction <= 3.0 {
        2.0
    } else if fraction <= 7.0 {
        5.0
    } else {
        10.0
    };
    nice * magnitude
}

/// Nachkommastellen, die ein glatter Schritt braucht (`0.05` → 2, `20` → 0).
pub fn step_decimals(step: f64) -> usize {
    if !(step.is_finite() && step > 0.0) {
        return 0;
    }
    let exponent = step.log10().floor() as i32;
    (-exponent).clamp(0, 12) as usize
}

/// Marken innerhalb von `[min, max]`, auf Vielfachen des glatten Schritts.
pub fn price_ticks(min: f64, max: f64, target_intervals: f64) -> PriceTicks {
    let step = nice_step(max - min, target_intervals);
    let decimals = step_decimals(step);
    let mut values = Vec::new();
    if step > 0.0 && max > min {
        let first = (min / step).ceil();
        let last = (max / step).floor();
        let count = (last - first) as i64;
        // Obergrenze gegen Ausreißer (extreme Spannen, NaN-nahe Werte).
        if (0..=200).contains(&count) {
            for i in 0..=count {
                // Aus dem Index multipliziert statt aufsummiert: kein Auseinanderdriften.
                values.push((first + i as f64) * step);
            }
        }
    }
    PriceTicks {
        step,
        values,
        decimals,
    }
}

/// Rundet beide Enden nach außen auf den glatten Schritt, damit die oberste und
/// unterste Gitterlinie am Rand der Zeichenfläche liegen.
pub fn nice_bounds(min: f64, max: f64, target_intervals: f64) -> (f64, f64) {
    if !(min.is_finite() && max.is_finite()) || max <= min {
        return (min, max);
    }
    let step = nice_step(max - min, target_intervals);
    ((min / step).floor() * step, (max / step).ceil() * step)
}

/// Formatiert einen Preis mit fester Stellenzahl und echtem Minuszeichen (U+2212).
pub fn format_price(value: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, value.abs());
    // `-0.00` ist keine Beschriftung.
    if value < 0.0 && text.chars().any(|c| c.is_ascii_digit() && c != '0') {
        format!("\u{2212}{text}")
    } else {
        text
    }
}

/// Welche Marken beschriftet werden, wenn sie näher als `min_gap_px` beieinander
/// liegen: jede n-te. Das Gitter bleibt an allen Marken.
pub fn label_stride(tick_gap_px: f64, min_gap_px: f64) -> usize {
    if tick_gap_px <= 0.0 || tick_gap_px >= min_gap_px {
        1
    } else {
        (min_gap_px / tick_gap_px).ceil() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn steps_are_one_two_or_five_times_a_power_of_ten() {
        assert_eq!(nice_step(100.0, 5.0), 20.0);
        assert_eq!(nice_step(10.0, 5.0), 2.0);
        assert_eq!(nice_step(7.0, 5.0), 1.0);
        assert_eq!(nice_step(0.033, 6.0), 0.005);
        assert_eq!(nice_step(0.012, 5.0), 0.002);
        assert_eq!(nice_step(480.0, 5.0), 100.0);
    }

    #[test]
    fn degenerate_ranges_do_not_panic() {
        assert_eq!(nice_step(0.0, 5.0), 1.0);
        assert_eq!(nice_step(f64::NAN, 5.0), 1.0);
        assert!(price_ticks(5.0, 5.0, 5.0).values.is_empty());
    }

    #[test]
    fn decimals_follow_the_step() {
        assert_eq!(step_decimals(20.0), 0);
        assert_eq!(step_decimals(1.0), 0);
        assert_eq!(step_decimals(0.5), 1);
        assert_eq!(step_decimals(0.05), 2);
        assert_eq!(step_decimals(0.0005), 4);
    }

    #[test]
    fn ticks_lie_on_multiples_of_the_step_inside_the_range() {
        let ticks = price_ticks(1.0834, 1.0951, 5.0);
        assert_eq!(ticks.step, 0.002);
        assert_eq!(ticks.decimals, 3);
        assert!(ticks.values.iter().all(|v| *v >= 1.0834 && *v <= 1.0951));
        assert_eq!(format_price(ticks.values[0], ticks.decimals), "1.084");
        assert_eq!(ticks.values.len(), 6);
    }

    #[test]
    fn ticks_are_computed_from_the_index_and_do_not_drift() {
        let ticks = price_ticks(0.0, 1.0, 10.0);
        assert_eq!(ticks.values.len(), 11);
        assert_eq!(format_price(ticks.values[3], ticks.decimals), "0.3");
    }

    #[test]
    fn bounds_round_outward_onto_the_step() {
        let (low, high) = nice_bounds(1.0834, 1.0951, 5.0);
        assert!(low <= 1.0834 && high >= 1.0951);
        let step = nice_step(1.0951 - 1.0834, 5.0);
        assert!(((low / step).round() * step - low).abs() < 1e-12);
    }

    #[test]
    fn a_negative_value_uses_a_real_minus_sign_and_no_negative_zero() {
        assert_eq!(format_price(-2.5, 1), "\u{2212}2.5");
        assert_eq!(format_price(-0.0004, 2), "0.00");
    }

    #[test]
    fn crowded_labels_are_thinned_not_the_grid() {
        assert_eq!(label_stride(30.0, 14.0), 1);
        assert_eq!(label_stride(7.0, 14.0), 2);
        assert_eq!(label_stride(4.0, 14.0), 4);
    }
}
