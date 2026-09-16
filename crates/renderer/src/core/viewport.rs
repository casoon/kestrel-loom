//! Viewport - Manages visible range and coordinate transformations
//!
//! The viewport tracks what portion of the data is visible and provides
//! transformations between different coordinate spaces:
//! - Data space (time, price)
//! - Logical space (bar indices)
//! - Screen space (pixels)
//!
//! **Positioniert wird nach Bar-Index, nicht nach Zeit** (M8, seit 2026-09-16).
//! Der sichtbare Ausschnitt ist ein Bereich fraktionaler Bar-Indizes; Zeit ist
//! davon abgeleitet. Vorher interpolierte `time_to_x` linear über die Zeitspanne
//! — eine 49-Stunden-Wochenendpause bekam damit denselben Pixelanteil wie 49
//! Handelsstunden, und die Preislinie zog sichtbar darüber hinweg. Herleitung
//! und Modell: `plan/spezifikation/02-bar-index-achse.md`.

use crate::core::bar_index::BarIndex;
use crate::core::types::{Candle, Seconds, Timeframe};

/// Time range in seconds (unix timestamp)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRange {
    pub start: Seconds,
    pub end: Seconds,
}

/// Sichtbarer Ausschnitt in fraktionalen Bar-Indizes.
///
/// `f64` statt `usize` aus drei Gründen: Zoom bleibt stufenlos, `first` darf
/// negativ und `last` größer als die Barzahl werden (der Leerraum links und
/// rechts), und Werkzeuge ankern auf Zeiten, die keine Bar sind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarRange {
    /// Index am linken Rand.
    pub first: f64,
    /// Index am rechten Rand.
    pub last: f64,
}

impl BarRange {
    pub fn span(&self) -> f64 {
        self.last - self.first
    }
}

/// Price range
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceRange {
    pub min: f64,
    pub max: f64,
}

/// Pixel dimensions
#[derive(Debug, Clone, Copy)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
    pub pixel_ratio: f64,
}

/// Price scale display mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportScaleMode {
    #[default]
    Price,
    Log,
    Percent,
    Indexed,
}

/// Kleinste und größte Zahl sichtbarer Bars.
const MIN_VISIBLE_BARS: f64 = 5.0;
const MAX_VISIBLE_BARS: f64 = 5000.0;

/// Viewport state
#[derive(Debug, Clone)]
pub struct Viewport {
    /// Sichtbarer Ausschnitt in Bar-Indizes — der Zustand, aus dem alles folgt.
    bars: BarRange,
    /// Zeitstempel je Bar. Nur über [`Viewport::sync_bars`] und
    /// [`Viewport::push_bar`] zu ändern, damit Index und Kerzensatz nicht
    /// auseinanderlaufen können — dasselbe Muster wie `apply_dimensions`.
    index: BarIndex,
    /// Visible price range
    pub price: PriceRange,
    /// Screen dimensions
    pub dimensions: Dimensions,
    /// Timeframe (for bar spacing). Privat — nur über
    /// [`Viewport::set_timeframe`], damit die Bar-Dauer des Index mitzieht.
    timeframe: Timeframe,
    /// Use logarithmic price scale
    pub log_scale: bool,
    /// When true, fit_to_data() leaves the price range unchanged
    pub price_locked: bool,
    /// Extra CSS pixels added to every bar's slot width (positive = wider)
    pub bar_spacing_extra: f64,
    /// Explicit bar body ratio (0.0 = auto, otherwise overrides computed ratio)
    pub bar_width_ratio: f64,
    /// Timezone offset in minutes from UTC (e.g. 60 = UTC+1, -300 = UTC-5)
    pub timezone_offset_minutes: i32,
    /// Price scale display mode
    pub scale_mode: ViewportScaleMode,
    /// Base price for percent/indexed modes (first visible candle close)
    pub scale_base_price: f64,
}

impl Viewport {
    /// Create new viewport
    pub fn new(width: u32, height: u32) -> Self {
        let timeframe = Timeframe::M5;
        Self {
            bars: BarRange {
                first: 0.0,
                last: 1.0,
            },
            index: BarIndex::empty(timeframe.duration_secs()),
            price: PriceRange {
                min: 0.0,
                max: 100.0,
            },
            dimensions: Dimensions {
                width,
                height,
                pixel_ratio: 1.0,
            },
            timeframe,
            log_scale: false,
            price_locked: false,
            bar_spacing_extra: 0.0,
            bar_width_ratio: 0.0,
            timezone_offset_minutes: 0,
            scale_mode: ViewportScaleMode::Price,
            scale_base_price: 0.0,
        }
    }

    /// Set dimensions
    pub fn set_dimensions(&mut self, width: u32, height: u32, pixel_ratio: f64) {
        self.dimensions = Dimensions {
            width,
            height,
            pixel_ratio,
        };
    }

    /// Setzt den Zeitrahmen — und damit die Bar-Dauer, mit der außerhalb der
    /// Daten extrapoliert wird.
    pub fn timeframe(&self) -> Timeframe {
        self.timeframe
    }

    pub fn set_timeframe(&mut self, timeframe: Timeframe) {
        self.timeframe = timeframe;
        self.index.set_bar_duration(timeframe.duration_secs());
    }

    // --- Bar-Index: der einzige Schreibweg ---

    /// Schreibt den Bar-Index auf den Kerzensatz fort.
    pub fn sync_bars(&mut self, candles: &[Candle]) {
        self.index = BarIndex::from_candles(candles, self.timeframe.duration_secs());
    }

    /// Hängt eine einzelne Bar an (Live-Betrieb).
    pub fn push_bar(&mut self, time: Seconds) {
        self.index.push_bar(time);
    }

    /// Lesezugriff auf den Index — für Achsenbeschriftung und Treffererkennung.
    pub fn bar_index(&self) -> &BarIndex {
        &self.index
    }

    /// Zahl der indizierten Bars.
    pub fn bar_count(&self) -> usize {
        self.index.len()
    }

    /// Sichtbarer Ausschnitt in Bar-Indizes.
    pub fn bars(&self) -> BarRange {
        self.bars
    }

    /// Setzt den sichtbaren Ausschnitt, geklemmt auf sinnvolle Bar-Zahlen.
    pub fn set_bars(&mut self, first: f64, last: f64) {
        let span = (last - first).clamp(MIN_VISIBLE_BARS, MAX_VISIBLE_BARS);
        // Mittelpunkt halten, wenn die Klemmung greift.
        let center = (first + last) / 2.0;
        self.bars = BarRange {
            first: center - span / 2.0,
            last: center + span / 2.0,
        };
    }

    /// Leerraum rechts vom letzten Bar, in Bars.
    ///
    /// Fällt aus dem fraktionalen Modell kostenlos an: `last` darf größer als
    /// die Barzahl werden. Handelsplattformen bieten diesen Platz an, um
    /// Werkzeuge in die Zukunft zu zeichnen.
    pub fn right_offset(&self) -> f64 {
        self.bars.last - (self.index.len() as f64 - 0.5)
    }

    /// Setzt den Leerraum rechts, ohne den Zoom zu ändern.
    pub fn set_right_offset(&mut self, bars: f64) {
        let span = self.bars.span();
        let last = self.index.len() as f64 - 0.5 + bars;
        self.bars = BarRange {
            first: last - span,
            last,
        };
    }

    // --- Zeit als abgeleitete Sicht ---

    /// Sichtbares Zeitfenster — abgeleitet aus dem Bar-Ausschnitt.
    pub fn time_range(&self) -> TimeRange {
        TimeRange {
            start: self.index.fractional_index_to_time(self.bars.first),
            end: self.index.fractional_index_to_time(self.bars.last),
        }
    }

    /// Setzt den Ausschnitt über ein Zeitfenster (Import, externe Steuerung).
    pub fn set_time_range(&mut self, range: TimeRange) {
        let first = self.index.time_to_fractional_index(range.start);
        let last = self.index.time_to_fractional_index(range.end);
        self.set_bars(first, last);
    }

    /// Fit viewport to data range
    ///
    /// Das Zeitfenster wird nur noch als Auswahl der Bars gelesen; die Ränder
    /// entstehen als Bruchteil der **Barzahl**, nicht der Zeitspanne.
    pub fn fit_to_data(&mut self, time_range: TimeRange, price_range: PriceRange) {
        debug_assert!(
            !self.index.is_empty(),
            "fit_to_data() ohne Bar-Index: jede Zeit bildet dann auf Index 0 ab und \
             der ganze Chart fällt auf eine Spalte zusammen. Vorher sync_bars() rufen."
        );
        let first_bar = self.index.time_to_fractional_index(time_range.start);
        let last_bar = self.index.time_to_fractional_index(time_range.end);
        let count = (last_bar - first_bar).max(1.0);
        let padding = count * 0.05;

        self.set_bars(first_bar - 0.5 - padding, last_bar + 0.5 + padding);

        // Respect axis lock: do not change price range when locked
        if !self.price_locked {
            self.price = price_range;
        }
    }

    /// Pan by pixel delta.
    ///
    /// Beide Achsen folgen dem Zeigefinger: nach rechts ziehen holt frühere
    /// Bars ins Bild, nach unten ziehen höhere Preise. `delta_y` wurde bis
    /// 2026-09-16 verworfen — vertikales Ziehen und die Pfeiltasten hoch/runter
    /// taten schlicht nichts.
    pub fn pan(&mut self, delta_x: i32, delta_y: i32) {
        if delta_x != 0 {
            let bars_per_pixel = self.bars.span() / self.dimensions.width.max(1) as f64;
            let shift = -delta_x as f64 * bars_per_pixel;
            self.bars.first += shift;
            self.bars.last += shift;
        }

        if delta_y != 0 {
            self.pan_price(delta_y as f64);
        }
    }

    /// Verschiebt den Preisbereich um `delta_y` Pixel (positiv = nach unten
    /// ziehen = höhere Preise ins Bild).
    ///
    /// Im Log-Modus wird im Logarithmus verschoben, sonst würde derselbe
    /// Pixelweg unten anders wirken als oben.
    pub fn pan_price(&mut self, delta_y: f64) {
        let h = self.dimensions.height as f64;
        if h <= 0.0 {
            return;
        }

        if self.log_scale {
            let log_min = self.price.min.max(1e-10).ln();
            let log_max = self.price.max.max(1e-10).ln();
            let shift = (delta_y / h) * (log_max - log_min);
            self.price.min = (log_min + shift).exp();
            self.price.max = (log_max + shift).exp();
        } else {
            let price_per_pixel = (self.price.max - self.price.min) / h;
            let shift = delta_y * price_per_pixel;
            self.price.min += shift;
            self.price.max += shift;
        }
    }

    /// Zoom around a point.
    ///
    /// Die Klemmung zählt jetzt **echte Bars**. Vorher rechnete sie
    /// `Zeitspanne / timeframe.duration_secs()` und zählte damit über einer
    /// Pause Bars mit, die es nicht gibt — der Zoom blockierte an der falschen
    /// Stelle.
    pub fn zoom(&mut self, factor: f64, center_x: Option<u32>) {
        let cx = center_x
            .map(|c| c as f64)
            .unwrap_or(self.dimensions.width as f64 / 2.0);
        let center_bar = self.x_to_bar(cx);

        let new_span = self.bars.span() * factor;
        if !(MIN_VISIBLE_BARS..=MAX_VISIBLE_BARS).contains(&new_span) {
            return;
        }

        self.bars = BarRange {
            first: center_bar - (center_bar - self.bars.first) * factor,
            last: center_bar + (self.bars.last - center_bar) * factor,
        };
    }

    // --- Koordinaten ---

    /// Bildschirm-x eines fraktionalen Bar-Index.
    pub fn bar_to_x(&self, bar: f64) -> f64 {
        let span = self.bars.span();
        if span <= 0.0 {
            return 0.0;
        }
        (bar - self.bars.first) / span * self.dimensions.width as f64
    }

    /// Fraktionaler Bar-Index an einer Bildschirmposition.
    pub fn x_to_bar(&self, x: f64) -> f64 {
        let width = self.dimensions.width.max(1) as f64;
        self.bars.first + (x / width) * self.bars.span()
    }

    /// Convert time to x pixel coordinate
    pub fn time_to_x(&self, time: Seconds) -> f64 {
        self.bar_to_x(self.index.time_to_fractional_index(time))
    }

    /// Convert x pixel to time
    pub fn x_to_time(&self, x: f64) -> Seconds {
        self.index.fractional_index_to_time(self.x_to_bar(x))
    }

    /// Convert price to y pixel coordinate
    pub fn price_to_y(&self, price: f64) -> f64 {
        let h = self.dimensions.height as f64;
        if self.log_scale {
            let log_min = self.price.min.max(1e-10).ln();
            let log_max = self.price.max.max(1e-10).ln();
            let log_p = price.max(1e-10).ln();
            (log_max - log_p) / (log_max - log_min) * h
        } else {
            // Y is inverted (0 at top)
            (self.price.max - price) / (self.price.max - self.price.min) * h
        }
    }

    /// Convert y pixel to price
    pub fn y_to_price(&self, y: f64) -> f64 {
        let h = self.dimensions.height as f64;
        if self.log_scale {
            let log_min = self.price.min.max(1e-10).ln();
            let log_max = self.price.max.max(1e-10).ln();
            let frac = y / h;
            (log_max - frac * (log_max - log_min)).exp()
        } else {
            // Y is inverted
            self.price.max - (y / h) * (self.price.max - self.price.min)
        }
    }

    /// Compute logarithmically-spaced price levels for grid lines (log scale only)
    pub fn log_grid_prices(&self, num_lines: usize) -> Vec<f64> {
        let log_min = self.price.min.max(1e-10).ln();
        let log_max = self.price.max.max(1e-10).ln();
        (0..=num_lines)
            .map(|i| (log_min + i as f64 * (log_max - log_min) / num_lines as f64).exp())
            .collect()
    }

    /// Get bar slot width in CSS pixels — exakt, nicht geschätzt.
    pub fn bar_width(&self) -> f64 {
        let span = self.bars.span();
        if span <= 0.0 {
            return 1.0;
        }
        let base = self.dimensions.width as f64 / span;
        (base + self.bar_spacing_extra).clamp(1.0, 200.0)
    }

    /// Get number of visible bars — exakt, nicht über die Zeitspanne geschätzt.
    pub fn visible_bars(&self) -> usize {
        self.bars.span().ceil().max(0.0) as usize
    }

    /// Get viewport time start (for optimizations)
    pub fn time_start(&self) -> Seconds {
        self.time_range().start
    }

    /// Get viewport time end (for optimizations)
    pub fn time_end(&self) -> Seconds {
        self.time_range().end
    }

    /// Get viewport width (for optimizations)
    pub fn width(&self) -> u32 {
        self.dimensions.width
    }

    /// Get viewport height (for optimizations)
    pub fn height(&self) -> u32 {
        self.dimensions.height
    }

    /// Scale price range around center (for interactive scaling)
    /// Similar to lightweight-charts implementation
    pub fn scale_price_around_center(&mut self, scale_coefficient: f64) {
        let center = (self.price.min + self.price.max) / 2.0;
        let range = self.price.max - self.price.min;
        let new_range = range * scale_coefficient;

        // Clamp to prevent extreme zoom (minimum 0.1x, maximum 10x of original)
        let clamped_range = new_range.max(range * 0.1).min(range * 10.0);

        self.price.min = center - clamped_range / 2.0;
        self.price.max = center + clamped_range / 2.0;
    }

    /// Start price scaling - captures initial Y position
    /// Returns the inverted Y coordinate for tracking
    pub fn start_price_scale(&self, y: f64) -> f64 {
        // Invert Y (0 is top, height is bottom)
        self.dimensions.height as f64 - y
    }

    /// Apply price scaling based on Y movement
    /// start_y: Initial Y position (inverted) from start_price_scale()
    /// current_y: Current Y position (not inverted)
    /// initial_price_range: Snapshot of price range when scaling started
    pub fn apply_price_scale(
        &mut self,
        start_y: f64,
        current_y: f64,
        initial_price_range: &PriceRange,
    ) {
        // Invert current Y
        let y = self.dimensions.height as f64 - current_y;

        // Clamp to valid range
        let y = y.max(0.0);

        // Calculate scale coefficient with 20% padding (like lightweight-charts)
        let height = self.dimensions.height as f64;
        let padding_factor = 0.2;

        let scale_coeff =
            (start_y + (height - 1.0) * padding_factor) / (y + (height - 1.0) * padding_factor);

        // Limit scale coefficient to minimum 0.1 (10x minimum zoom)
        let scale_coeff = scale_coeff.max(0.1);

        // Calculate new range from initial snapshot
        let center = (initial_price_range.min + initial_price_range.max) / 2.0;
        let initial_range = initial_price_range.max - initial_price_range.min;
        let new_range = initial_range * scale_coeff;

        // Apply new range
        self.price.min = center - new_range / 2.0;
        self.price.max = center + new_range / 2.0;
    }

    /// Start time scaling - captures initial X position
    /// Returns the X coordinate for tracking
    pub fn start_time_scale(&self, x: f64) -> f64 {
        x
    }

    /// Apply time scaling based on X movement.
    ///
    /// `initial_bars`: Schnappschuss des Ausschnitts, als das Ziehen begann.
    pub fn apply_time_scale(&mut self, start_x: f64, current_x: f64, initial_bars: &BarRange) {
        // Clamp to valid range
        let current_x = current_x.max(0.0);
        let start_x = start_x.max(0.0);

        // Calculate scale coefficient with 20% padding
        let width = self.dimensions.width as f64;
        let padding_factor = 0.2;

        let scale_coeff = (start_x + (width - 1.0) * padding_factor)
            / (current_x + (width - 1.0) * padding_factor);

        // Limit scale coefficient to minimum 0.1 (10x minimum zoom)
        let scale_coeff = scale_coeff.max(0.1);

        let center = (initial_bars.first + initial_bars.last) / 2.0;
        let new_span = initial_bars.span() * scale_coeff;

        self.set_bars(center - new_span / 2.0, center + new_span / 2.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H1: i64 = 3600;

    fn candles_at(times: &[i64]) -> Vec<Candle> {
        times
            .iter()
            .map(|&t| Candle::new(Seconds::new(t), 100.0, 105.0, 95.0, 102.0, 10.0))
            .collect()
    }

    /// Zehn Stundenbars, 49 Stunden Pause, fünf weitere Bars.
    fn weekend_viewport() -> Viewport {
        let mut times: Vec<i64> = (0..10).map(|i| i * H1).collect();
        let close = times[9];
        times.extend((1..=5).map(|i| close + 49 * H1 + i * H1));

        let mut vp = Viewport::new(800, 600);
        vp.set_timeframe(Timeframe::H1);
        vp.sync_bars(&candles_at(&times));
        vp.fit_to_data(
            TimeRange {
                start: Seconds::new(times[0]),
                end: Seconds::new(*times.last().unwrap()),
            },
            PriceRange {
                min: 90.0,
                max: 110.0,
            },
        );
        vp
    }

    fn regular_viewport(n: usize) -> Viewport {
        let times: Vec<i64> = (0..n as i64).map(|i| i * H1).collect();
        let mut vp = Viewport::new(800, 600);
        vp.set_timeframe(Timeframe::H1);
        vp.sync_bars(&candles_at(&times));
        vp.fit_to_data(
            TimeRange {
                start: Seconds::new(times[0]),
                end: Seconds::new(*times.last().unwrap()),
            },
            PriceRange {
                min: 90.0,
                max: 110.0,
            },
        );
        vp
    }

    #[test]
    fn bars_map_to_pixels_and_back() {
        let vp = regular_viewport(100);
        for bar in [0.0, 12.5, 99.0] {
            let x = vp.bar_to_x(bar);
            assert!(
                (vp.x_to_bar(x) - bar).abs() < 1e-9,
                "Rundgang für Bar {bar} über x = {x}"
            );
        }
    }

    #[test]
    fn price_maps_to_pixels() {
        let mut vp = Viewport::new(800, 600);
        vp.price = PriceRange {
            min: 100.0,
            max: 200.0,
        };
        assert_eq!(vp.price_to_y(200.0), 0.0);
        assert_eq!(vp.price_to_y(100.0), 600.0);
        assert_eq!(vp.price_to_y(150.0), 300.0);
    }

    /// Der Befund aus `plan/spezifikation/02-bar-index-achse.md`: auf der Zeitachse bekam eine
    /// 49-Stunden-Pause 49 Bar-Breiten Platz. Jetzt genau eine.
    #[test]
    fn a_trading_break_takes_exactly_one_bar_of_width() {
        let vp = weekend_viewport();

        let friday = vp.time_to_x(Seconds::new(9 * H1));
        let monday = vp.time_to_x(Seconds::new(9 * H1 + 50 * H1));
        let step = vp.time_to_x(Seconds::new(H1)) - vp.time_to_x(Seconds::new(0));

        assert!(
            ((monday - friday) - step).abs() < 1e-6,
            "über die Pause: {} px, zwischen zwei Bars: {step} px",
            monday - friday
        );
    }

    /// Ebenfalls aus dem Befund: `visible_bars()` schätzte über die Zeitspanne
    /// und zählte über einer Pause Bars mit, die es nicht gibt.
    #[test]
    fn visible_bars_counts_bars_that_exist() {
        let vp = weekend_viewport();
        let visible = vp.visible_bars();
        assert!(
            (15..=18).contains(&visible),
            "15 Bars plus Ränder, gefunden {visible} — auf der Zeitachse waren es über 60"
        );
    }

    #[test]
    fn bar_width_stays_usable_across_a_break() {
        let vp = weekend_viewport();
        assert!(
            vp.bar_width() > 10.0,
            "15 Bars auf 800 px, gefunden {}",
            vp.bar_width()
        );
    }

    #[test]
    fn panning_shifts_the_visible_bars() {
        let mut vp = regular_viewport(100);
        let before = vp.bars();

        vp.pan(100, 0);

        assert!(
            vp.bars().first < before.first,
            "nach rechts ziehen holt frühere Bars ins Bild"
        );
        assert!(
            (vp.bars().span() - before.span()).abs() < 1e-9,
            "Verschieben ändert den Zoom nicht"
        );
    }

    #[test]
    fn zoom_halves_the_visible_span() {
        let mut vp = regular_viewport(200);
        let before = vp.bars().span();

        vp.zoom(0.5, Some(400));

        assert!((vp.bars().span() - before * 0.5).abs() < 1e-9);
    }

    #[test]
    fn zoom_keeps_the_bar_under_the_cursor_in_place() {
        let mut vp = regular_viewport(200);
        let anchor = 300.0;
        let bar_before = vp.x_to_bar(anchor);

        vp.zoom(0.5, Some(anchor as u32));

        assert!(
            (vp.x_to_bar(anchor) - bar_before).abs() < 1e-6,
            "unter dem Zeiger muss dieselbe Bar bleiben"
        );
    }

    #[test]
    fn zoom_stops_at_the_limits() {
        let mut vp = regular_viewport(100);
        for _ in 0..200 {
            vp.zoom(0.9, None);
        }
        assert!(vp.bars().span() >= MIN_VISIBLE_BARS);

        for _ in 0..400 {
            vp.zoom(1.1, None);
        }
        assert!(vp.bars().span() <= MAX_VISIBLE_BARS);
    }

    #[test]
    fn the_time_range_follows_the_bars() {
        let vp = regular_viewport(100);
        let range = vp.time_range();
        assert!(range.start < 0, "links vom ersten Bar liegt Rand");
        assert!(range.end > 99 * H1, "rechts vom letzten Bar liegt Rand");
    }

    #[test]
    fn a_time_range_can_be_set_and_read_back() {
        let mut vp = regular_viewport(100);
        vp.set_time_range(TimeRange {
            start: Seconds::new(10 * H1),
            end: Seconds::new(60 * H1),
        });

        let back = vp.time_range();
        assert_eq!(back.start, 10 * H1);
        assert_eq!(back.end, 60 * H1);
    }

    #[test]
    fn the_right_offset_leaves_room_after_the_last_bar() {
        let mut vp = regular_viewport(100);
        let span_before = vp.bars().span();

        vp.set_right_offset(20.0);

        assert!((vp.right_offset() - 20.0).abs() < 1e-9);
        assert!(
            (vp.bars().span() - span_before).abs() < 1e-9,
            "der Leerraum ändert den Zoom nicht"
        );
        // Der letzte Bar steht jetzt links vom rechten Rand.
        let last_x = vp.time_to_x(Seconds::new(99 * H1));
        assert!(last_x < vp.dimensions.width as f64);
    }

    #[test]
    fn vertical_panning_moves_the_price_window() {
        let mut vp = regular_viewport(100);
        let before = vp.price;

        vp.pan(0, 60);

        assert!(vp.price.min > before.min);
        assert!((vp.price.max - vp.price.min - (before.max - before.min)).abs() < 1e-9);
    }
}

#[cfg(test)]
mod unit_consistency_tests {
    use super::*;
    use crate::core::{CandleGenerator, GeneratorConfig, Timeframe};

    /// Regression: Generator, `Candle.time` und die Viewport-Rechnungen müssen
    /// dieselbe Zeiteinheit (Unix-Sekunden) benutzen. Vorher lieferte der Generator
    /// Millisekunden — `visible_bars` war dadurch um Faktor 1000 zu hoch,
    /// `bar_width` auf 1 px geklemmt und `zoom` wirkungslos.
    ///
    /// Seit M8 zählt `visible_bars()` echte Bars; der Einheitenfehler zeigte sich
    /// dann in `time_to_fractional_index`, das über die Bar-Dauer extrapoliert.
    #[test]
    fn generator_and_viewport_share_the_time_unit() {
        let tf = Timeframe::M5;
        let mut generator =
            CandleGenerator::new(GeneratorConfig::crypto().with_seed(1).with_timeframe(tf));
        let candles = generator.generate(100);

        let step = candles[1].time - candles[0].time;
        assert_eq!(
            step,
            tf.duration_secs(),
            "Generator-Schrittweite muss der Timeframe-Dauer in Sekunden entsprechen"
        );

        let mut viewport = Viewport::new(800, 400);
        viewport.set_timeframe(tf);
        viewport.sync_bars(&candles);
        viewport.fit_to_data(
            TimeRange {
                start: candles[0].time,
                end: candles[candles.len() - 1].time,
            },
            PriceRange {
                min: 90.0,
                max: 110.0,
            },
        );

        let visible = viewport.visible_bars();
        assert!(
            (50..=200).contains(&visible),
            "visible_bars() = {visible}, erwartet in der Größenordnung von 100"
        );
        assert!(
            viewport.bar_width() > 1.0,
            "bar_width() darf nicht auf das Minimum geklemmt sein"
        );

        // Die Extrapolation jenseits der Daten arbeitet in Sekunden.
        let last_time = candles.last().unwrap().time;
        let one_bar_later = viewport.bar_index().fractional_index_to_time(100.0);
        assert_eq!(one_bar_later, last_time + tf.duration_secs());
    }

    #[test]
    fn zoom_actually_changes_the_visible_span() {
        let tf = Timeframe::M5;
        let candles =
            CandleGenerator::new(GeneratorConfig::crypto().with_seed(1).with_timeframe(tf))
                .generate(100);

        let mut viewport = Viewport::new(800, 400);
        viewport.set_timeframe(tf);
        viewport.sync_bars(&candles);
        viewport.fit_to_data(
            TimeRange {
                start: candles[0].time,
                end: candles.last().unwrap().time,
            },
            PriceRange {
                min: 90.0,
                max: 110.0,
            },
        );

        let before = viewport.bars().span();
        viewport.zoom(0.5, None);
        let after = viewport.bars().span();

        assert!(
            after < before,
            "zoom(0.5) muss den Ausschnitt verkleinern (vorher {before}, nachher {after})"
        );
    }
}
