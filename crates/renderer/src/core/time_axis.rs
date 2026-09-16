//! Beschriftung der Zeitachse auf der Bar-Achse.
//!
//! Auf einer Zeitachse konnte die Achse Ticks in gleichen Zeitabständen setzen.
//! Auf der Bar-Achse ist das nicht mehr definierbar: zwischen zwei benachbarten
//! Bars können fünf Minuten oder ein Wochenende liegen. Ticks stehen deshalb
//! **an Bars, an denen ein Kalendersprung stattfindet** — so machen es
//! Handelsplattformen, und so steht über einer Wochenendpause genau ein
//! Tageswechsel statt zweier Beschriftungen ohne Bars dazwischen.
//!
//! Siehe `plan/spezifikation/02-bar-index-achse.md` §5.

use chrono::{DateTime, Datelike, Timelike};

use crate::core::bar_index::BarIndex;
use crate::core::types::Seconds;
use crate::core::viewport::BarRange;

/// Wie grob die Achse beschriftet wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickUnit {
    /// Sprung alle n Sekunden (Minuten- und Stundenstufen).
    Span(i64),
    Day,
    Month,
    Year,
}

/// Leiter von fein nach grob. Die Achse nimmt die feinste Stufe, die noch passt.
const TICK_LADDER: &[TickUnit] = &[
    TickUnit::Span(60),
    TickUnit::Span(5 * 60),
    TickUnit::Span(15 * 60),
    TickUnit::Span(30 * 60),
    TickUnit::Span(3600),
    TickUnit::Span(3 * 3600),
    TickUnit::Span(6 * 3600),
    TickUnit::Span(12 * 3600),
    TickUnit::Day,
    TickUnit::Month,
    TickUnit::Year,
];

/// Eine Beschriftung an der Zeitachse.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisTick {
    /// Bar, an der der Sprung stattfindet.
    pub bar: usize,
    /// Zeitstempel dieser Bar (UTC).
    pub time: Seconds,
    pub label: String,
    /// Tages- oder gröberer Wechsel — verdient die ausführliche Beschriftung.
    pub major: bool,
}

/// Ticks für den sichtbaren Ausschnitt.
///
/// `min_label_px` ist der Platz, den eine Beschriftung mindestens braucht; die
/// feinste Stufe, die das einhält, gewinnt.
pub fn axis_ticks(
    index: &BarIndex,
    bars: BarRange,
    width_px: f64,
    timezone_offset_minutes: i32,
    min_label_px: f64,
) -> Vec<AxisTick> {
    if index.is_empty() || width_px <= 0.0 || bars.span() <= 0.0 {
        return Vec::new();
    }

    let first = bars.first.ceil().max(0.0) as usize;
    let last = (bars.last.floor().max(0.0) as usize).min(index.len().saturating_sub(1));
    if first > last {
        return Vec::new();
    }

    let offset = timezone_offset_minutes as i64 * 60;
    let max_ticks = (width_px / min_label_px).floor().max(1.0) as usize;

    for &unit in TICK_LADDER {
        let boundaries = boundaries_for(index, first, last, unit, offset);
        if boundaries.is_empty() {
            continue;
        }
        if boundaries.len() <= max_ticks {
            let ticks = boundaries
                .into_iter()
                .map(|bar| {
                    let time = index.index_to_time(bar).unwrap_or_default();
                    let major = is_boundary(index, bar, TickUnit::Day, offset);
                    AxisTick {
                        bar,
                        time,
                        label: format_label(time.get() + offset, unit, major),
                        major,
                    }
                })
                .collect();
            return thin_out(ticks, bars, width_px, min_label_px);
        }
    }

    // Selbst Jahre sind zu dicht (sehr weit herausgezoomt): gleichmäßig ausdünnen.
    let stride = ((last - first + 1) as f64 / max_ticks as f64)
        .ceil()
        .max(1.0) as usize;
    (first..=last)
        .step_by(stride)
        .map(|bar| {
            let time = index.index_to_time(bar).unwrap_or_default();
            AxisTick {
                bar,
                time,
                label: format_label(time.get() + offset, TickUnit::Year, true),
                major: true,
            }
        })
        .collect()
}

/// Entfernt Beschriftungen, die einander überlappen würden.
///
/// Dass die *durchschnittliche* Dichte passt, heißt nicht, dass keine zwei
/// Ticks aneinanderstoßen: direkt neben einer Handelspause liegen der letzte
/// Stundenwechsel davor und der Tageswechsel danach auf benachbarten Bars, also
/// wenige Pixel auseinander. Bei einem Zusammenstoß gewinnt der Tageswechsel —
/// er trägt mehr Information als eine weitere Uhrzeit.
fn thin_out(
    ticks: Vec<AxisTick>,
    bars: BarRange,
    width_px: f64,
    min_label_px: f64,
) -> Vec<AxisTick> {
    let span = bars.span();
    if span <= 0.0 {
        return ticks;
    }
    let x_of = |tick: &AxisTick| (tick.bar as f64 - bars.first) / span * width_px;

    let mut kept: Vec<AxisTick> = Vec::with_capacity(ticks.len());
    for tick in ticks {
        match kept.last() {
            Some(previous) if x_of(&tick) - x_of(previous) < min_label_px => {
                if tick.major && !previous.major {
                    kept.pop();
                    kept.push(tick);
                }
            }
            _ => kept.push(tick),
        }
    }
    kept
}

/// Bars, an denen `unit` gegenüber der Vorgängerbar springt.
fn boundaries_for(
    index: &BarIndex,
    first: usize,
    last: usize,
    unit: TickUnit,
    offset: i64,
) -> Vec<usize> {
    (first..=last)
        .filter(|&bar| is_boundary(index, bar, unit, offset))
        .collect()
}

/// Springt an dieser Bar die angegebene Einheit?
///
/// Die allererste Bar zählt nicht als Sprung — sonst stünde links immer eine
/// Beschriftung, egal wo der Ausschnitt gerade liegt.
fn is_boundary(index: &BarIndex, bar: usize, unit: TickUnit, offset: i64) -> bool {
    let Some(time) = index.index_to_time(bar) else {
        return false;
    };
    let Some(previous) = bar.checked_sub(1).and_then(|b| index.index_to_time(b)) else {
        return false;
    };

    bucket(time.get() + offset, unit) != bucket(previous.get() + offset, unit)
}

/// Kennzahl des Zeitabschnitts, in dem ein lokaler Zeitstempel liegt.
fn bucket(local: i64, unit: TickUnit) -> i64 {
    match unit {
        TickUnit::Span(secs) => local.div_euclid(secs),
        TickUnit::Day => local.div_euclid(86_400),
        TickUnit::Month => match DateTime::from_timestamp(local, 0) {
            Some(dt) => dt.year() as i64 * 12 + dt.month() as i64,
            None => 0,
        },
        TickUnit::Year => match DateTime::from_timestamp(local, 0) {
            Some(dt) => dt.year() as i64,
            None => 0,
        },
    }
}

/// Beschriftung eines lokalen Zeitstempels für die gewählte Stufe.
fn format_label(local: i64, unit: TickUnit, major: bool) -> String {
    let Some(dt) = DateTime::from_timestamp(local, 0) else {
        return local.to_string();
    };

    match unit {
        // Ein Tageswechsel innerhalb einer Stundenstufe bekommt das Datum —
        // sonst stünde dort ein nichtssagendes "00:00".
        TickUnit::Span(_) if major => dt.format("%m-%d").to_string(),
        TickUnit::Span(_) => format!("{:02}:{:02}", dt.hour(), dt.minute()),
        TickUnit::Day => dt.format("%m-%d").to_string(),
        TickUnit::Month => dt.format("%Y-%m").to_string(),
        TickUnit::Year => dt.year().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Candle;

    const H1: i64 = 3600;
    /// 2026-09-11 00:00:00 UTC, ein Freitag.
    const FRIDAY_MIDNIGHT: i64 = 1_789_084_800;

    fn index_of(times: &[i64], duration: i64) -> BarIndex {
        let candles: Vec<Candle> = times
            .iter()
            .map(|&t| Candle::new(Seconds::new(t), 1.0, 2.0, 0.5, 1.5, 1.0))
            .collect();
        BarIndex::from_candles(&candles, duration)
    }

    fn all_bars(index: &BarIndex) -> BarRange {
        BarRange {
            first: -0.5,
            last: index.len() as f64 - 0.5,
        }
    }

    #[test]
    fn an_empty_index_has_no_ticks() {
        let index = BarIndex::empty(H1);
        assert!(axis_ticks(&index, all_bars(&index), 800.0, 0, 60.0).is_empty());
    }

    /// Tageswechsel — die Ticks, die auf jeder Stufe als `major` markiert sind.
    fn day_changes(ticks: &[AxisTick]) -> Vec<usize> {
        ticks.iter().filter(|t| t.major).map(|t| t.bar).collect()
    }

    #[test]
    fn hourly_bars_across_two_days_get_one_day_tick() {
        // 48 Stundenbars ab Mitternacht: genau ein Tageswechsel.
        let times: Vec<i64> = (0..48).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let index = index_of(&times, H1);

        let ticks = axis_ticks(&index, all_bars(&index), 400.0, 0, 60.0);

        assert_eq!(day_changes(&ticks), vec![24], "{ticks:?}");
    }

    /// Der Test aus `plan/spezifikation/02-bar-index-achse.md` §5: über einer Pause steht
    /// **ein** Tageswechsel, nicht zwei.
    ///
    /// Auf der Zeitachse hätten Samstag und Sonntag je eine Beschriftung
    /// bekommen — mit Datumsangaben, zu denen es gar keine Bars gibt.
    #[test]
    fn a_weekend_break_gets_exactly_one_day_tick() {
        // Freitag 00:00 bis 23:00, dann 49 Stunden Pause, dann Montag 00:00–23:00.
        let mut times: Vec<i64> = (0..24).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let friday_close = *times.last().unwrap();
        times.extend((0..24).map(|i| friday_close + 49 * H1 + i * H1));
        let index = index_of(&times, H1);

        let ticks = axis_ticks(&index, all_bars(&index), 400.0, 0, 60.0);

        assert_eq!(
            day_changes(&ticks),
            vec![24],
            "die Pause überspringt Samstag und Sonntag, also ein einziger Wechsel \
             auf der ersten Bar danach: {ticks:?}"
        );
    }

    #[test]
    fn a_timezone_offset_moves_the_day_boundary() {
        let times: Vec<i64> = (0..48).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let index = index_of(&times, H1);

        let utc = axis_ticks(&index, all_bars(&index), 400.0, 0, 60.0);
        // UTC+2: der Tageswechsel liegt zwei Bars früher.
        let berlin = axis_ticks(&index, all_bars(&index), 400.0, 120, 60.0);

        assert_eq!(day_changes(&utc), vec![24]);
        assert_eq!(
            day_changes(&berlin),
            vec![22, 46],
            "UTC+2 verschiebt jeden Tageswechsel um zwei Stundenbars nach vorn — \
             und schiebt dadurch einen zweiten in den Ausschnitt"
        );
    }

    #[test]
    fn a_narrow_axis_falls_back_to_a_coarser_unit() {
        let times: Vec<i64> = (0..48).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let index = index_of(&times, H1);

        let wide = axis_ticks(&index, all_bars(&index), 2000.0, 0, 60.0);
        let narrow = axis_ticks(&index, all_bars(&index), 120.0, 0, 60.0);

        assert!(
            wide.len() >= narrow.len(),
            "mehr Platz erlaubt eine feinere Stufe: {} gegen {}",
            wide.len(),
            narrow.len()
        );
        assert!(narrow.len() <= 2, "auf 120 px passen höchstens zwei Labels");
    }

    /// Genau der Fall aus dem Browserlauf: die letzte Stunde vor einer Pause und
    /// der Tageswechsel danach liegen auf benachbarten Bars — ihre
    /// Beschriftungen überlappten sichtbar.
    #[test]
    fn labels_on_neighbouring_bars_do_not_collide() {
        // 200 Stundenbars, dann 49 Stunden Pause, dann weiter.
        let mut times: Vec<i64> = (0..200).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let close = *times.last().unwrap();
        times.extend((1..=50).map(|i| close + 49 * H1 + i * H1));
        let index = index_of(&times, H1);

        let bars = BarRange {
            first: 180.0,
            last: 220.0,
        };
        let width = 800.0;
        let min_label = 64.0;
        let ticks = axis_ticks(&index, bars, width, 0, min_label);

        let x_of = |tick: &AxisTick| (tick.bar as f64 - bars.first) / bars.span() * width;
        for pair in ticks.windows(2) {
            let distance = x_of(&pair[1]) - x_of(&pair[0]);
            assert!(
                distance >= min_label,
                "{:?} und {:?} liegen nur {distance:.1} px auseinander",
                pair[0].label,
                pair[1].label
            );
        }
    }

    /// Beim Zusammenstoß überlebt der Tageswechsel, nicht die Uhrzeit.
    #[test]
    fn a_day_change_wins_against_a_neighbouring_hour() {
        let mut times: Vec<i64> = (0..200).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let close = *times.last().unwrap();
        times.extend((1..=50).map(|i| close + 49 * H1 + i * H1));
        let index = index_of(&times, H1);

        let ticks = axis_ticks(
            &index,
            BarRange {
                first: 180.0,
                last: 220.0,
            },
            800.0,
            0,
            64.0,
        );

        assert!(
            ticks.iter().any(|t| t.major),
            "der Tageswechsel nach der Pause muss stehen bleiben: {ticks:?}"
        );
    }

    #[test]
    fn ticks_stay_inside_the_visible_range() {
        let times: Vec<i64> = (0..96).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let index = index_of(&times, H1);

        let ticks = axis_ticks(
            &index,
            BarRange {
                first: 30.0,
                last: 60.0,
            },
            800.0,
            0,
            60.0,
        );

        assert!(!ticks.is_empty());
        assert!(
            ticks.iter().all(|t| t.bar >= 30 && t.bar <= 60),
            "{ticks:?}"
        );
    }

    #[test]
    fn an_hour_tick_on_a_day_change_shows_the_date() {
        let times: Vec<i64> = (0..30).map(|i| FRIDAY_MIDNIGHT + i * H1).collect();
        let index = index_of(&times, H1);

        let ticks = axis_ticks(&index, all_bars(&index), 2000.0, 0, 60.0);
        let day_tick = ticks.iter().find(|t| t.major).expect("ein Tageswechsel");

        assert!(
            day_tick.label.contains('-'),
            "Tageswechsel wird als Datum beschriftet, nicht als 00:00: {}",
            day_tick.label
        );
    }
}
