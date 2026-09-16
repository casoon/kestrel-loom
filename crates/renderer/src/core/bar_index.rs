//! Logische Bar-Indizes — die Grundlage der sitzungsstetigen Achse.
//!
//! Eine Zeitachse gibt einer 49-Stunden-Wochenendpause denselben Pixelanteil wie
//! 49 Handelsstunden: im Chart klafft eine Lücke, und die Preislinie zieht
//! sichtbar darüber hinweg. Handelsplattformen positionieren deshalb nach
//! **Bar-Index**: jede Bar ist gleich breit, Pausen existieren nicht.
//!
//! `BarIndex` ist die Übersetzung zwischen beiden Welten. Nach außen (Werkzeuge,
//! Export, Crosshair, Achsenbeschriftung) bleibt alles bei Unix-Sekunden;
//! gerechnet wird in Indizes.
//!
//! Siehe `plan/spezifikation/02-bar-index-achse.md`.

use crate::core::Candle;

/// Zeitstempel in Indexreihenfolge, plus die Bar-Dauer für alles außerhalb.
#[derive(Debug, Clone, Default)]
pub struct BarIndex {
    /// Aufsteigend sortiert; die Position **ist** der logische Index.
    times: Vec<i64>,
    /// Dauer einer Bar in Sekunden — nur zum Extrapolieren jenseits der Daten.
    bar_duration_secs: i64,
}

impl BarIndex {
    /// Baut den Index aus einem sortierten Kerzensatz.
    pub fn from_candles(candles: &[Candle], bar_duration_secs: i64) -> Self {
        Self {
            times: candles.iter().map(|c| c.time).collect(),
            bar_duration_secs: bar_duration_secs.max(1),
        }
    }

    /// Leerer Index mit bekannter Bar-Dauer.
    pub fn empty(bar_duration_secs: i64) -> Self {
        Self {
            times: Vec::new(),
            bar_duration_secs: bar_duration_secs.max(1),
        }
    }

    /// Setzt die Bar-Dauer (Zeitrahmenwechsel).
    pub fn set_bar_duration(&mut self, bar_duration_secs: i64) {
        self.bar_duration_secs = bar_duration_secs.max(1);
    }

    pub fn bar_duration_secs(&self) -> i64 {
        self.bar_duration_secs
    }

    /// Hängt eine Bar an oder ersetzt die letzte, wenn der Zeitstempel gleich ist.
    ///
    /// Der Weg für den Live-Betrieb: `from_candles` bei jeder eintreffenden Bar
    /// wäre eine O(n)-Kopie je Tick.
    pub fn push_bar(&mut self, time: i64) {
        match self.times.last() {
            Some(&last) if last == time => {}
            Some(&last) if last > time => {
                // Rückläufiger Zeitstempel: der Satz ist nicht mehr sortiert,
                // also neu aufbauen statt eine kaputte Ordnung zu behalten.
                self.times.push(time);
                self.times.sort_unstable();
                self.times.dedup();
            }
            _ => self.times.push(time),
        }
    }

    /// Exakter Index eines Zeitstempels.
    pub fn time_to_index(&self, time: i64) -> Option<usize> {
        self.times.binary_search(&time).ok()
    }

    /// Zeitstempel eines Index — O(1).
    pub fn index_to_time(&self, index: usize) -> Option<i64> {
        self.times.get(index).copied()
    }

    pub fn len(&self) -> usize {
        self.times.len()
    }

    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Erster und letzter Zeitstempel.
    pub fn span(&self) -> Option<(i64, i64)> {
        match (self.times.first(), self.times.last()) {
            (Some(&a), Some(&b)) => Some((a, b)),
            _ => None,
        }
    }

    /// Sichtbarer Bar-Bereich (einschließlich Start, ausschließlich Ende) für ein
    /// Zeitfenster.
    ///
    /// Gibt `(0, 0)`, wenn keine Bar im Fenster liegt.
    pub fn visible_range(&self, start: i64, end: i64) -> (usize, usize) {
        let first = self.times.partition_point(|&t| t < start);
        let last = self.times.partition_point(|&t| t <= end);
        if first >= last {
            (0, 0)
        } else {
            (first, last)
        }
    }

    /// Fraktionaler Index eines Zeitstempels — **die Funktion, an der alles hängt.**
    ///
    /// - Zeit liegt auf einer Bar → deren Index.
    /// - Zeit liegt *zwischen* zwei Bars → linear zwischen den Nachbarindizes.
    ///   Hier verschwindet die Pause: jeder Zeitpunkt im Wochenende bildet auf das
    ///   Intervall `[freitag, montag]` ab, also auf **eine** Bar-Breite statt auf
    ///   49 Stunden.
    /// - Zeit liegt außerhalb → über die Bar-Dauer extrapoliert, damit Werkzeuge
    ///   und die Zeichenfläche rechts vom letzten Bar weiter funktionieren.
    ///
    /// Ohne Daten ist der Index leer; dann bleibt nur die Extrapolation ab 0.
    pub fn time_to_fractional_index(&self, time: i64) -> f64 {
        let n = self.times.len();
        if n == 0 {
            return 0.0;
        }

        // Erste Position mit times[pos] >= time
        let pos = self.times.partition_point(|&t| t < time);

        if pos == n {
            let last = self.times[n - 1];
            return (n - 1) as f64 + (time - last) as f64 / self.bar_duration_secs as f64;
        }
        if self.times[pos] == time {
            return pos as f64;
        }
        if pos == 0 {
            let first = self.times[0];
            return (time - first) as f64 / self.bar_duration_secs as f64;
        }

        let before = self.times[pos - 1];
        let after = self.times[pos];
        let width = (after - before) as f64;
        (pos - 1) as f64 + (time - before) as f64 / width
    }

    /// Gegenrichtung zu [`Self::time_to_fractional_index`] — für Crosshair,
    /// Achsenbeschriftung und Export.
    pub fn fractional_index_to_time(&self, index: f64) -> i64 {
        let n = self.times.len();
        if n == 0 {
            return 0;
        }

        if index <= 0.0 {
            return self.times[0] + (index * self.bar_duration_secs as f64).round() as i64;
        }
        let last_index = (n - 1) as f64;
        if index >= last_index {
            return self.times[n - 1]
                + ((index - last_index) * self.bar_duration_secs as f64).round() as i64;
        }

        let floor = index.floor();
        let frac = index - floor;
        let i = floor as usize;
        let before = self.times[i];
        let after = self.times[i + 1];
        before + ((after - before) as f64 * frac).round() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H1: i64 = 3600;

    fn index_from(times: &[i64], duration: i64) -> BarIndex {
        let candles: Vec<Candle> = times
            .iter()
            .map(|&t| Candle::new(t, 1.0, 2.0, 0.5, 1.5, 100.0))
            .collect();
        BarIndex::from_candles(&candles, duration)
    }

    /// Ein Handelstag Mo–Fr, dann 49 Stunden Pause, dann geht es weiter.
    fn weekend_index() -> BarIndex {
        let mut times: Vec<i64> = (0..10).map(|i| i * H1).collect();
        let friday_close = times[9];
        times.extend((1..=5).map(|i| friday_close + 49 * H1 + i * H1));
        index_from(&times, H1)
    }

    #[test]
    fn an_empty_index_has_no_positions() {
        let idx = BarIndex::empty(H1);
        assert!(idx.is_empty());
        assert_eq!(idx.time_to_index(0), None);
        assert_eq!(idx.index_to_time(0), None);
        assert_eq!(idx.visible_range(0, 1000), (0, 0));
        assert_eq!(idx.span(), None);
    }

    #[test]
    fn exact_timestamps_map_to_their_position() {
        let idx = index_from(&[0, 300, 600, 900, 1200], 300);
        assert_eq!(idx.time_to_index(0), Some(0));
        assert_eq!(idx.time_to_index(900), Some(3));
        assert_eq!(idx.time_to_index(500), None);
        assert_eq!(idx.index_to_time(4), Some(1200));
        assert_eq!(idx.index_to_time(5), None);
    }

    #[test]
    fn a_timestamp_on_a_bar_has_an_integer_index() {
        let idx = index_from(&[0, 300, 600], 300);
        assert_eq!(idx.time_to_fractional_index(300), 1.0);
    }

    #[test]
    fn a_timestamp_between_bars_interpolates() {
        let idx = index_from(&[0, 300, 600], 300);
        assert_eq!(idx.time_to_fractional_index(150), 0.5);
        assert_eq!(idx.time_to_fractional_index(450), 1.5);
    }

    #[test]
    fn a_timestamp_beyond_the_last_bar_extrapolates() {
        let idx = index_from(&[0, 300, 600], 300);
        assert_eq!(idx.time_to_fractional_index(900), 3.0);
        assert_eq!(idx.time_to_fractional_index(1050), 3.5);
    }

    #[test]
    fn a_timestamp_before_the_first_bar_is_negative() {
        let idx = index_from(&[1000, 1300, 1600], 300);
        assert_eq!(idx.time_to_fractional_index(700), -1.0);
    }

    /// Der Kern des ganzen Umbaus: eine 49-Stunden-Pause darf genau eine
    /// Bar-Breite einnehmen, nicht 49.
    #[test]
    fn a_trading_break_is_exactly_one_bar_wide() {
        let idx = weekend_index();

        let friday = idx.time_to_fractional_index(9 * H1);
        let monday = idx.time_to_fractional_index(9 * H1 + 50 * H1);
        assert_eq!(friday, 9.0);
        assert_eq!(monday, 10.0);
        assert_eq!(
            monday - friday,
            1.0,
            "über die Pause hinweg liegt genau eine Bar-Breite"
        );

        // Mitten im Wochenende: auf halber Strecke zwischen den beiden Bars.
        let saturday = idx.time_to_fractional_index(9 * H1 + 25 * H1);
        assert!(
            (saturday - 9.5).abs() < 0.02,
            "Mitte der Pause liegt in der Mitte der einen Bar-Breite, war {saturday}"
        );
    }

    #[test]
    fn time_survives_the_round_trip() {
        let idx = weekend_index();
        for &time in &[0, 3 * H1, 9 * H1, 9 * H1 + 50 * H1, 9 * H1 + 54 * H1] {
            let f = idx.time_to_fractional_index(time);
            assert_eq!(
                idx.fractional_index_to_time(f),
                time,
                "Rundgang für {time} über Index {f}"
            );
        }
    }

    #[test]
    fn the_round_trip_also_holds_outside_the_data() {
        let idx = index_from(&[1000, 1300, 1600], 300);
        for &time in &[100, 400, 1900, 3000] {
            let f = idx.time_to_fractional_index(time);
            assert_eq!(idx.fractional_index_to_time(f), time);
        }
    }

    #[test]
    fn visible_range_covers_the_window() {
        let idx = index_from(&[0, 300, 600, 900, 1200], 300);
        assert_eq!(idx.visible_range(0, 1200), (0, 5));
        assert_eq!(idx.visible_range(300, 900), (1, 4));
        assert_eq!(idx.visible_range(5000, 9000), (0, 0));
    }

    #[test]
    fn a_new_bar_appends_in_place() {
        let mut idx = index_from(&[0, 300], 300);
        idx.push_bar(600);
        assert_eq!(idx.len(), 3);
        assert_eq!(idx.time_to_fractional_index(600), 2.0);
    }

    #[test]
    fn repeating_the_last_bar_does_not_grow_the_index() {
        let mut idx = index_from(&[0, 300], 300);
        idx.push_bar(300);
        assert_eq!(
            idx.len(),
            2,
            "eine laufende Bar wird aktualisiert, nicht angehängt"
        );
    }

    #[test]
    fn an_out_of_order_bar_restores_the_ordering() {
        let mut idx = index_from(&[0, 600], 300);
        idx.push_bar(300);
        assert_eq!(idx.index_to_time(1), Some(300));
        assert_eq!(idx.index_to_time(2), Some(600));
    }
}
