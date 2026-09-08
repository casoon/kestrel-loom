//! kestrel-loom — interaktiver Chart-Renderer der `kestrel`-Familie.
//!
//! Dieses Crate kennt keine Browser-API. Es hält Chart-Zustand, Viewport, Skalen,
//! Panes und Werkzeuge und produziert daraus [`RenderCommand`]s; das Ausführen auf
//! Canvas ist Sache von `kestrel-loom-wasm`.
//!
//! Indikator-Berechnung findet hier **nicht** statt — dafür ist `kestrel-chartkit`
//! zuständig (siehe `plan/00-konzept.md`).

pub mod canvas;
pub mod commands;
pub mod core;
pub mod drawings;
pub mod panels;
pub mod primitives;
pub mod rendering;
pub mod state;
pub mod tools;
pub mod utils;

pub use core::{
    Candle, CandleGenerator, Chart, ChartBuffer, ChartConfig, ChartOptions, ChartState,
    CrosshairState, Dimensions, EventHandler, GeneratorConfig, InteractionState, KeyboardEvent,
    MarketType, MouseButton, MouseEvent, Point, PriceRange, TimeRange, Timeframe, TouchEvent,
    Trend, Viewport, VolatilityRegime,
};

pub use commands::{Command, CommandHistory};

pub use drawings::{
    drawing::{LineStyle as DrawingLineStyle, Point as DrawingPoint},
    renderer::{DrawCommand, Viewport as DrawingViewport},
    Drawing, DrawingManager, DrawingRenderer, DrawingStyle, DrawingType,
};

pub use panels::{
    OverlayConfig, OverlayScale, Panel, PanelConfig, PanelId, PanelManager, PanelType, PriceScale,
    ScaleMapper, ScaleRange,
};

pub use primitives::{Color, LineStyle, PlotConfig};

pub use rendering::{DrawStyle, RenderCommand, Renderer, TextAlign, TextBaseline};

pub mod prelude {
    pub use crate::core::*;
    pub use crate::primitives::*;
    pub use crate::rendering::{DrawStyle, RenderCommand, Renderer};
}
