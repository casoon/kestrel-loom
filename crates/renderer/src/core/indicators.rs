//! Indikatoren als laufende Instanzen über der Kerzenserie.
//!
//! Gerechnet wird in `kestrel-chartkit`; dieses Modul hält nur die Instanzen,
//! füttert sie **inkrementell** (`Indicator::on_bar` je neuer Kerze, kein
//! Neuberechnen des Fensters) und bewahrt die Werte für die Darstellung auf.
//!
//! Das ist der Bruch mit `loomcharts` Modell, das jeden Indikator bei jedem Frame
//! über das volle Fenster neu rechnete.

use std::collections::HashMap;

use kestrel_chartkit::indicator::registry::{build_checked, catalog};
use kestrel_chartkit::{Bar, Indicator};

use crate::core::Candle;

/// Eine laufende Indikator-Instanz mit ihren bisherigen Ausgaben.
pub struct IndicatorSeries {
    name: String,
    params: HashMap<String, f64>,
    inner: Box<dyn Indicator>,
    /// Zeit/Wert-Paare, in Kerzenreihenfolge. Nur Bars nach dem Warmup liefern etwas.
    values: Vec<(i64, f64)>,
    /// Wie viele Kerzen bereits eingespeist wurden.
    fed: usize,
    /// Zeitstempel der zuletzt eingespeisten Kerze — erkennt Serienwechsel.
    last_time: Option<i64>,
}

impl IndicatorSeries {
    /// Baut eine Instanz über den Chartkit-Katalog.
    ///
    /// Unbekannte Namen und ungültige Parameter sind Fehler, keine stillen Defaults —
    /// ein Chart, der einen falsch benannten Indikator einfach weglässt, lügt.
    pub fn new(name: &str, params: HashMap<String, f64>) -> Result<Self, String> {
        let inner = build_checked(name, &params).map_err(|e| format!("{e:?}"))?;
        Ok(Self {
            name: name.to_string(),
            params,
            inner,
            values: Vec::new(),
            fed: 0,
            last_time: None,
        })
    }

    /// Namen aller verfügbaren Indikatoren.
    pub fn available() -> Vec<String> {
        catalog().iter().map(|e| e.name.to_string()).collect()
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn params(&self) -> &HashMap<String, f64> {
        &self.params
    }

    pub fn values(&self) -> &[(i64, f64)] {
        &self.values
    }

    /// Verwirft den Zustand und beginnt von vorn.
    pub fn reset(&mut self) {
        self.inner.reset();
        self.values.clear();
        self.fed = 0;
        self.last_time = None;
    }

    /// Speist alle noch nicht verarbeiteten Kerzen ein.
    ///
    /// Wächst die Serie nur am Ende, kostet das genau die neuen Bars. Wurde sie
    /// ersetzt oder gekürzt (Timeframe-Wechsel, neues Instrument, Replay-Sprung),
    /// wird zurückgesetzt und neu aufgebaut — inkrementeller Zustand darf nicht über
    /// einen Serienbruch hinweg weiterlaufen.
    pub fn feed(&mut self, candles: &[Candle]) {
        if candles.len() < self.fed {
            self.reset();
        } else if let (Some(last), true) = (self.last_time, self.fed > 0) {
            if candles.get(self.fed - 1).map(|c| c.time) != Some(last) {
                self.reset();
            }
        }

        for candle in &candles[self.fed..] {
            let bar = Bar::new(
                candle.time,
                candle.o,
                candle.h,
                candle.l,
                candle.c,
                candle.v,
            );
            if let Some(out) = self.inner.on_bar(&bar) {
                if out.value.is_finite() {
                    self.values.push((candle.time, out.value));
                }
            }
            self.last_time = Some(candle.time);
        }
        self.fed = candles.len();
    }
}

impl std::fmt::Debug for IndicatorSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndicatorSeries")
            .field("name", &self.name)
            .field("fed", &self.fed)
            .field("values", &self.values.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CandleGenerator, GeneratorConfig};

    fn candles(n: usize) -> Vec<Candle> {
        CandleGenerator::new(GeneratorConfig::crypto().with_seed(42)).generate(n)
    }

    #[test]
    fn unknown_indicator_is_an_error() {
        assert!(IndicatorSeries::new("gibtsnicht", HashMap::new()).is_err());
    }

    #[test]
    fn catalog_is_not_empty() {
        let names = IndicatorSeries::available();
        assert!(names.len() > 50, "erwartet 90+, gefunden {}", names.len());
        assert!(names.iter().any(|n| n == "rsi"));
    }

    #[test]
    fn rsi_produces_values_after_warmup() {
        let mut series = IndicatorSeries::new("rsi", HashMap::new()).unwrap();
        series.feed(&candles(100));
        assert!(!series.values().is_empty());
        assert!(series
            .values()
            .iter()
            .all(|(_, v)| (0.0..=100.0).contains(v)));
    }

    #[test]
    fn feeding_incrementally_matches_feeding_at_once() {
        let all = candles(120);

        let mut at_once = IndicatorSeries::new("rsi", HashMap::new()).unwrap();
        at_once.feed(&all);

        let mut stepwise = IndicatorSeries::new("rsi", HashMap::new()).unwrap();
        for n in 1..=all.len() {
            stepwise.feed(&all[..n]);
        }

        assert_eq!(at_once.values(), stepwise.values());
    }

    #[test]
    fn a_replaced_series_resets_the_state() {
        let mut series = IndicatorSeries::new("rsi", HashMap::new()).unwrap();
        series.feed(&candles(100));
        let before = series.values().len();

        // Andere Serie gleicher Länge: der Zustand darf nicht weiterlaufen.
        let other = CandleGenerator::new(GeneratorConfig::crypto().with_seed(7)).generate(100);
        series.feed(&other);

        assert_eq!(
            series.values().len(),
            before,
            "neu aufgebaut, nicht angehängt"
        );
        assert_eq!(
            series.values().last().map(|(t, _)| *t),
            other.last().map(|c| c.time)
        );
    }
}
