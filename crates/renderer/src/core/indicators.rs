//! Indikatoren als laufende Instanzen über der Kerzenserie.
//!
//! Gerechnet wird in `kestrel-chartkit`; dieses Modul hält nur die Instanzen,
//! füttert sie **inkrementell** (`Indicator::on_bar` je neuer Kerze, kein
//! Neuberechnen des Fensters) und bewahrt die Werte für die Darstellung auf.
//!
//! Das ist der Bruch mit `loomcharts` Modell, das jeden Indikator bei jedem Frame
//! über das volle Fenster neu rechnete.

use std::collections::HashMap;

use kestrel_chartkit::artifact::Artifact;
use kestrel_chartkit::indicator::registry::{build_checked, catalog};
use kestrel_chartkit::{Bar, Indicator};

use crate::core::Candle;

/// Wohin ein Indikator gezeichnet wird.
///
/// `kestrel-chartkit` sagt das nicht: `IndicatorCatalogEntry` trägt nur Name,
/// Beschreibung und Standardparameter. Entscheidend ist, ob die Ausgabe in
/// **Preiseinheiten** liegt — dann gehört sie auf den Preischart, sonst in ein
/// eigenes Pane mit eigener Skala. Das ist eine Eigenschaft des Indikators, keine
/// Darstellungsvorliebe, und gehört langfristig in Chartkits Katalog
/// (siehe `plan/02-chartkit-vertrag.md`). Bis dahin steht sie hier — explizit
/// aufgeführt statt aus Wertebereichen geraten: ein RSI auf einem Instrument, das
/// um 50 notiert, wäre von einer Heuristik nicht von einem Preis zu unterscheiden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndicatorPlacement {
    /// Ausgabe in Preiseinheiten — auf den Preischart.
    Overlay,
    /// Eigene Skala — eigenes Pane.
    #[default]
    Pane,
}

/// Indikatoren, deren Ausgabe in Preiseinheiten liegt.
///
/// Unbekannte Namen gelten als [`IndicatorPlacement::Pane`]: ein falsch platziertes
/// Overlay verzerrt die Preisskala des Hauptcharts, ein überflüssiges Pane nicht.
const PRICE_UNIT_INDICATORS: &[&str] = &[
    "alligator",
    "anchored_vwap",
    "bollinger",
    "chandelier_exit",
    "chandelier_flip_radar",
    "dema",
    "donchian",
    "ema",
    "envelope",
    "hma",
    "ichimoku",
    "kama",
    "keltner",
    "lsma",
    "mcginley",
    "midas",
    "parabolic_sar",
    "sma",
    "supertrend",
    "tema",
    "vwap",
    "vwma",
    "wma",
    "zigzag",
    "zigzag_advanced",
];

/// Zusatzwerte aus `IndicatorOutput::extra`, die eine eigene Linie sind.
///
/// Chartkit legt Nebenserien überwiegend in `extra` ab (44 der Indikatoren nutzen
/// `with_extra`, nur 5 das Feld `secondary`) — MACDs Signallinie und Histogramm etwa,
/// oder die Bänder von Bollinger, Keltner und Donchian. Dort stehen aber auch Werte,
/// die **keine** Linie sind: `bandwidth`, `percent_b`, `width`, `atr`, `trend`. Die
/// als Linie zu zeichnen würde die Skala des Panes verzerren.
///
/// Deshalb eine ausdrückliche Liste statt „alles aus `extra`". Die Namen sind über
/// Chartkit hinweg einheitlich vergeben, sie gilt also nicht je Indikator.
const SERIES_EXTRA_KEYS: &[&str] = &[
    "basis", "upper", "lower", "signal", "hist", "tenkan", "kijun", "senkou_a", "senkou_b",
];

/// Reihenfolge der Linien — fest, damit Farben und Fixtures stabil bleiben.
const LINE_ORDER: &[&str] = &[
    "value",
    "basis",
    "upper",
    "lower",
    "secondary",
    "signal",
    "hist",
    "tenkan",
    "kijun",
    "senkou_a",
    "senkou_b",
];
/// Wohin dieser Indikator gehört.
pub fn placement_for(name: &str) -> IndicatorPlacement {
    if PRICE_UNIT_INDICATORS.contains(&name) {
        IndicatorPlacement::Overlay
    } else {
        IndicatorPlacement::Pane
    }
}

/// Eine benannte Linie einer Indikatorausgabe.
#[derive(Debug, Clone, Default)]
pub struct IndicatorLine {
    pub label: &'static str,
    pub points: Vec<(i64, f64)>,
}

/// Eine laufende Indikator-Instanz mit ihren bisherigen Ausgaben.
pub struct IndicatorSeries {
    name: String,
    params: HashMap<String, f64>,
    inner: Box<dyn Indicator>,
    /// Alle Linien der Ausgabe in fester Reihenfolge ([`LINE_ORDER`]). Sie alle zu
    /// zeichnen ist der Unterschied zwischen einem MACD und einer einzelnen Linie,
    /// die so tut, als wäre sie einer.
    lines: Vec<IndicatorLine>,
    /// Artefakte der zuletzt meldenden Bar — Zonen, Pivots, Profile.
    ///
    /// Bewusst nur die letzte Meldung, nicht gesammelt: Seit Chartkit 0.2.0 tragen
    /// Artefakte ihre eigene Zeitspanne (`from_ts`/`to_ts`), die jüngste Meldung
    /// beschreibt den Sachverhalt also bereits vollständig. Anzusammeln hieße, jede
    /// Bar dieselbe Zone erneut zu speichern.
    artifacts: Vec<Artifact>,
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
            lines: LINE_ORDER
                .iter()
                .map(|label| IndicatorLine {
                    label,
                    points: Vec::new(),
                })
                .collect(),
            artifacts: Vec::new(),
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

    /// Die Hauptlinie.
    pub fn values(&self) -> &[(i64, f64)] {
        &self.lines[0].points
    }

    /// Alle Linien, die tatsächlich Werte haben.
    pub fn lines(&self) -> impl Iterator<Item = &IndicatorLine> {
        self.lines.iter().filter(|l| !l.points.is_empty())
    }

    /// Wohin dieser Indikator gehört.
    pub fn placement(&self) -> IndicatorPlacement {
        placement_for(&self.name)
    }

    /// Artefakte der zuletzt meldenden Bar.
    pub fn artifacts(&self) -> &[Artifact] {
        &self.artifacts
    }

    /// Verwirft den Zustand und beginnt von vorn.
    pub fn reset(&mut self) {
        self.inner.reset();
        for line in &mut self.lines {
            line.points.clear();
        }
        self.artifacts.clear();
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
                for line in &mut self.lines {
                    let value = match line.label {
                        "value" => Some(out.value),
                        "secondary" => out.secondary,
                        "signal" => out.signal.or_else(|| out.extra.get("signal").copied()),
                        key if SERIES_EXTRA_KEYS.contains(&key) => out.extra.get(key).copied(),
                        _ => None,
                    };
                    if let Some(v) = value {
                        if v.is_finite() {
                            line.points.push((candle.time, v));
                        }
                    }
                }

                if !out.artifacts.is_empty() {
                    self.artifacts.clone_from(&out.artifacts);
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
            .field("values", &self.lines[0].points.len())
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
    fn macd_delivers_more_than_one_line() {
        let mut series = IndicatorSeries::new("macd", HashMap::new()).unwrap();
        series.feed(&candles(120));
        assert!(
            series.lines().count() >= 2,
            "MACD hat Signallinie und/oder Histogramm, nicht nur einen Wert"
        );
    }

    #[test]
    fn placement_separates_price_units_from_own_scales() {
        assert_eq!(placement_for("bollinger"), IndicatorPlacement::Overlay);
        assert_eq!(placement_for("supertrend"), IndicatorPlacement::Overlay);
        assert_eq!(placement_for("rsi"), IndicatorPlacement::Pane);
        assert_eq!(placement_for("macd"), IndicatorPlacement::Pane);
        assert_eq!(
            placement_for("gibtsnicht"),
            IndicatorPlacement::Pane,
            "unbekannt gilt als Pane — ein falsches Overlay verzerrt die Preisskala"
        );
    }

    #[test]
    fn every_overlay_name_exists_in_the_catalog() {
        let known = IndicatorSeries::available();
        for name in PRICE_UNIT_INDICATORS {
            assert!(
                known.iter().any(|k| k == name),
                "{name} steht in der Overlay-Liste, aber nicht in Chartkits Katalog"
            );
        }
    }

    #[test]
    fn artifacts_arrive_with_their_own_time_span() {
        let mut series = IndicatorSeries::new("extended_volume_profile", HashMap::new()).unwrap();
        series.feed(&candles(300));

        assert!(
            !series.artifacts().is_empty(),
            "das Volumenprofil meldet Zonen und Profile"
        );

        let spans: Vec<Option<(i64, i64)>> = series
            .artifacts()
            .iter()
            .map(|a| match a {
                Artifact::Zone(z) => z.span(),
                Artifact::Profile(p) => p.span(),
                _ => None,
            })
            .collect();
        assert!(
            spans.iter().all(|s| s.is_some()),
            "seit Chartkit 0.2.0 tragen sie ihre Zeitgrenzen selbst: {spans:?}"
        );
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
