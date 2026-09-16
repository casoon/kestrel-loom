//! Die Zeitleiste am unteren Rand — Ausschnitt zeigen und greifen.
//!
//! Auf der Bar-Achse ist der sichtbare Ausschnitt ein Bereich von Bar-Indizes,
//! und die Gesamtmenge der Bars ist bekannt. Damit lässt sich beides exakt
//! anzeigen: wie viel vom Bestand man gerade sieht und wo. Auf der alten
//! Zeitachse hätte derselbe Balken Handelspausen mitgemessen und einen
//! Ausschnitt vorgetäuscht, der größer ist als er ist.
//!
//! Der Balken ist nicht nur Anzeige: der Griff lässt sich ziehen (verschieben),
//! seine Ränder ebenso (zoomen), und ein Klick daneben blättert um eine
//! Bildbreite.

use crate::core::viewport::BarRange;

/// Höhe des Balkens in CSS-Pixeln.
pub const SCROLLBAR_HEIGHT: f64 = 10.0;
/// So schmal darf der Griff nie werden, sonst ist er nicht mehr zu treffen.
const MIN_THUMB_PX: f64 = 28.0;
/// Greifzone an den Griffrändern zum Zoomen.
const EDGE_GRAB_PX: f64 = 6.0;

/// Lage und Maße des Balkens auf der Zeichenfläche.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollbarGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Der Griff: der Teil des Balkens, der den sichtbaren Ausschnitt darstellt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thumb {
    pub x: f64,
    pub width: f64,
}

/// Was der Zeiger auf dem Balken trifft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbarHit {
    /// Griffmitte — ziehen verschiebt.
    Body,
    /// Linker Griffrand — ziehen zoomt.
    Start,
    /// Rechter Griffrand — ziehen zoomt.
    End,
    /// Bahn links vom Griff — eine Bildbreite zurück.
    TrackBefore,
    /// Bahn rechts vom Griff — eine Bildbreite vor.
    TrackAfter,
}

impl ScrollbarHit {
    /// Die Cursor-Form, die diese Trefferzone anzeigt.
    ///
    /// Der Kern zeichnet nur; die Einbindung setzt daraus `canvas.style.cursor`.
    pub fn cursor(self) -> &'static str {
        match self {
            ScrollbarHit::Body => "grab",
            ScrollbarHit::Start | ScrollbarHit::End => "ew-resize",
            ScrollbarHit::TrackBefore | ScrollbarHit::TrackAfter => "pointer",
        }
    }
}

impl ScrollbarGeometry {
    /// Der darstellbare Bar-Bereich: der Datenbestand, erweitert um den
    /// Ausschnitt, falls über den Rand hinaus verschoben wurde.
    ///
    /// Ohne diese Erweiterung liefe der Griff aus der Bahn heraus, sobald man
    /// in den Leerraum rechts vom letzten Bar zieht.
    pub fn domain(bars: BarRange, bar_count: usize) -> (f64, f64) {
        let data_first = -0.5;
        let data_last = bar_count as f64 - 0.5;
        (
            bars.first.min(data_first),
            bars.last
                .max(data_last)
                .max(bars.first.min(data_first) + 1.0),
        )
    }

    /// Griffposition für einen Ausschnitt.
    pub fn thumb(&self, bars: BarRange, bar_count: usize) -> Thumb {
        let (start, end) = Self::domain(bars, bar_count);
        let span = (end - start).max(1e-9);

        let left = self.x + (bars.first - start) / span * self.width;
        let right = self.x + (bars.last - start) / span * self.width;

        let width = (right - left).clamp(MIN_THUMB_PX.min(self.width), self.width);
        let x = left.clamp(self.x, self.x + self.width - width);

        Thumb { x, width }
    }

    /// Bar-Position unter einer Bildschirm-x-Koordinate auf der Bahn.
    pub fn bar_at(&self, x: f64, bars: BarRange, bar_count: usize) -> f64 {
        let (start, end) = Self::domain(bars, bar_count);
        if self.width <= 0.0 {
            return start;
        }
        start + (x - self.x) / self.width * (end - start)
    }

    /// Was liegt unter dem Zeiger?
    pub fn hit(&self, bars: BarRange, bar_count: usize, x: f64, y: f64) -> Option<ScrollbarHit> {
        if y < self.y || y > self.y + self.height || x < self.x || x > self.x + self.width {
            return None;
        }

        let thumb = self.thumb(bars, bar_count);
        // Die Greifzonen dürfen sich nicht überlappen — bei einem schmalen
        // Griff gewinnt sonst der Rand und Verschieben wird unmöglich.
        let edge = EDGE_GRAB_PX.min(thumb.width / 3.0);

        if x < thumb.x {
            return Some(ScrollbarHit::TrackBefore);
        }
        if x > thumb.x + thumb.width {
            return Some(ScrollbarHit::TrackAfter);
        }
        if x < thumb.x + edge {
            return Some(ScrollbarHit::Start);
        }
        if x > thumb.x + thumb.width - edge {
            return Some(ScrollbarHit::End);
        }
        Some(ScrollbarHit::Body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar() -> ScrollbarGeometry {
        ScrollbarGeometry {
            x: 0.0,
            y: 380.0,
            width: 800.0,
            height: SCROLLBAR_HEIGHT,
        }
    }

    fn view(first: f64, last: f64) -> BarRange {
        BarRange { first, last }
    }

    #[test]
    fn the_whole_dataset_fills_the_track() {
        let thumb = bar().thumb(view(-0.5, 99.5), 100);
        assert!((thumb.x - 0.0).abs() < 1e-6);
        assert!((thumb.width - 800.0).abs() < 1e-6);
    }

    #[test]
    fn a_tenth_of_the_data_fills_a_tenth_of_the_track() {
        let thumb = bar().thumb(view(-0.5, 9.5), 100);
        assert!((thumb.width - 80.0).abs() < 1e-6, "{thumb:?}");
        assert!((thumb.x - 0.0).abs() < 1e-6);
    }

    #[test]
    fn the_thumb_sits_where_the_view_sits() {
        let thumb = bar().thumb(view(49.5, 59.5), 100);
        assert!((thumb.x - 400.0).abs() < 1e-6, "{thumb:?}");
    }

    /// Bei sehr vielen Bars würde der Griff auf Bruchteile eines Pixels
    /// schrumpfen und wäre nicht mehr zu treffen.
    #[test]
    fn a_tiny_view_still_gets_a_grabbable_thumb() {
        let thumb = bar().thumb(view(0.0, 5.0), 100_000);
        assert!(thumb.width >= MIN_THUMB_PX, "{thumb:?}");
        assert!(thumb.x + thumb.width <= 800.0 + 1e-6);
    }

    /// Wer in den Leerraum rechts zieht, darf den Griff nicht aus der Bahn
    /// schieben.
    #[test]
    fn panning_past_the_last_bar_keeps_the_thumb_inside() {
        let thumb = bar().thumb(view(120.0, 140.0), 100);
        assert!(thumb.x >= 0.0);
        assert!(thumb.x + thumb.width <= 800.0 + 1e-6, "{thumb:?}");
    }

    #[test]
    fn a_click_beside_the_track_hits_nothing() {
        assert_eq!(bar().hit(view(-0.5, 99.5), 100, 400.0, 100.0), None);
        assert_eq!(bar().hit(view(-0.5, 99.5), 100, -5.0, 385.0), None);
    }

    #[test]
    fn the_middle_of_the_thumb_is_for_dragging() {
        assert_eq!(
            bar().hit(view(49.5, 59.5), 100, 440.0, 385.0),
            Some(ScrollbarHit::Body)
        );
    }

    #[test]
    fn the_thumb_edges_are_for_zooming() {
        let thumb = bar().thumb(view(49.5, 59.5), 100);
        assert_eq!(
            bar().hit(view(49.5, 59.5), 100, thumb.x + 1.0, 385.0),
            Some(ScrollbarHit::Start)
        );
        assert_eq!(
            bar().hit(view(49.5, 59.5), 100, thumb.x + thumb.width - 1.0, 385.0),
            Some(ScrollbarHit::End)
        );
    }

    #[test]
    fn the_track_beside_the_thumb_pages() {
        assert_eq!(
            bar().hit(view(49.5, 59.5), 100, 10.0, 385.0),
            Some(ScrollbarHit::TrackBefore)
        );
        assert_eq!(
            bar().hit(view(49.5, 59.5), 100, 790.0, 385.0),
            Some(ScrollbarHit::TrackAfter)
        );
    }

    /// Bei einem schmalen Griff dürfen die Zoom-Ränder nicht die ganze Fläche
    /// beanspruchen — sonst ließe er sich nicht mehr verschieben.
    #[test]
    fn a_narrow_thumb_keeps_a_draggable_middle() {
        let bars = view(0.0, 5.0);
        let thumb = bar().thumb(bars, 100_000);
        let middle = thumb.x + thumb.width / 2.0;
        assert_eq!(
            bar().hit(bars, 100_000, middle, 385.0),
            Some(ScrollbarHit::Body)
        );
    }

    #[test]
    fn x_maps_back_to_a_bar_position() {
        let geometry = bar();
        let bars = view(-0.5, 99.5);
        assert!((geometry.bar_at(0.0, bars, 100) - -0.5).abs() < 1e-6);
        assert!((geometry.bar_at(800.0, bars, 100) - 99.5).abs() < 1e-6);
        assert!((geometry.bar_at(400.0, bars, 100) - 49.5).abs() < 1e-6);
    }

    #[test]
    fn each_hit_zone_names_its_cursor() {
        assert_eq!(ScrollbarHit::Body.cursor(), "grab");
        assert_eq!(ScrollbarHit::Start.cursor(), "ew-resize");
        assert_eq!(ScrollbarHit::End.cursor(), "ew-resize");
        assert_eq!(ScrollbarHit::TrackBefore.cursor(), "pointer");
        assert_eq!(ScrollbarHit::TrackAfter.cursor(), "pointer");
    }
}
