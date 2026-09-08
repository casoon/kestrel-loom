// Core types for kestrel-loom

use serde::{Deserialize, Serialize};

/// OHLCV Candle - core data type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Candle {
    /// Unix timestamp in seconds (not milliseconds!)
    pub time: i64,
    /// Open price
    pub o: f64,
    /// High price
    pub h: f64,
    /// Low price
    pub l: f64,
    /// Close price
    pub c: f64,
    /// Volume
    pub v: f64,
}

impl Candle {
    pub fn new(time: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> Self {
        Self {
            time,
            o,
            h,
            l,
            c,
            v,
        }
    }

    /// Get typical price (HLC/3)
    pub fn typical_price(&self) -> f64 {
        (self.h + self.l + self.c) / 3.0
    }

    /// Get median price (HL/2)
    pub fn median_price(&self) -> f64 {
        (self.h + self.l) / 2.0
    }

    /// Get weighted close (HLCC/4)
    pub fn weighted_close(&self) -> f64 {
        (self.h + self.l + self.c + self.c) / 4.0
    }

    /// Check if candle is bullish
    pub fn is_bullish(&self) -> bool {
        self.c > self.o
    }

    /// Check if candle is bearish
    pub fn is_bearish(&self) -> bool {
        self.c < self.o
    }

    /// Get body size (absolute)
    pub fn body_size(&self) -> f64 {
        (self.c - self.o).abs()
    }

    /// Get range (high - low)
    pub fn range(&self) -> f64 {
        self.h - self.l
    }

    /// Get upper wick size
    pub fn upper_wick(&self) -> f64 {
        if self.is_bullish() {
            self.h - self.c
        } else {
            self.h - self.o
        }
    }

    /// Get lower wick size
    pub fn lower_wick(&self) -> f64 {
        if self.is_bullish() {
            self.o - self.l
        } else {
            self.c - self.l
        }
    }

    /// Liegt der Punkt (x, y) innerhalb der Kerze?
    ///
    /// Getroffen wird die volle Höhe inklusive Dochte — `open`/`close` spielen für
    /// den Treffertest daher keine Rolle und sind nicht Teil der Signatur.
    pub fn in_range(
        &self,
        x: f64,
        y: f64,
        candle_x: f64,
        candle_width: f64,
        high_y: f64,
        low_y: f64,
    ) -> bool {
        let half_width = candle_width / 2.0;
        let left = candle_x - half_width;
        let right = candle_x + half_width;
        let top = high_y.min(low_y); // Y is inverted
        let bottom = high_y.max(low_y);

        x >= left && x <= right && y >= top && y <= bottom
    }

    /// Check if X coordinate is within candle X range
    pub fn in_x_range(&self, x: f64, candle_x: f64, candle_width: f64) -> bool {
        let half_width = candle_width / 2.0;
        x >= (candle_x - half_width) && x <= (candle_x + half_width)
    }

    /// Check if Y coordinate is within candle Y range
    pub fn in_y_range(&self, y: f64, high_y: f64, low_y: f64) -> bool {
        let top = high_y.min(low_y); // Y is inverted
        let bottom = high_y.max(low_y);
        y >= top && y <= bottom
    }

    /// Get tooltip position (midpoint between open and close)
    pub fn tooltip_position(&self, candle_x: f64, open_y: f64, close_y: f64) -> (f64, f64) {
        (candle_x, (open_y + close_y) / 2.0)
    }

    /// Get center point (midpoint between high and low)
    pub fn center_point(&self, candle_x: f64, high_y: f64, low_y: f64) -> (f64, f64) {
        (candle_x, (high_y + low_y) / 2.0)
    }

    /// Format OHLC for tooltip display
    pub fn format_ohlc(&self) -> String {
        format!(
            "O: {:.2}  H: {:.2}  L: {:.2}  C: {:.2}",
            self.o, self.h, self.l, self.c
        )
    }
}

/// Point on chart (time, price)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub time: i64,
    pub price: f64,
}

impl Point {
    pub fn new(time: i64, price: f64) -> Self {
        Self { time, price }
    }
}

/// Timeframe enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Timeframe {
    #[serde(rename = "1s")]
    S1,
    #[serde(rename = "5s")]
    S5,
    #[serde(rename = "15s")]
    S15,
    #[serde(rename = "30s")]
    S30,
    #[serde(rename = "1m")]
    M1,
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "15m")]
    M15,
    #[serde(rename = "30m")]
    M30,
    #[serde(rename = "1h")]
    H1,
    #[serde(rename = "2h")]
    H2,
    #[serde(rename = "4h")]
    H4,
    #[serde(rename = "6h")]
    H6,
    #[serde(rename = "12h")]
    H12,
    #[serde(rename = "1d")]
    D1,
    #[serde(rename = "1w")]
    W1,
    #[serde(rename = "1M")]
    MN1,
}

impl Timeframe {
    pub fn as_str(&self) -> &'static str {
        match self {
            Timeframe::S1 => "1s",
            Timeframe::S5 => "5s",
            Timeframe::S15 => "15s",
            Timeframe::S30 => "30s",
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H2 => "2h",
            Timeframe::H4 => "4h",
            Timeframe::H6 => "6h",
            Timeframe::H12 => "12h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
            Timeframe::MN1 => "1M",
        }
    }

    // Bewusst Option statt FromStr: unbekannte Timeframes sind kein Fehlerfall, sondern „nicht unterstützt".
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "1s" => Some(Timeframe::S1),
            "5s" => Some(Timeframe::S5),
            "15s" => Some(Timeframe::S15),
            "30s" => Some(Timeframe::S30),
            "1m" => Some(Timeframe::M1),
            "5m" => Some(Timeframe::M5),
            "15m" => Some(Timeframe::M15),
            "30m" => Some(Timeframe::M30),
            "1h" => Some(Timeframe::H1),
            "2h" => Some(Timeframe::H2),
            "4h" => Some(Timeframe::H4),
            "6h" => Some(Timeframe::H6),
            "12h" => Some(Timeframe::H12),
            "1d" => Some(Timeframe::D1),
            "1w" => Some(Timeframe::W1),
            "1M" => Some(Timeframe::MN1),
            _ => None,
        }
    }

    /// Get timeframe duration in milliseconds
    pub fn duration_secs(&self) -> i64 {
        match self {
            Timeframe::S1 => 1,
            Timeframe::S5 => 5,
            Timeframe::S15 => 15,
            Timeframe::S30 => 30,
            Timeframe::M1 => 60,
            Timeframe::M5 => 300,
            Timeframe::M15 => 900,
            Timeframe::M30 => 1800,
            Timeframe::H1 => 3600,
            Timeframe::H2 => 7200,
            Timeframe::H4 => 14400,
            Timeframe::H6 => 21600,
            Timeframe::H12 => 43200,
            Timeframe::D1 => 86400,
            Timeframe::W1 => 604800,
            Timeframe::MN1 => 2592000, // Approximate 30 days
        }
    }

    /// Dauer in Millisekunden. Abgeleitet aus [`Timeframe::duration_secs`] — die
    /// Zeitachse des Charts rechnet durchgehend in Sekunden.
    pub fn duration_ms(&self) -> i64 {
        self.duration_secs() * 1000
    }
}

/// Session configuration for trading session markers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub name: String,
    /// Open time: (hour, minute) in UTC
    pub open_utc: (u8, u8),
    /// Close time: (hour, minute) in UTC
    pub close_utc: (u8, u8),
    /// RGBA color tuple
    pub color: (u8, u8, u8, u8),
    pub show_open: bool,
    pub show_close: bool,
}

impl SessionConfig {
    pub fn nyse() -> Self {
        Self {
            name: "NYSE".to_string(),
            open_utc: (14, 30),
            close_utc: (21, 0),
            color: (59, 130, 246, 40),
            show_open: true,
            show_close: true,
        }
    }

    pub fn london() -> Self {
        Self {
            name: "London".to_string(),
            open_utc: (8, 0),
            close_utc: (16, 30),
            color: (16, 185, 129, 40),
            show_open: true,
            show_close: true,
        }
    }

    pub fn tokyo() -> Self {
        Self {
            name: "Tokyo".to_string(),
            open_utc: (0, 0),
            close_utc: (6, 0),
            color: (245, 158, 11, 40),
            show_open: true,
            show_close: true,
        }
    }

    pub fn sydney() -> Self {
        Self {
            name: "Sydney".to_string(),
            open_utc: (22, 0),
            close_utc: (7, 0),
            color: (239, 68, 68, 40),
            show_open: true,
            show_close: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_creation() {
        let candle = Candle::new(1000, 100.0, 105.0, 99.0, 103.0, 1000.0);
        assert_eq!(candle.time, 1000);
        assert_eq!(candle.o, 100.0);
        assert_eq!(candle.h, 105.0);
        assert_eq!(candle.l, 99.0);
        assert_eq!(candle.c, 103.0);
        assert_eq!(candle.v, 1000.0);
    }

    #[test]
    fn test_candle_bullish_bearish() {
        let bullish = Candle::new(1000, 100.0, 105.0, 99.0, 103.0, 1000.0);
        assert!(bullish.is_bullish());
        assert!(!bullish.is_bearish());

        let bearish = Candle::new(1000, 103.0, 105.0, 99.0, 100.0, 1000.0);
        assert!(bearish.is_bearish());
        assert!(!bearish.is_bullish());
    }

    #[test]
    fn test_candle_calculations() {
        let candle = Candle::new(1000, 100.0, 105.0, 95.0, 102.0, 1000.0);

        assert_eq!(candle.body_size(), 2.0);
        assert_eq!(candle.range(), 10.0);
        assert_eq!(candle.upper_wick(), 3.0);
        assert_eq!(candle.lower_wick(), 5.0);
        assert_eq!(candle.typical_price(), (105.0 + 95.0 + 102.0) / 3.0);
        assert_eq!(candle.median_price(), (105.0 + 95.0) / 2.0);
    }

    #[test]
    fn test_timeframe_conversion() {
        assert_eq!(Timeframe::M5.as_str(), "5m");
        assert_eq!(Timeframe::from_str("1h"), Some(Timeframe::H1));
        assert_eq!(Timeframe::from_str("invalid"), None);
    }

    #[test]
    fn test_timeframe_duration() {
        assert_eq!(Timeframe::M1.duration_ms(), 60_000);
        assert_eq!(Timeframe::H1.duration_ms(), 3_600_000);
        assert_eq!(Timeframe::D1.duration_ms(), 86_400_000);
    }
}
