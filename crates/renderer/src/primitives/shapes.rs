// Basic shapes and line styles

/// Point type (re-export from core for convenience)
pub use crate::core::Point;

/// Candle rendering style
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize, Default)]
pub enum CandleStyle {
    /// Traditional candlestick with filled body
    #[default]
    Candlestick,
    /// OHLC bars with horizontal ticks
    OHLC,
    /// Hollow candlestick (outline only)
    Hollow,
    /// Line connecting close prices
    Line,
    /// Filled area below close-price line
    Area,
    /// Footprint candles with bid/ask volume levels
    Footprint,
    /// Renko bricks — each brick covers `brick_size` price units
    Renko { brick_size: f64 },
}

/// Line style enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LineStyle {
    #[default]
    Solid,
    /// Strichlinie; die Maße sind in CSS-Pixeln gemeint.
    Dashed {
        dash_length: u32,
        gap_length: u32,
    },
    Dotted,
}

impl LineStyle {
    /// Strichlinie mit den üblichen Maßen (5 px Strich, 5 px Lücke).
    pub fn dashed() -> Self {
        LineStyle::Dashed {
            dash_length: 5,
            gap_length: 5,
        }
    }
}

/// Plot configuration for visualization
#[derive(Debug, Clone)]
pub struct PlotConfig {
    pub id: String,
    pub title: String,
    pub color: String,
    pub line_width: u8,
    pub line_style: LineStyle,
}

impl PlotConfig {
    pub fn new(id: &str, title: &str, color: &str) -> Self {
        Self {
            id: id.to_string(),
            title: title.to_string(),
            color: color.to_string(),
            line_width: 2,
            line_style: LineStyle::Solid,
        }
    }

    pub fn line_width(mut self, width: u8) -> Self {
        self.line_width = width;
        self
    }

    pub fn dashed(mut self) -> Self {
        self.line_style = LineStyle::dashed();
        self
    }

    pub fn dotted(mut self) -> Self {
        self.line_style = LineStyle::Dotted;
        self
    }
}
