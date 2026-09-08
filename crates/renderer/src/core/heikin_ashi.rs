//! Heikin-Ashi-Transformation.
//!
//! Eine abgeleitete Serie wie Renko: die Kerzen werden vor dem Zeichnen
//! umgerechnet, der Renderpfad bleibt derselbe.
//!
//! `loomchart` warb mit Heikin Ashi als Darstellung, hatte sie aber nie
//! implementiert (siehe `plan/04-befunde.md` B5).

use super::types::Candle;

/// Rechnet OHLCV-Kerzen in Heikin-Ashi-Kerzen um.
///
/// - `c` = Mittel aus Open, High, Low und Close der Rohkerze
/// - `o` = Mittel aus vorherigem Heikin-Ashi-Open und -Close; die erste Kerze
///   verankert auf `(o + c) / 2`
/// - `h` / `l` = Extremwert aus Rohkerze und den beiden Heikin-Ashi-Werten
///
/// Zeitstempel und Volumen bleiben unverändert; die Serie ist so lang wie die
/// Eingabe.
pub fn compute_heikin_ashi(candles: &[Candle]) -> Vec<Candle> {
    let mut out: Vec<Candle> = Vec::with_capacity(candles.len());
    let mut prev: Option<(f64, f64)> = None; // (open, close)

    for candle in candles {
        let close = (candle.o + candle.h + candle.l + candle.c) / 4.0;
        let open = match prev {
            Some((prev_open, prev_close)) => (prev_open + prev_close) / 2.0,
            None => (candle.o + candle.c) / 2.0,
        };
        let high = candle.h.max(open).max(close);
        let low = candle.l.min(open).min(close);

        out.push(Candle {
            time: candle.time,
            o: open,
            h: high,
            l: low,
            c: close,
            v: candle.v,
        });
        prev = Some((open, close));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(time: i64, o: f64, h: f64, l: f64, c: f64) -> Candle {
        Candle {
            time,
            o,
            h,
            l,
            c,
            v: 1.0,
        }
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(compute_heikin_ashi(&[]).is_empty());
    }

    #[test]
    fn first_candle_anchors_on_open_and_close() {
        let out = compute_heikin_ashi(&[candle(0, 10.0, 12.0, 9.0, 11.0)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].o, 10.5); // (10 + 11) / 2
        assert_eq!(out[0].c, 10.5); // (10 + 12 + 9 + 11) / 4
    }

    #[test]
    fn open_follows_the_previous_heikin_ashi_candle() {
        let raw = vec![
            candle(0, 10.0, 12.0, 9.0, 11.0),
            candle(1, 11.0, 14.0, 10.0, 13.0),
        ];
        let out = compute_heikin_ashi(&raw);

        // Zweite Kerze: Open = Mittel aus Open und Close der ersten HA-Kerze.
        assert_eq!(out[1].o, (out[0].o + out[0].c) / 2.0);
        assert_eq!(out[1].c, (11.0 + 14.0 + 10.0 + 13.0) / 4.0);
    }

    #[test]
    fn range_contains_open_and_close() {
        let raw = vec![
            candle(0, 10.0, 12.0, 9.0, 11.0),
            candle(1, 11.0, 14.0, 10.0, 13.0),
            candle(2, 13.0, 13.5, 8.0, 8.5),
        ];
        for ha in compute_heikin_ashi(&raw) {
            assert!(ha.h >= ha.o.max(ha.c), "High deckt den Körper");
            assert!(ha.l <= ha.o.min(ha.c), "Low deckt den Körper");
            assert!(ha.h >= ha.l);
        }
    }

    #[test]
    fn the_series_keeps_its_length_and_timestamps() {
        let raw: Vec<Candle> = (0..20)
            .map(|i| candle(i, 10.0 + i as f64, 12.0, 9.0, 11.0))
            .collect();
        let out = compute_heikin_ashi(&raw);
        assert_eq!(out.len(), raw.len());
        assert!(out.iter().zip(&raw).all(|(a, b)| a.time == b.time));
    }
}
