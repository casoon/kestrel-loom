# Changelog

All notable changes to kestrel-loom are listed here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and kestrel-loom follows
[Semantic Versioning](https://semver.org/): before 1.0, a minor version can contain breaking
changes.

This file was started after 0.1.0; the entries below are drawn from the git history.

## [Unreleased]

### Added

- **Time axis:** the viewport positions by bar index instead of time, so a trading pause takes the width of one bar; a time scrollbar with a draggable handle sits below the chart. Details: [Time axis](https://casoon.github.io/kestrel-loom/docs/concepts/time-axis/).
- **Input:** wheel, trackpad and pinch are told apart; touch pinch, a glide after a pan and a cursor for the scrollbar.
- **Panes:** an optional volume histogram (off by default) and the query `indicatorHasOwnPane`.
- **Pages:** project site with a live demo and the documentation.

### Changed

- **Pages:** the project site uses the current shared theme (0.16.3).
- **Dependencies:** kestrel-chartkit is pinned to v0.16.0 (from v0.2.0); the overlay list follows the 105 indicators of that release.
- **Input:** the mouse wheel scrolls the page; only zoom (⌘/Ctrl or pinch) and horizontal swipes go to the chart. Keys act only while the chart has focus, and live updates keep the visible range.
- **Rendering:** edges sit on device pixels, the pixel ratio is read again on every resize and screen change, and the crosshair snaps to the candle. Price marks use 1-2-5 steps with the grid on the marks; rising candles are hollow; panes get a title strip instead of separator lines; calmer light and dark palettes.
- **Rendering:** scenes are drawn in two passes, areas behind the candles and lines and labels in front of them.
- **Rendering:** the canvas renderer sets fill, stroke and font only when they change, which shortens each frame.
- **Time:** times are a `Seconds` type throughout the core; at the JavaScript boundary they stay a plain number.

### Fixed

- **Panes:** overlays, scenes, comparisons and drawing tools use the price scale of the candles when indicator panes shrink the main area; the crosshair price and the tools match the candles again.
- **Panes:** `addIndicatorPane` returns `''` for an overlay, and overlays no longer count as a pane in the layout.
- **Overlays:** indicators that compute in price units are drawn on the price chart instead of in a pane of their own; a test checks the list against what every catalog indicator computes.
- **Price axis:** labels at the edge are dropped instead of being drawn half cut off.

## [0.1.0] - 2026-09-09

### Added

- **Renderer:** a Rust core with chart state, viewport, panes, drawing tools and the render loop, which writes drawing commands; a WebAssembly crate executes them on a Canvas 2D context.
- **Indicators:** kestrel-chartkit as the calculation layer, fed bar by bar: overlays on the price chart, multi-line outputs, indicator artifacts, and scenes across the JavaScript boundary.
- **Chart types:** Heikin Ashi as a derived series next to the other candle styles.
- **Drawing tools:** adding a tool with an existing ID replaces it.
- **JavaScript:** a grouped wrapper, `kestrel-loom.js`, that takes a canvas and handles sizing, input and redrawing.
- **Examples:** `candles_and_viewport`, `tools_to_commands` and `state_roundtrip`, all runnable without a browser.
- **License:** BUSL-1.1, the same terms as kestrel-chartkit.
