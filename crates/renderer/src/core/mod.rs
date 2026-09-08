// Core types and chart state management

mod adaptive_fps;
mod bar_index;
mod buffer;
mod candle_buffer;
mod candle_lifecycle;
mod chart;
mod chart_renderer;
pub mod indicators;

mod chart_state;
mod config;
mod events;
pub mod footprint;
mod generator;
pub mod heikin_ashi;

mod invalidation;
pub mod overlay;
pub mod overlays;
mod pane;
pub mod renko;
mod scale;
pub mod scene;
mod types;
mod viewport;

pub use adaptive_fps::{AdaptiveFPSConfig, AdaptiveFrameScheduler, FPSStats, RenderComplexity};
pub use bar_index::{BarCoordMapper, BarIndex};
pub use buffer::ChartBuffer;
pub use candle_buffer::CandleBuffer;
pub use candle_lifecycle::{CandleEvent, CandleStore};
pub use chart::Chart;
pub use chart_renderer::{
    render_chart, update_indicator_panes, CompareSymbol, IndicatorPane, RenderExtras,
};
pub use chart_state::{ChartOptions, ChartState, CrosshairState, InteractionState, MagnetMode};
pub use config::ChartConfig;
pub use events::{EventHandler, KeyboardEvent, MouseButton, MouseEvent, TouchEvent};
pub use footprint::{FootprintCandle, FootprintConfig, FootprintLevel, FootprintRenderer};
pub use generator::{
    CandleGenerator, GeneratorConfig, MarketType, Scenario, Trend, VolatilityRegime,
};
pub use indicators::{placement_for, IndicatorLine, IndicatorPlacement, IndicatorSeries};
pub use invalidation::{
    InvalidationLevel, InvalidationLevels, InvalidationMask, LayeredInvalidation, RenderLayer,
};
pub use overlay::{ChartOverlay, OverlayRegistry};
pub use pane::{Pane, PaneLayout};
pub use scale::{PriceScale, ScaleMode};
pub use types::{Candle, Point, SessionConfig, Timeframe};
pub use viewport::{Dimensions, PriceRange, TimeRange, Viewport, ViewportScaleMode};
