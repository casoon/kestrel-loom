//! Rad- und Trackpad-Eingaben deuten.
//!
//! Ein Browser liefert für das Mausrad, das Trackpad und die Magic Mouse
//! dasselbe `wheel`-Ereignis — unterscheiden lassen sie sich nur über die Form
//! der Deltas. Das ist der einzige Ort, an dem diese Unterscheidung stattfindet;
//! `EventHandler` bekommt fertig gedeutete Gesten.
//!
//! Vorher zoomte jedes Rad-Ereignis um feste 10 %, unabhängig von der
//! Deltagröße. Eine Magic Mouse schickt pro Wischer dutzende kleine Ereignisse
//! samt Nachlauf — das ergab dutzende 10-%-Schritte und einen unbrauchbaren
//! Chart. Siehe `plan/09-scrolling.md`.

/// Ein Rad-Ereignis, so wie der Browser es liefert.
///
/// Alle Werte werden hereingereicht, nichts wird aus der Umgebung geholt
/// (`plan/06-anforderungen.md` A4) — deshalb trägt das Ereignis auch seinen
/// eigenen Zeitstempel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelInput {
    pub x: f64,
    pub y: f64,
    pub delta_x: f64,
    pub delta_y: f64,
    /// `ctrlKey`. macOS setzt das bei der Pinch-Geste auch ohne gedrückte Taste —
    /// es ist das einzige eindeutige Signal, das ein Browser über Gesten gibt.
    pub ctrl_key: bool,
    /// `deltaMode`: 0 = Pixel, 1 = Zeilen, 2 = Seiten.
    pub delta_mode: u32,
    /// `event.timeStamp` in Millisekunden.
    pub timestamp_ms: f64,
}

impl WheelInput {
    /// Klassisches Rad: grobe Rasterung, ein Ereignis je Rastung.
    pub fn wheel(x: f64, y: f64, delta_y: f64, timestamp_ms: f64) -> Self {
        Self {
            x,
            y,
            delta_x: 0.0,
            delta_y,
            ctrl_key: false,
            delta_mode: 0,
            timestamp_ms,
        }
    }
}

/// Was aus einem Rad-Ereignis werden soll.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelGesture {
    /// Zeitachse zoomen. `factor > 1` vergrößert das Fenster (herauszoomen).
    Zoom { factor: f64, center_x: f64 },
    /// Ausschnitt verschieben — in Rad-Deltas, nicht in Ziehpixeln.
    Pan { delta_x: f64, delta_y: f64 },
}

/// Die Geräteart hinter einem Ereignis, so weit sie sich erkennen lässt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WheelKind {
    /// Klassisches Rad oder Pinch — grobe Schritte, Zoom.
    Zoom,
    /// Trackpad, Magic Mouse — feine Zwei-Achsen-Deltas, Verschieben.
    Precision,
}

/// Eine Zeile entspricht rund dieser Pixelzahl (`deltaMode == 1`, Firefox).
const LINE_HEIGHT_PX: f64 = 16.0;
/// Eine Seite entspricht rund dieser Pixelzahl (`deltaMode == 2`).
const PAGE_HEIGHT_PX: f64 = 400.0;
/// Bis zu dieser Deltagröße gilt ein Pixel-Ereignis als Präzisionsgerät.
///
/// Ein klassisches Rad meldet je Rastung 100 px (Chromium) oder 120 px; ein
/// Trackpad meldet einstellige Werte.
const PRECISION_PX: f64 = 40.0;
/// Solange Ereignisse dichter als dieser Abstand aufeinander folgen, gehören
/// sie zur selben Geste und behalten deren Deutung.
///
/// Das ist der eigentliche Fix für den Nachlauf der Magic Mouse: die Geste wird
/// einmal am Anfang eingeordnet — bei kleinen Deltas also als Verschieben — und
/// bleibt dabei, auch wenn der Nachlauf einzelne große Deltas liefert.
const LATCH_MS: f64 = 200.0;
/// Ab diesem Verhältnis gilt eine Wischrichtung als gemeint und die andere als
/// Zittern.
///
/// Ein Trackpad-Wischer ist nie exakt waagerecht. Ohne diese Regel verschiebt
/// jeder Zeitwischer nebenbei die Preisachse — und sperrt sie dabei, sodass die
/// nächste Kerze den Ausschnitt nicht mehr nachführt.
const AXIS_DOMINANCE: f64 = 2.0;
/// Zoomstärke je Pixel Raddelta.
const ZOOM_PER_PIXEL: f64 = 0.0025;
/// Grenze für den Zoomfaktor eines einzelnen Ereignisses.
const MAX_ZOOM_STEP: f64 = 4.0;

/// Die laufende Geste.
#[derive(Debug, Clone, Copy)]
struct Gesture {
    kind: WheelKind,
    /// Zeitstempel des letzten Ereignisses.
    last_ts: f64,
    /// Summe der bisherigen Deltas — entscheidet die dominante Achse.
    sum_x: f64,
    sum_y: f64,
}

/// Deutet Rad-Ereignisse und hält dabei die laufende Geste fest.
#[derive(Debug, Clone, Default)]
pub struct WheelInterpreter {
    gesture: Option<Gesture>,
}

impl WheelInterpreter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deutet ein Ereignis. Gibt `None`, wenn nichts zu tun ist.
    pub fn interpret(&mut self, input: WheelInput) -> Option<WheelGesture> {
        let kind = self.classify(&input);
        let delta_x = to_pixels(input.delta_x, input.delta_mode);
        let delta_y = to_pixels(input.delta_y, input.delta_mode);

        let gesture = match self.gesture {
            Some(g) if g.kind == kind && self.continues(&input) => Gesture {
                kind,
                last_ts: input.timestamp_ms,
                sum_x: g.sum_x + delta_x,
                sum_y: g.sum_y + delta_y,
            },
            _ => Gesture {
                kind,
                last_ts: input.timestamp_ms,
                sum_x: delta_x,
                sum_y: delta_y,
            },
        };
        self.gesture = Some(gesture);

        match kind {
            WheelKind::Zoom => {
                if delta_y == 0.0 {
                    return None;
                }
                Some(WheelGesture::Zoom {
                    factor: zoom_factor(delta_y),
                    center_x: input.x,
                })
            }
            WheelKind::Precision => {
                let (delta_x, delta_y) = dominant_axis(gesture, delta_x, delta_y);
                if delta_x == 0.0 && delta_y == 0.0 {
                    return None;
                }
                Some(WheelGesture::Pan { delta_x, delta_y })
            }
        }
    }

    /// Gehört das Ereignis noch zur laufenden Geste?
    fn continues(&self, input: &WheelInput) -> bool {
        match self.gesture {
            Some(g) => input.timestamp_ms - g.last_ts < LATCH_MS && input.timestamp_ms >= g.last_ts,
            None => false,
        }
    }

    fn classify(&self, input: &WheelInput) -> WheelKind {
        // Pinch ist eindeutig und schlägt jede laufende Geste.
        if input.ctrl_key {
            return WheelKind::Zoom;
        }

        // Innerhalb einer laufenden Geste nicht umdeuten.
        if self.continues(input) {
            if let Some(g) = self.gesture {
                return g.kind;
            }
        }

        // Zeilen und Seiten meldet nur ein klassisches Rad.
        if input.delta_mode != 0 {
            return WheelKind::Zoom;
        }

        // Zwei Achsen kann ein Rad nicht.
        if input.delta_x != 0.0 {
            return WheelKind::Precision;
        }

        // Gebrochene Pixel meldet nur ein Präzisionsgerät.
        if input.delta_y.fract() != 0.0 {
            return WheelKind::Precision;
        }

        if input.delta_y.abs() < PRECISION_PX {
            WheelKind::Precision
        } else {
            WheelKind::Zoom
        }
    }
}

/// Unterdrückt die Nebenachse, wenn die Geste eindeutig in eine Richtung geht.
fn dominant_axis(gesture: Gesture, delta_x: f64, delta_y: f64) -> (f64, f64) {
    let (x, y) = (gesture.sum_x.abs(), gesture.sum_y.abs());
    if x > y * AXIS_DOMINANCE {
        (delta_x, 0.0)
    } else if y > x * AXIS_DOMINANCE {
        (0.0, delta_y)
    } else {
        (delta_x, delta_y)
    }
}

/// Rechnet ein Delta unabhängig von `deltaMode` in Pixel um.
fn to_pixels(delta: f64, delta_mode: u32) -> f64 {
    match delta_mode {
        1 => delta * LINE_HEIGHT_PX,
        2 => delta * PAGE_HEIGHT_PX,
        _ => delta,
    }
}

/// Zoomfaktor aus einem Pixeldelta — stetig, nicht in festen Stufen.
///
/// Exponentiell, damit zweimal halb so weit scrollen dasselbe ergibt wie einmal
/// ganz: `f(a) * f(b) == f(a + b)`. Genau das fehlte vorher und machte den
/// Nachlauf eines Präzisionsgeräts so zerstörerisch.
pub fn zoom_factor(delta_px: f64) -> f64 {
    (delta_px * ZOOM_PER_PIXEL)
        .exp()
        .clamp(1.0 / MAX_ZOOM_STEP, MAX_ZOOM_STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magic_mouse(delta_y: f64, ts: f64) -> WheelInput {
        WheelInput {
            x: 400.0,
            y: 200.0,
            delta_x: 0.0,
            delta_y,
            ctrl_key: false,
            delta_mode: 0,
            timestamp_ms: ts,
        }
    }

    fn classic_wheel(delta_y: f64, ts: f64) -> WheelInput {
        WheelInput {
            x: 400.0,
            y: 200.0,
            delta_x: 0.0,
            delta_y,
            ctrl_key: false,
            delta_mode: 0,
            timestamp_ms: ts,
        }
    }

    #[test]
    fn classic_wheel_zooms() {
        let mut interp = WheelInterpreter::new();
        let gesture = interp.interpret(classic_wheel(100.0, 0.0)).unwrap();
        assert!(matches!(gesture, WheelGesture::Zoom { .. }));
    }

    #[test]
    fn line_mode_zooms_even_with_a_small_delta() {
        let mut interp = WheelInterpreter::new();
        let input = WheelInput {
            delta_mode: 1,
            ..magic_mouse(3.0, 0.0)
        };
        assert!(matches!(
            interp.interpret(input),
            Some(WheelGesture::Zoom { .. })
        ));
    }

    #[test]
    fn pinch_zooms_even_with_a_tiny_delta() {
        let mut interp = WheelInterpreter::new();
        let input = WheelInput {
            ctrl_key: true,
            ..magic_mouse(2.0, 0.0)
        };
        assert!(matches!(
            interp.interpret(input),
            Some(WheelGesture::Zoom { .. })
        ));
    }

    #[test]
    fn horizontal_delta_pans() {
        let mut interp = WheelInterpreter::new();
        let input = WheelInput {
            delta_x: 12.0,
            ..magic_mouse(0.0, 0.0)
        };
        assert_eq!(
            interp.interpret(input),
            Some(WheelGesture::Pan {
                delta_x: 12.0,
                delta_y: 0.0
            })
        );
    }

    #[test]
    fn fractional_delta_pans() {
        let mut interp = WheelInterpreter::new();
        assert!(matches!(
            interp.interpret(magic_mouse(2.5, 0.0)),
            Some(WheelGesture::Pan { .. })
        ));
    }

    #[test]
    fn small_delta_pans() {
        let mut interp = WheelInterpreter::new();
        assert!(matches!(
            interp.interpret(magic_mouse(4.0, 0.0)),
            Some(WheelGesture::Pan { .. })
        ));
    }

    /// Der eigentliche Befund: der Nachlauf einer Magic Mouse darf nicht
    /// mitten in der Geste zum Zoomen umschlagen.
    #[test]
    fn momentum_stays_a_pan() {
        let mut interp = WheelInterpreter::new();
        // Anwischen: kleine Deltas.
        for step in 0..3 {
            let g = interp.interpret(magic_mouse(3.0, step as f64 * 16.0));
            assert!(matches!(g, Some(WheelGesture::Pan { .. })));
        }
        // Nachlauf: ein einzelnes großes Delta, dicht am vorigen Ereignis.
        let g = interp.interpret(magic_mouse(140.0, 48.0));
        assert!(
            matches!(g, Some(WheelGesture::Pan { .. })),
            "Nachlauf derselben Geste muss Verschieben bleiben, nicht Zoom werden"
        );
    }

    /// Der Fund aus dem Browserlauf: ein Zeitwischer ist nie exakt waagerecht,
    /// und das bisschen Zittern verschob die Preisachse — und sperrte sie.
    #[test]
    fn a_horizontal_swipe_does_not_move_the_price() {
        let mut interp = WheelInterpreter::new();
        let mut vertical_total = 0.0;

        for step in 0..20 {
            let input = WheelInput {
                delta_x: 8.0,
                ..magic_mouse(2.0, step as f64 * 16.0)
            };
            if let Some(WheelGesture::Pan { delta_y, .. }) = interp.interpret(input) {
                vertical_total += delta_y;
            }
        }

        assert_eq!(
            vertical_total, 0.0,
            "die Nebenachse wird unterdrückt, solange die Geste waagerecht bleibt"
        );
    }

    #[test]
    fn a_vertical_swipe_does_not_move_time() {
        let mut interp = WheelInterpreter::new();
        let mut horizontal_total = 0.0;

        for step in 0..20 {
            let input = WheelInput {
                delta_x: 1.0,
                ..magic_mouse(9.0, step as f64 * 16.0)
            };
            if let Some(WheelGesture::Pan { delta_x, .. }) = interp.interpret(input) {
                horizontal_total += delta_x;
            }
        }

        assert_eq!(horizontal_total, 0.0);
    }

    #[test]
    fn a_diagonal_swipe_keeps_both_axes() {
        let mut interp = WheelInterpreter::new();
        let input = WheelInput {
            delta_x: 6.0,
            ..magic_mouse(6.0, 0.0)
        };
        assert_eq!(
            interp.interpret(input),
            Some(WheelGesture::Pan {
                delta_x: 6.0,
                delta_y: 6.0
            })
        );
    }

    #[test]
    fn a_new_gesture_is_classified_afresh() {
        let mut interp = WheelInterpreter::new();
        interp.interpret(magic_mouse(3.0, 0.0));
        // Deutlich später: neue Geste, grobes Rad.
        let g = interp.interpret(classic_wheel(100.0, 5_000.0));
        assert!(matches!(g, Some(WheelGesture::Zoom { .. })));
    }

    #[test]
    fn zero_delta_does_nothing() {
        let mut interp = WheelInterpreter::new();
        assert_eq!(interp.interpret(magic_mouse(0.0, 0.0)), None);
    }

    /// Stetigkeit: zwei halbe Schritte müssen so weit zoomen wie ein ganzer.
    /// Ohne diese Eigenschaft summiert sich der Nachlauf zu einem Sprung.
    #[test]
    fn zoom_is_continuous() {
        let whole = zoom_factor(100.0);
        let halves = zoom_factor(50.0) * zoom_factor(50.0);
        assert!(
            (whole - halves).abs() < 1e-9,
            "f(100) = {whole}, f(50)² = {halves}"
        );
    }

    #[test]
    fn scrolling_down_zooms_out() {
        assert!(zoom_factor(100.0) > 1.0);
        assert!(zoom_factor(-100.0) < 1.0);
    }

    #[test]
    fn a_single_event_cannot_zoom_arbitrarily_far() {
        assert!(zoom_factor(100_000.0) <= MAX_ZOOM_STEP);
        assert!(zoom_factor(-100_000.0) >= 1.0 / MAX_ZOOM_STEP);
    }
}
