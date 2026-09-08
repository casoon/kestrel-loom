//! Rendering system
//!
//! Defines the Renderer trait and implementations for different backends

pub mod optimizations;
mod renderer;

pub use renderer::{
    BatchRenderer, CandleData, DrawStyle, LineStyle, RenderCommand, Renderer, TextAlign,
    TextBaseline,
};

pub use optimizations::{
    calculate_indicator_complexity, calculate_visible_count, cull_candles, should_render_detail,
    should_render_indicator, RenderDetail,
};
