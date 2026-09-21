---
title: Overview
description: What kestrel-loom draws, where it stops, and how this documentation is organised.
order: 0
---

kestrel-loom is an interactive trading-chart renderer for the browser. It draws candlestick
charts, indicator panes and drawing tools onto a Canvas 2D context. Chart state, viewport math
and the render loop are plain Rust, compiled to WebAssembly; only a thin façade knows what a
canvas is. The indicator math comes from
[kestrel-chartkit](https://casoon.github.io/kestrel-chartkit/).

## What it covers

- **Chart types:** candlestick, OHLC, hollow, line, area, Heikin Ashi, Renko and footprint.
- **Indicators:** every indicator of kestrel-chartkit, fed incrementally. Price-unit indicators
  draw on the price chart, oscillators in their own pane.
- **Tools:** trend lines, horizontal and vertical lines, rectangles, ellipses, Fibonacci
  retracements and text labels, with hit testing, selection, snapping and undo/redo.
- **Interaction:** wheel, trackpad and pinch told apart, kinetic glide, a time scrollbar,
  crosshair, log and linear price scale, light and dark theme.
- **Scenes:** kestrel-chartkit's `viz::scene` model (zones, pivots, profiles) rendered onto the
  canvas.

## Where it stops

No indicator math, no strategies, no backtesting, no persistence, no data feed and no backend.
kestrel-loom draws what it is given; where the bars come from is your application's business.

## Status

**Alpha.** The chart runs and is exercised in a browser, but the API is not stable. It is not
published to crates.io or npm: you build it from the repository. The [demo](../demo/) on this
site runs the current build.

## License

Business Source License 1.1 (`BUSL-1.1`), the same terms as kestrel-chartkit: free for
non-commercial use including production, commercial use requires a license from the Licensor,
and the work converts to Apache-2.0 four years after publication. The
[LICENSE](https://github.com/casoon/kestrel-loom/blob/main/LICENSE) file is authoritative.

## How the docs are organised

- **Getting started:** build the package and put a first chart on a page.
- **Guides:** indicators and panes, drawing tools.
- **Concepts:** the browser-free core and the session-continuous time axis.
- **Reference:** the grouped JavaScript API.
