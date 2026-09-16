//! Chart State Management - Central state container for the chart engine

use super::types::{Candle, Seconds, SessionConfig, Timeframe};
use super::viewport::{BarRange, PriceRange, TimeRange, Viewport};
use crate::primitives::{CandleStyle, Color};

/// Magnet/snap mode for drawing tool placement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnetMode {
    /// Snapping disabled
    Off,
    /// Snap only when near a candle high or low (within 20px)
    Weak,
    /// Snap to any OHLC level (within 20px)
    Strong,
}

/// Chart configuration
#[derive(Debug, Clone)]
pub struct ChartOptions {
    pub background_color: Color,
    pub grid_color: Color,
    pub text_color: Color,
    pub crosshair_color: Color,
    pub bullish_color: Color,
    pub bearish_color: Color,
    pub unchanged_color: Color,
    pub candle_style: CandleStyle,
    pub show_grid: bool,
    pub show_crosshair: bool,
    pub show_volume: bool,
    pub sessions: Vec<SessionConfig>,
    pub show_sessions: bool,
    /// Zeitleiste am unteren Rand.
    pub show_scrollbar: bool,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            background_color: Color::rgba(10, 14, 18, 1.0),
            grid_color: Color::rgba(45, 54, 64, 0.3),
            text_color: Color::rgba(231, 233, 234, 1.0),
            crosshair_color: Color::rgba(139, 152, 165, 0.5),
            bullish_color: Color::rgba(34, 197, 94, 1.0),
            bearish_color: Color::rgba(239, 68, 68, 1.0),
            unchanged_color: Color::rgba(201, 203, 207, 1.0), // Gray for Doji
            candle_style: CandleStyle::Candlestick,
            show_grid: true,
            show_crosshair: true,
            show_volume: true,
            sessions: Vec::new(),
            show_sessions: false,
            show_scrollbar: true,
        }
    }
}

/// Crosshair state
#[derive(Debug, Clone, Copy)]
pub struct CrosshairState {
    pub visible: bool,
    pub x: f64,
    pub y: f64,
    pub time: Seconds,
    pub price: f64,
}

impl Default for CrosshairState {
    fn default() -> Self {
        Self {
            visible: false,
            x: 0.0,
            y: 0.0,
            time: Seconds::default(),
            price: 0.0,
        }
    }
}

/// Interaction state for tracking mouse/touch input
#[derive(Debug, Clone, PartialEq, Default)]
pub enum InteractionState {
    #[default]
    Idle,
    Panning {
        start_x: f64,
        start_y: f64,
    },
    Selecting {
        start_x: f64,
        start_y: f64,
    },
    ScalingPrice {
        start_y: f64,                    // Inverted Y coordinate from start
        initial_price_range: PriceRange, // Snapshot of price range
    },
    ScalingTime {
        start_x: f64,           // X coordinate from start
        initial_bars: BarRange, // Schnappschuss des Bar-Ausschnitts
    },
    /// Der Griff der Zeitleiste wird gezogen.
    DraggingScrollbar {
        /// Wo im Griff gepackt wurde, in Bars ab dessen linkem Rand — damit der
        /// Griff nicht unter dem Zeiger wegspringt.
        grab_offset_bars: f64,
    },
    /// Ein Rand des Griffs wird gezogen (zoomen).
    ResizingScrollbar {
        /// `true` = linker Rand.
        start_edge: bool,
    },
}

/// Main chart state container
pub struct ChartState {
    pub viewport: Viewport,
    /// Der Kerzensatz. **Privat**, weil der Bar-Index des Viewports mit ihm
    /// synchron bleiben muss: jeder Schreibweg geht über [`ChartState::set_candles`]
    /// oder [`ChartState::add_candle`], und beide schreiben den Index mit fort.
    /// Dasselbe Muster wie `apply_dimensions` gegen B4 — ein Desync ist damit
    /// nicht formulierbar, statt nur unwahrscheinlich.
    candles: Vec<Candle>,
    pub options: ChartOptions,
    pub crosshair: CrosshairState,
    pub interaction: InteractionState,
    pub timeframe: Timeframe,
    pub tool_manager: crate::tools::ToolManager,
    pub selected_tools: Vec<String>,
    pub magnet_mode: MagnetMode,
    dirty: bool,
}

impl ChartState {
    pub fn new(width: u32, height: u32, timeframe: Timeframe) -> Self {
        let mut viewport = Viewport::new(width, height);
        viewport.set_timeframe(timeframe);

        Self {
            viewport,
            candles: Vec::new(),
            options: ChartOptions::default(),
            crosshair: CrosshairState::default(),
            interaction: InteractionState::default(),
            timeframe,
            tool_manager: crate::tools::ToolManager::new(),
            selected_tools: Vec::new(),
            magnet_mode: MagnetMode::Off,
            dirty: true,
        }
    }

    /// Lesezugriff auf den Kerzensatz.
    pub fn candles(&self) -> &[Candle] {
        &self.candles
    }

    /// Set candle data and auto-fit viewport
    pub fn set_candles(&mut self, candles: Vec<Candle>) {
        self.candles = candles;
        self.viewport.sync_bars(&self.candles);
        if !self.candles.is_empty() {
            self.fit_to_data();
        }
        self.mark_dirty();
    }

    /// Add a single candle (for real-time updates)
    pub fn add_candle(&mut self, candle: Candle) {
        let time = candle.time;
        // Check if we should update the last candle or add a new one
        if let Some(last) = self.candles.last_mut() {
            if last.time == candle.time {
                *last = candle;
            } else {
                self.candles.push(candle);
            }
        } else {
            self.candles.push(candle);
        }
        // Fortschreiben statt neu aufbauen: `sync_bars` wäre je Tick eine
        // O(n)-Kopie des gesamten Zeitstempelsatzes.
        self.viewport.push_bar(time);
        self.mark_dirty();
    }

    /// Fit viewport to show all data
    pub fn fit_to_data(&mut self) {
        if self.candles.is_empty() {
            return;
        }

        let time_start = self.candles.first().unwrap().time;
        let time_end = self.candles.last().unwrap().time;

        let mut min_price = f64::MAX;
        let mut max_price = f64::MIN;

        for candle in &self.candles {
            min_price = min_price.min(candle.l);
            max_price = max_price.max(candle.h);
        }

        // Add 5% padding
        let price_padding = (max_price - min_price) * 0.05;

        self.viewport.fit_to_data(
            TimeRange {
                start: time_start,
                end: time_end,
            },
            PriceRange {
                min: min_price - price_padding,
                max: max_price + price_padding,
            },
        );

        self.mark_dirty();
    }

    /// Resize the chart
    pub fn resize(&mut self, width: u32, height: u32) {
        let pixel_ratio = self.viewport.dimensions.pixel_ratio;
        self.apply_dimensions(width, height, pixel_ratio);
    }

    /// Setzt Maße samt Pixelverhältnis — und markiert den Zustand als verändert.
    ///
    /// Der einzige Weg, die Maße zu ändern. Vorher setzte die WASM-Fassade sie im
    /// Zweig mit angehängtem Renderer direkt am Viewport und vergaß dabei
    /// `mark_dirty()`; da der Browser den Canvas beim Setzen von `width`/`height`
    /// löscht und der nächste `render()` bei sauberem Zustand sofort zurückkehrt,
    /// blieb der Chart nach jeder Größenänderung leer. Über diesen Weg ist der
    /// Fehler nicht mehr formulierbar.
    pub fn apply_dimensions(&mut self, width: u32, height: u32, pixel_ratio: f64) {
        self.viewport.set_dimensions(width, height, pixel_ratio);
        self.mark_dirty();
    }

    /// Pan the viewport.
    ///
    /// Vertikales Verschieben sperrt die Preisskala — sonst zöge das nächste
    /// `fit_to_data()` (jede eintreffende Kerze) den Ausschnitt sofort wieder
    /// zurück. `reset_view()` hebt die Sperre auf.
    pub fn pan(&mut self, delta_x: i32, delta_y: i32) {
        self.viewport.pan(delta_x, delta_y);
        if delta_y != 0 {
            self.viewport.price_locked = true;
        }
        self.mark_dirty();
    }

    /// Setzt den sichtbaren Bar-Ausschnitt und markiert den Zustand als verändert.
    pub fn set_bars(&mut self, first: f64, last: f64) {
        self.viewport.set_bars(first, last);
        self.mark_dirty();
    }

    /// Wird gerade an der Zeitleiste gezogen?
    pub fn is_scrolling(&self) -> bool {
        matches!(
            self.interaction,
            InteractionState::DraggingScrollbar { .. } | InteractionState::ResizingScrollbar { .. }
        )
    }

    /// Cursor-Form über der Zeitleiste — `None`, wenn der Zeiger woanders ist.
    ///
    /// Der Kern zeichnet nur; die Einbindung setzt daraus `canvas.style.cursor`.
    /// Vorher gab es keine Trefferabfrage über die Fassade, also ließ sich die
    /// Greifhand am Griff von außen nicht anzeigen.
    pub fn scrollbar_cursor(&self, x: f64, y: f64) -> Option<&'static str> {
        if !self.options.show_scrollbar || self.viewport.bar_count() == 0 {
            return None;
        }

        if self.is_scrolling() {
            return Some("grabbing");
        }

        let geometry = super::chart_renderer::scrollbar_geometry(
            self.viewport.dimensions.width as f64,
            self.viewport.dimensions.height as f64,
        );
        geometry
            .hit(self.viewport.bars(), self.viewport.bar_count(), x, y)
            .map(super::scrollbar::ScrollbarHit::cursor)
    }

    /// Setzt die Ansicht zurück: Preissperre lösen und auf die Daten einpassen.
    pub fn reset_view(&mut self) {
        self.viewport.price_locked = false;
        self.fit_to_data();
    }

    /// Zoom the viewport
    pub fn zoom(&mut self, factor: f64, center_x: Option<u32>) {
        self.viewport.zoom(factor, center_x);
        self.mark_dirty();
    }

    /// Update crosshair position
    pub fn update_crosshair(&mut self, x: f64, y: f64) {
        self.crosshair.visible = true;
        self.crosshair.x = x;
        self.crosshair.y = y;
        self.crosshair.time = self.viewport.x_to_time(x);
        self.crosshair.price = self.viewport.y_to_price(y);
        self.mark_dirty();
    }

    /// Hide crosshair
    pub fn hide_crosshair(&mut self) {
        self.crosshair.visible = false;
        self.mark_dirty();
    }

    /// Sichtbarer Bar-Bereich als Index-Paar (einschließlich Start, ausschließlich
    /// Ende), auf den Kerzensatz geklemmt.
    pub fn visible_bar_range(&self) -> (usize, usize) {
        let n = self.candles.len();
        if n == 0 {
            return (0, 0);
        }
        let bars = self.viewport.bars();
        let first = bars.first.floor().clamp(0.0, n as f64) as usize;
        let last = (bars.last.ceil() + 1.0).clamp(0.0, n as f64) as usize;
        (first.min(last), last)
    }

    /// Get candles visible in current viewport.
    ///
    /// Ein Slice, keine Sammlung von Referenzen: auf der Bar-Achse sind die
    /// sichtbaren Kerzen ein zusammenhängender Abschnitt, also braucht es dafür
    /// weder einen Filterdurchlauf über alle Kerzen noch eine Allokation.
    pub fn visible_candles(&self) -> &[Candle] {
        let (first, last) = self.visible_bar_range();
        &self.candles[first..last]
    }

    /// Find candle at a given time
    pub fn candle_at_time(&self, time: Seconds) -> Option<&Candle> {
        self.candles.iter().find(|c| c.time == time)
    }

    /// Kerze an einer Bildschirmspalte — ein Direktzugriff statt einer Zeitsuche.
    ///
    /// Vorher suchte diese Funktion nach Kerzen innerhalb einer halben Bar-Dauer
    /// **in Zeit**. Neben einer Handelspause traf das die falsche Kerze oder gar
    /// keine, weil dort zwischen zwei benachbarten Bars Stunden liegen.
    pub fn candle_at_x(&self, x: f64) -> Option<&Candle> {
        let bar = self.viewport.x_to_bar(x).round();
        if bar < 0.0 {
            return None;
        }
        self.candles.get(bar as usize)
    }

    /// Find candle at position with hit-testing (includes Y coordinate check)
    pub fn candle_at_position(&self, x: f64, y: f64) -> Option<&Candle> {
        let bar_width = self.viewport.bar_width();
        let candle = self.candle_at_x(x)?;

        let candle_x = self.viewport.time_to_x(candle.time);
        let high_y = self.viewport.price_to_y(candle.h);
        let low_y = self.viewport.price_to_y(candle.l);

        candle
            .in_range(x, y, candle_x, bar_width, high_y, low_y)
            .then_some(candle)
    }

    /// Get OHLC data at crosshair position (for tooltip)
    pub fn get_ohlc_at_crosshair(&self) -> Option<(f64, f64, f64, f64, f64)> {
        if !self.crosshair.visible {
            return None;
        }

        self.candle_at_x(self.crosshair.x)
            .map(|c| (c.o, c.h, c.l, c.c, c.v))
    }

    /// Get formatted OHLC string at crosshair position
    pub fn get_ohlc_formatted(&self) -> Option<String> {
        if !self.crosshair.visible {
            return None;
        }

        self.candle_at_x(self.crosshair.x).map(|c| c.format_ohlc())
    }

    /// Mark state as dirty (needs redraw)
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Check if state needs redraw
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear dirty flag after rendering
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Start panning interaction
    pub fn start_pan(&mut self, x: f64, y: f64) {
        self.interaction = InteractionState::Panning {
            start_x: x,
            start_y: y,
        };
    }

    /// Start selection interaction
    pub fn start_select(&mut self, x: f64, y: f64) {
        self.interaction = InteractionState::Selecting {
            start_x: x,
            start_y: y,
        };
    }

    /// End current interaction
    pub fn end_interaction(&mut self) {
        self.interaction = InteractionState::Idle;
    }

    /// Get OHLCV data for display
    pub fn get_ohlcv_at_crosshair(&self) -> Option<(f64, f64, f64, f64, f64)> {
        if !self.crosshair.visible {
            return None;
        }

        self.candle_at_x(self.crosshair.x)
            .map(|c| (c.o, c.h, c.l, c.c, c.v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_state_creation() {
        let state = ChartState::new(800, 600, Timeframe::M5);
        assert_eq!(state.viewport.dimensions.width, 800);
        assert_eq!(state.viewport.dimensions.height, 600);
        assert!(state.is_dirty());
    }

    #[test]
    fn test_set_candles_and_fit() {
        let mut state = ChartState::new(800, 600, Timeframe::M5);
        let candles = vec![
            Candle::new(Seconds::new(1000), 100.0, 105.0, 95.0, 102.0, 1000.0),
            Candle::new(Seconds::new(1300), 102.0, 108.0, 100.0, 106.0, 1200.0),
        ];

        state.set_candles(candles);
        assert_eq!(state.candles.len(), 2);
        let time = state.viewport.time_range();
        assert!(time.start <= 1000);
        assert!(time.end >= 1300);
    }

    #[test]
    fn test_crosshair_update() {
        let mut state = ChartState::new(800, 600, Timeframe::M5);
        state.update_crosshair(400.0, 300.0);

        assert!(state.crosshair.visible);
        assert_eq!(state.crosshair.x, 400.0);
        assert_eq!(state.crosshair.y, 300.0);
    }

    #[test]
    fn test_interaction_states() {
        let mut state = ChartState::new(800, 600, Timeframe::M5);

        state.start_pan(100.0, 200.0);
        assert!(matches!(
            state.interaction,
            InteractionState::Panning { .. }
        ));

        state.end_interaction();
        assert_eq!(state.interaction, InteractionState::Idle);
    }
}

#[cfg(test)]
mod dimension_tests {
    use super::*;

    fn state() -> ChartState {
        let mut state = ChartState::new(800, 400, Timeframe::M5);
        state.clear_dirty();
        state
    }

    #[test]
    fn applying_dimensions_marks_the_state_dirty() {
        let mut s = state();
        s.apply_dimensions(1000, 500, 2.0);

        assert_eq!(s.viewport.dimensions.width, 1000);
        assert_eq!(s.viewport.dimensions.height, 500);
        assert_eq!(s.viewport.dimensions.pixel_ratio, 2.0);
        assert!(
            s.is_dirty(),
            "sonst bleibt der Canvas nach dem Umschalten leer — siehe plan/spezifikation/04-befunde.md B4"
        );
    }

    #[test]
    fn resize_keeps_the_pixel_ratio() {
        let mut s = state();
        s.apply_dimensions(800, 400, 3.0);
        s.clear_dirty();

        s.resize(640, 320);

        assert_eq!(s.viewport.dimensions.pixel_ratio, 3.0);
        assert!(s.is_dirty());
    }
}
