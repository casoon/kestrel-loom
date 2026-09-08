//! kestrel-loom-wasm — `wasm-bindgen`-Fassade über `kestrel-loom`.
//!
//! Führt die `RenderCommand`s des Kerns auf einem HTML-Canvas aus und stellt die
//! JavaScript-API bereit. Alles, was hier steht, kennt den Browser; alles, was im
//! Kern steht, nicht.

mod canvas2d;
mod chart;

pub use canvas2d::Canvas2DRenderer;
pub use chart::WasmChart;
