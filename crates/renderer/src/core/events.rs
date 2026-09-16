//! Event System - Handle mouse/touch interactions (pan, zoom, click)

use super::chart_renderer::scrollbar_geometry;
use super::chart_state::{ChartState, InteractionState};
use super::scrollbar::ScrollbarHit;
use super::wheel::{WheelGesture, WheelInput, WheelInterpreter};

/// Mouse button enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Mouse event types
#[derive(Debug, Clone)]
pub enum MouseEvent {
    Down { x: f64, y: f64, button: MouseButton },
    Up { x: f64, y: f64, button: MouseButton },
    Move { x: f64, y: f64 },
    Wheel(WheelInput),
    Leave,
    DoubleClick { x: f64, y: f64 },
}

/// Touch event types (for mobile)
#[derive(Debug, Clone)]
pub enum TouchEvent {
    Start { x: f64, y: f64 },
    Move { x: f64, y: f64 },
    End { x: f64, y: f64 },
    Cancel,
}

/// Keyboard event types
#[derive(Debug, Clone)]
pub enum KeyboardEvent {
    KeyDown { key: String },
    KeyUp { key: String },
}

/// Event handler for chart interactions
pub struct EventHandler {
    /// Track if we're currently dragging
    is_dragging: bool,
    /// Last mouse position during drag
    last_drag_x: f64,
    last_drag_y: f64,
    /// Track double-click timing
    last_click_time: f64,
    double_click_threshold: f64, // milliseconds
    /// Deutet Rad-/Trackpad-Ereignisse und hält die laufende Geste fest
    wheel: WheelInterpreter,
}

impl EventHandler {
    pub fn new() -> Self {
        Self {
            is_dragging: false,
            last_drag_x: 0.0,
            last_drag_y: 0.0,
            last_click_time: 0.0,
            double_click_threshold: 300.0,
            wheel: WheelInterpreter::new(),
        }
    }

    /// Handle mouse event and update chart state
    pub fn handle_mouse_event(&mut self, event: MouseEvent, state: &mut ChartState) {
        match event {
            MouseEvent::Down { x, y, button } => {
                self.handle_mouse_down(x, y, button, state);
            }
            MouseEvent::Up { x, y, button } => {
                self.handle_mouse_up(x, y, button, state);
            }
            MouseEvent::Move { x, y } => {
                self.handle_mouse_move(x, y, state);
            }
            MouseEvent::Wheel(input) => {
                self.handle_wheel(input, state);
            }
            MouseEvent::Leave => {
                self.handle_mouse_leave(state);
            }
            MouseEvent::DoubleClick { x: _, y: _ } => {
                self.handle_double_click(state);
            }
        }
    }

    fn handle_mouse_down(&mut self, x: f64, y: f64, button: MouseButton, state: &mut ChartState) {
        if button != MouseButton::Left {
            return;
        }

        // Die Zeitleiste hat Vorrang: sie liegt über dem Chart, und ein Klick
        // dort darf nicht als Pan des Kursbilds ankommen.
        if state.options.show_scrollbar && self.grab_scrollbar(x, y, state) {
            return;
        }

        self.is_dragging = true;
        self.last_drag_x = x;
        self.last_drag_y = y;
        state.start_pan(x, y);
    }

    /// Prüft, ob der Klick die Zeitleiste trifft, und beginnt die passende
    /// Interaktion. Gibt `true`, wenn der Klick verbraucht ist.
    fn grab_scrollbar(&mut self, x: f64, y: f64, state: &mut ChartState) -> bool {
        let vp = &state.viewport;
        if vp.bar_count() == 0 {
            return false;
        }

        let geometry = scrollbar_geometry(vp.dimensions.width as f64, vp.dimensions.height as f64);
        let bars = vp.bars();
        let count = vp.bar_count();
        let Some(hit) = geometry.hit(bars, count, x, y) else {
            return false;
        };

        match hit {
            ScrollbarHit::Body => {
                let grab_offset_bars = geometry.bar_at(x, bars, count) - bars.first;
                state.interaction = InteractionState::DraggingScrollbar { grab_offset_bars };
            }
            ScrollbarHit::Start => {
                state.interaction = InteractionState::ResizingScrollbar { start_edge: true };
            }
            ScrollbarHit::End => {
                state.interaction = InteractionState::ResizingScrollbar { start_edge: false };
            }
            // Blättern um eine Bildbreite — ein einzelner Sprung, kein Ziehen.
            ScrollbarHit::TrackBefore => {
                let span = bars.span();
                state.set_bars(bars.first - span, bars.last - span);
            }
            ScrollbarHit::TrackAfter => {
                let span = bars.span();
                state.set_bars(bars.first + span, bars.last + span);
            }
        }
        true
    }

    /// Führt ein laufendes Ziehen an der Zeitleiste fort.
    fn drag_scrollbar(&mut self, x: f64, state: &mut ChartState) -> bool {
        let vp = &state.viewport;
        let geometry = scrollbar_geometry(vp.dimensions.width as f64, vp.dimensions.height as f64);
        let bars = vp.bars();
        let count = vp.bar_count();
        let position = geometry.bar_at(x, bars, count);

        match state.interaction {
            InteractionState::DraggingScrollbar { grab_offset_bars } => {
                let first = position - grab_offset_bars;
                state.set_bars(first, first + bars.span());
                true
            }
            InteractionState::ResizingScrollbar { start_edge } => {
                if start_edge {
                    state.set_bars(position, bars.last);
                } else {
                    state.set_bars(bars.first, position);
                }
                true
            }
            _ => false,
        }
    }

    fn handle_mouse_up(&mut self, _x: f64, _y: f64, button: MouseButton, state: &mut ChartState) {
        if button == MouseButton::Left {
            self.is_dragging = false;
            state.end_interaction();
        }
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64, state: &mut ChartState) {
        if state.is_scrolling() {
            self.drag_scrollbar(x, state);
            // Der Zeiger steuert weiter den Griff — das Fadenkreuz folgt ihm
            // trotzdem, sonst verschwindet die Ablesung während des Ziehens.
            state.update_crosshair(x, y);
            return;
        }

        if self.is_dragging {
            // Calculate delta from last position
            let delta_x = (x - self.last_drag_x) as i32;
            let delta_y = (y - self.last_drag_y) as i32;

            // Pan the chart
            state.pan(delta_x, delta_y);

            // Update last position
            self.last_drag_x = x;
            self.last_drag_y = y;
        } else {
            // Update crosshair position
            state.update_crosshair(x, y);
        }
    }

    fn handle_wheel(&mut self, input: WheelInput, state: &mut ChartState) {
        let Some(gesture) = self.wheel.interpret(input) else {
            return;
        };

        match gesture {
            WheelGesture::Zoom { factor, center_x } => {
                state.zoom(factor, Some(center_x.max(0.0) as u32));
            }
            WheelGesture::Pan { delta_x, delta_y } => {
                // Rad-Deltas zeigen in die Gegenrichtung des Ziehens: nach unten
                // scrollen schiebt den Inhalt nach oben.
                state.pan(-delta_x as i32, -delta_y as i32);
            }
        }
    }

    fn handle_mouse_leave(&mut self, state: &mut ChartState) {
        self.is_dragging = false;
        state.hide_crosshair();
        state.end_interaction();
    }

    fn handle_double_click(&mut self, state: &mut ChartState) {
        // Reset view to fit all data — inklusive Preissperre, die ein
        // vertikales Verschieben gesetzt haben kann.
        state.reset_view();
    }

    /// Handle touch event (for mobile support)
    pub fn handle_touch_event(&mut self, event: TouchEvent, state: &mut ChartState) {
        match event {
            TouchEvent::Start { x, y } => {
                self.is_dragging = true;
                self.last_drag_x = x;
                self.last_drag_y = y;
                state.start_pan(x, y);
            }
            TouchEvent::Move { x, y } => {
                if self.is_dragging {
                    let delta_x = (x - self.last_drag_x) as i32;
                    let delta_y = (y - self.last_drag_y) as i32;

                    state.pan(delta_x, delta_y);

                    self.last_drag_x = x;
                    self.last_drag_y = y;
                }
            }
            TouchEvent::End { x: _, y: _ } | TouchEvent::Cancel => {
                self.is_dragging = false;
                state.end_interaction();
            }
        }
    }

    /// Handle keyboard event
    pub fn handle_keyboard_event(&mut self, event: KeyboardEvent, state: &mut ChartState) {
        match event {
            KeyboardEvent::KeyDown { key } => match key.as_str() {
                "ArrowLeft" => state.pan(50, 0),
                "ArrowRight" => state.pan(-50, 0),
                "ArrowUp" => state.pan(0, 50),
                "ArrowDown" => state.pan(0, -50),
                "+" | "=" => state.zoom(0.9, None),
                "-" | "_" => state.zoom(1.1, None),
                "Home" | "h" => state.reset_view(),
                _ => {}
            },
            KeyboardEvent::KeyUp { key: _ } => {
                // No action on key up for now
            }
        }
    }

    /// Check if we're detecting a double-click (based on timing)
    pub fn is_double_click(&mut self, current_time: f64) -> bool {
        let is_double = current_time - self.last_click_time < self.double_click_threshold;
        self.last_click_time = current_time;
        is_double
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Timeframe;

    #[test]
    fn test_event_handler_creation() {
        let handler = EventHandler::new();
        assert!(!handler.is_dragging);
    }

    #[test]
    fn test_mouse_drag() {
        let mut handler = EventHandler::new();
        let mut state = ChartState::new(800, 600, Timeframe::M5);

        // Start drag
        handler.handle_mouse_event(
            MouseEvent::Down {
                x: 100.0,
                y: 100.0,
                button: MouseButton::Left,
            },
            &mut state,
        );
        assert!(handler.is_dragging);

        // Move during drag
        handler.handle_mouse_event(MouseEvent::Move { x: 150.0, y: 100.0 }, &mut state);

        // End drag
        handler.handle_mouse_event(
            MouseEvent::Up {
                x: 150.0,
                y: 100.0,
                button: MouseButton::Left,
            },
            &mut state,
        );
        assert!(!handler.is_dragging);
    }

    #[test]
    fn test_crosshair_update() {
        let mut handler = EventHandler::new();
        let mut state = ChartState::new(800, 600, Timeframe::M5);

        handler.handle_mouse_event(MouseEvent::Move { x: 400.0, y: 300.0 }, &mut state);

        assert!(state.crosshair.visible);
        assert_eq!(state.crosshair.x, 400.0);
        assert_eq!(state.crosshair.y, 300.0);
    }

    #[test]
    fn test_mouse_leave() {
        let mut handler = EventHandler::new();
        let mut state = ChartState::new(800, 600, Timeframe::M5);

        // First show crosshair
        handler.handle_mouse_event(MouseEvent::Move { x: 400.0, y: 300.0 }, &mut state);
        assert!(state.crosshair.visible);

        // Then leave
        handler.handle_mouse_event(MouseEvent::Leave, &mut state);
        assert!(!state.crosshair.visible);
    }

    fn wheel(delta_x: f64, delta_y: f64, ts: f64) -> MouseEvent {
        MouseEvent::Wheel(WheelInput {
            x: 400.0,
            y: 300.0,
            delta_x,
            delta_y,
            ctrl_key: false,
            delta_mode: 0,
            timestamp_ms: ts,
        })
    }

    fn fitted_state() -> ChartState {
        let mut state = ChartState::new(800, 600, Timeframe::H1);
        let candles: Vec<_> = (0..200)
            .map(|i| {
                let t = 1_600_000_000 + i * 3600;
                crate::core::Candle::new(t, 100.0, 105.0, 95.0, 102.0, 10.0)
            })
            .collect();
        state.set_candles(candles);
        state
    }

    /// Der Befund: eine Magic Mouse schickt pro Wischer dutzende kleine
    /// Ereignisse. Vorher zoomte jedes davon um 10 % — nach einem Wischer war
    /// der Chart unbrauchbar. Jetzt verschiebt er.
    #[test]
    fn a_trackpad_swipe_pans_instead_of_zooming() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();

        let range_before = state.viewport.time_end() - state.viewport.time_start();
        let start_before = state.viewport.time_start();

        for step in 0..30 {
            handler.handle_mouse_event(wheel(6.0, 0.0, step as f64 * 16.0), &mut state);
        }

        let range_after = state.viewport.time_end() - state.viewport.time_start();
        assert_eq!(
            range_before, range_after,
            "ein Wischer darf den Zoom nicht anfassen"
        );
        assert!(
            state.viewport.time_start() > start_before,
            "nach rechts wischen muss den Ausschnitt nach vorn schieben"
        );
    }

    #[test]
    fn a_classic_wheel_notch_still_zooms() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();

        let before = state.viewport.time_end() - state.viewport.time_start();
        handler.handle_mouse_event(wheel(0.0, -100.0, 0.0), &mut state);
        let after = state.viewport.time_end() - state.viewport.time_start();

        assert!(after < before, "hochscrollen muss hineinzoomen");
    }

    #[test]
    fn a_vertical_swipe_moves_the_price_range() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();

        let min_before = state.viewport.price.min;
        handler.handle_mouse_event(wheel(0.0, 8.0, 0.0), &mut state);

        assert!(
            state.viewport.price.min < min_before,
            "nach unten scrollen muss niedrigere Preise ins Bild holen"
        );
        assert!(
            state.viewport.price_locked,
            "sonst zieht die nächste Kerze den Ausschnitt sofort zurück"
        );
    }

    #[test]
    fn a_double_click_releases_the_price_lock() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();

        handler.handle_mouse_event(wheel(0.0, 8.0, 0.0), &mut state);
        assert!(state.viewport.price_locked);

        handler.handle_mouse_event(MouseEvent::DoubleClick { x: 1.0, y: 1.0 }, &mut state);
        assert!(!state.viewport.price_locked);
    }

    // --- Zeitleiste ---

    fn scrollbar_y(state: &ChartState) -> f64 {
        let geometry = crate::core::chart_renderer::scrollbar_geometry(
            state.viewport.dimensions.width as f64,
            state.viewport.dimensions.height as f64,
        );
        geometry.y + geometry.height / 2.0
    }

    fn press(handler: &mut EventHandler, state: &mut ChartState, x: f64, y: f64) {
        handler.handle_mouse_event(
            MouseEvent::Down {
                x,
                y,
                button: MouseButton::Left,
            },
            state,
        );
    }

    #[test]
    fn dragging_the_scrollbar_thumb_moves_the_view() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();
        state.set_bars(20.0, 60.0);

        let y = scrollbar_y(&state);
        let geometry = crate::core::chart_renderer::scrollbar_geometry(800.0, 600.0);
        let thumb = geometry.thumb(state.viewport.bars(), state.viewport.bar_count());
        let grab = thumb.x + thumb.width / 2.0;

        press(&mut handler, &mut state, grab, y);
        handler.handle_mouse_event(MouseEvent::Move { x: grab + 100.0, y }, &mut state);

        let bars = state.viewport.bars();
        assert!(
            bars.first > 20.0,
            "der Ausschnitt muss nach rechts gewandert sein"
        );
        assert!(
            (bars.span() - 40.0).abs() < 1e-6,
            "Ziehen verschiebt, es zoomt nicht"
        );
    }

    #[test]
    fn dragging_a_thumb_edge_zooms() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();
        state.set_bars(20.0, 60.0);

        let y = scrollbar_y(&state);
        let geometry = crate::core::chart_renderer::scrollbar_geometry(800.0, 600.0);
        let thumb = geometry.thumb(state.viewport.bars(), state.viewport.bar_count());

        press(&mut handler, &mut state, thumb.x + thumb.width - 1.0, y);
        handler.handle_mouse_event(
            MouseEvent::Move {
                x: thumb.x + thumb.width + 60.0,
                y,
            },
            &mut state,
        );

        assert!(
            state.viewport.bars().span() > 40.0,
            "den rechten Rand nach außen ziehen zeigt mehr Bars"
        );
    }

    #[test]
    fn clicking_the_track_pages_by_one_screen() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();
        state.set_bars(60.0, 100.0);
        let before = state.viewport.bars();

        let y = scrollbar_y(&state);
        press(&mut handler, &mut state, 5.0, y);

        let after = state.viewport.bars();
        assert!((after.first - (before.first - before.span())).abs() < 1e-6);
        assert!((after.span() - before.span()).abs() < 1e-6);
    }

    /// Ein Klick auf die Leiste darf nicht als Pan des Kursbilds durchschlagen.
    #[test]
    fn a_click_on_the_scrollbar_does_not_pan_the_chart() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();
        state.set_bars(20.0, 60.0);

        let y = scrollbar_y(&state);
        let geometry = crate::core::chart_renderer::scrollbar_geometry(800.0, 600.0);
        let thumb = geometry.thumb(state.viewport.bars(), state.viewport.bar_count());

        press(&mut handler, &mut state, thumb.x + thumb.width / 2.0, y);

        assert!(
            !handler.is_dragging,
            "sonst würde der Chart zusätzlich mitgezogen"
        );
        assert!(state.is_scrolling());
    }

    #[test]
    fn a_click_on_the_chart_still_pans() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();

        press(&mut handler, &mut state, 400.0, 200.0);

        assert!(handler.is_dragging);
        assert!(!state.is_scrolling());
    }

    #[test]
    fn releasing_the_mouse_ends_the_scrollbar_drag() {
        let mut handler = EventHandler::new();
        let mut state = fitted_state();
        state.set_bars(20.0, 60.0);

        let y = scrollbar_y(&state);
        let geometry = crate::core::chart_renderer::scrollbar_geometry(800.0, 600.0);
        let thumb = geometry.thumb(state.viewport.bars(), state.viewport.bar_count());
        press(&mut handler, &mut state, thumb.x + thumb.width / 2.0, y);

        handler.handle_mouse_event(
            MouseEvent::Up {
                x: 400.0,
                y,
                button: MouseButton::Left,
            },
            &mut state,
        );

        assert!(!state.is_scrolling());
    }

    #[test]
    fn test_keyboard_navigation() {
        let mut handler = EventHandler::new();
        let mut state = ChartState::new(800, 600, Timeframe::M5);

        // Test arrow key panning
        handler.handle_keyboard_event(
            KeyboardEvent::KeyDown {
                key: "ArrowLeft".to_string(),
            },
            &mut state,
        );
        assert!(state.is_dirty());
    }
}
