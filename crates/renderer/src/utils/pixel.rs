//! Ausrichtung auf Gerätepixel.
//!
//! Alle Maße hier laufen in **Gerätepixeln**, nicht in CSS-Pixeln. Ein Canvas, dessen
//! Kontext mit dem Pixelverhältnis skaliert ist, füllt eine Kante nur dann scharf,
//! wenn sie auf ganzen Gerätepixeln liegt; Bruchteile verwaschen sie. Das gilt für
//! Linien (ungerade Breite → Mitte auf halbem Pixel) wie für Rechtecke.

/// Rundet einen CSS-Wert auf das nächste ganze Gerätepixel (zurück in CSS-Pixeln).
pub fn snap(css: f64, pixel_ratio: f64) -> f64 {
    (css * pixel_ratio).round() / pixel_ratio
}

/// Linienbreite in ganzen Gerätepixeln, mindestens eines.
pub fn device_line_width(css_width: f64, pixel_ratio: f64) -> f64 {
    (css_width * pixel_ratio).round().max(1.0)
}

/// Mitte einer Linie von `device_width` Gerätepixeln, in CSS-Pixeln, so dass beide
/// Kanten auf Pixelgrenzen liegen: ungerade Breite → halbes Pixel, gerade → ganzes.
pub fn line_center(css: f64, pixel_ratio: f64, device_width: f64) -> f64 {
    let device = css * pixel_ratio;
    let center = if (device_width as i64) % 2 == 1 {
        device.floor() + 0.5
    } else {
        device.round()
    };
    center / pixel_ratio
}

/// Spalten einer Kerze in Gerätepixeln: Docht und Körper, beide um dieselbe Mitte.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CandleColumns {
    pub wick_left: f64,
    pub wick_width: f64,
    pub body_left: f64,
    pub body_width: f64,
}

/// Legt Docht und Körper einer Kerze auf ganze Gerätepixel.
///
/// Die Körperbreite bekommt die Parität der Dochtbreite, damit der Docht genau in
/// der Mitte sitzt — bei gerader Körperbreite und ungeradem Docht läge er sonst ein
/// halbes Pixel daneben.
pub fn candle_columns(
    center_css: f64,
    body_css_width: f64,
    wick_css_width: f64,
    pixel_ratio: f64,
) -> CandleColumns {
    let wick_width = device_line_width(wick_css_width, pixel_ratio);
    let mut body_width = (body_css_width * pixel_ratio).round().max(1.0);
    if (body_width as i64 - wick_width as i64) % 2 != 0 {
        body_width += 1.0;
    }
    let center = line_center(center_css, pixel_ratio, wick_width) * pixel_ratio;
    CandleColumns {
        wick_left: center - wick_width / 2.0,
        wick_width,
        body_left: center - body_width / 2.0,
        body_width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapping_lands_on_device_pixels() {
        assert_eq!(snap(10.26, 2.0), 10.5);
        assert_eq!(snap(10.24, 2.0), 10.0);
        assert_eq!(snap(10.4, 1.0), 10.0);
    }

    #[test]
    fn a_one_device_pixel_line_is_centred_on_a_half_pixel() {
        // Bei Verhältnis 2 ist 1 CSS-px = 2 Gerätepixel: gerade Breite, ganze Pixel.
        let w = device_line_width(1.0, 2.0);
        assert_eq!(w, 2.0);
        assert_eq!(line_center(10.3, 2.0, w) * 2.0, 21.0);
        // Bei Verhältnis 1 ist die Breite 1: halbes Pixel.
        let w = device_line_width(1.0, 1.0);
        assert_eq!(line_center(10.3, 1.0, w), 10.5);
    }

    #[test]
    fn a_line_never_vanishes() {
        assert_eq!(device_line_width(0.1, 1.0), 1.0);
    }

    #[test]
    fn the_wick_sits_in_the_middle_of_the_body() {
        for pixel_ratio in [1.0, 1.25, 1.5, 2.0, 3.0] {
            for step in 0..40 {
                let x = 100.0 + step as f64 * 0.37;
                let c = candle_columns(x, 6.0, 1.0, pixel_ratio);
                let wick_mid = c.wick_left + c.wick_width / 2.0;
                let body_mid = c.body_left + c.body_width / 2.0;
                assert_eq!(wick_mid, body_mid, "x {x} bei Verhältnis {pixel_ratio}");
                assert_eq!(c.wick_left.fract(), 0.0);
                assert_eq!(c.body_left.fract(), 0.0);
                assert_eq!(c.body_width.fract(), 0.0);
            }
        }
    }

    #[test]
    fn equal_slots_give_equal_pixel_widths() {
        let a = candle_columns(100.0, 6.0, 1.0, 2.0);
        let b = candle_columns(107.5, 6.0, 1.0, 2.0);
        assert_eq!(a.body_width, b.body_width);
    }
}
