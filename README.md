# kestrel-loom

Interactive trading-chart renderer: Rust → WebAssembly → Canvas 2D.

`kestrel-loom` draws candlestick charts, indicator panes and drawing tools in the
browser. The chart state, the viewport math and the whole render loop live in plain
Rust and produce a stream of drawing commands; only a thin façade knows what a canvas
is. Indicator math is not in this repository — it comes from
[`kestrel-chartkit`](https://github.com/casoon/kestrel-chartkit), which contributes 91
streaming indicators.

> **Alpha.** The chart runs and is exercised in a browser, but the API is not stable
> and pieces are missing — see [Status](#status).

## Why the split

A renderer that owns its own indicator library ends up maintaining two of everything.
Here the boundary is deliberate:

| | |
|---|---|
| `kestrel-chartkit` | indicator math, scoring, artifacts, the renderer-neutral scene model |
| **`kestrel-loom`** | **viewport, scales, panes, tools, the render loop, canvas output** |

Chartkit knows nothing about browsers; this crate knows nothing about how an RSI is
computed. Neither is a plugin of the other — they meet at a data contract.

## Layout

```
crates/renderer/   kestrel-loom        core: state, viewport, scales, panes, tools,
                                       render loop, RenderCommand model — no browser API
crates/wasm/       kestrel-loom-wasm   wasm-bindgen façade + Canvas 2D execution
js/                kestrel-loom.js     grouped JavaScript wrapper — what you embed
demo/                                  a small page to look at and copy from
```

**The core stays browser-free, and that is enforced rather than intended:** CI fails if
`cargo tree -p kestrel-loom` ever grows a `web-sys`, `js-sys` or `wasm-bindgen`
dependency. That is what makes a full frame testable without a browser — a render pass
writes `RenderCommand`s into a recorder, and golden fixtures compare the resulting
stream.

## Quick start

```sh
./build-wasm.sh                 # wasm-pack build into ./pkg
python3 -m http.server 8777     # then open http://localhost:8777/demo/
```

```js
import { createChart } from './js/kestrel-loom.js';

const chart = await createChart(canvas, { timeframe: '5m', dark: true });

chart.data.set(candles);          // [{ time, o, h, l, c, v }], time in Unix seconds
chart.view.fit();
chart.indicators.add('rsi');      // any name from chart.indicators.available()
chart.tools.trendLine('t1', { time: 1600010000, price: 99 },
                            { time: 1600060000, price: 103 });
```

The wrapper groups the façade's 86 flat methods into `data`, `view`, `style`,
`indicators`, `tools`, `compare` and `state`, and takes care of the parts every caller
would otherwise write again: canvas sizing with `devicePixelRatio`, a `ResizeObserver`,
mouse/touch/keyboard wiring, a render loop that only draws when something changed, JSON
marshalling, and the `BigInt` timestamps at the WASM boundary. The flat API stays
reachable as `chart.raw`.

## What it does

- **Chart types** — candlestick, OHLC, hollow, line, area, Heikin Ashi, Renko, footprint
- **Indicators** — all 91 from `kestrel-chartkit`, fed incrementally (`on_bar` per new
  bar, no window recomputation). Price-unit indicators such as Bollinger, Keltner or
  Supertrend draw as overlays on the price chart; oscillators get their own pane. Multi-
  line outputs (MACD signal and histogram, band upper/lower) are drawn, not dropped.
- **Tools** — trend lines, horizontal and vertical lines, rectangles, ellipses,
  Fibonacci retracements, text labels; hit testing, selection, snapping, undo/redo
- **Interaction** — zoom, pan, crosshair, log/linear price scale, themes
- **Scenes** — `kestrel-chartkit`'s `viz::scene` model rendered onto canvas: zones,
  pivots, profiles, with z-order and opacity

Time is Unix **seconds**, UTC, everywhere — the same domain the scene model declares.

## What it does not do

No indicator math, no strategies, no backtesting, no persistence, no data feed, no
backend. Those belong to other crates. It also does not decide what your chart means:
it draws what it is given.

## Status

Working: core, WASM façade, JS wrapper, demo, 204 tests, CI gate (fmt, clippy
`-D warnings`, tests, wasm32 build, `wasm-pack` smoke build, strict rustdoc).

Missing:

- Scenes cannot yet be handed across the JS boundary — the renderer draws them, the
  façade always passes `None`.
- Indicator artifacts (order blocks, fair value gaps, volume profiles) are computed but
  not yet collected into a scene.
- The WASM façade has no tests of its own.

## Provenance

The rendering core comes from [`loomchart`](https://github.com/casoon/loomchart) (now
archived), stripped of its own indicator layer and of a dead second drawing system. The
move surfaced a few things that had never shown up there: the documented "main render
loop" was unreachable code, indicators recomputed whole windows despite documentation
claiming otherwise, an advertised chart type did not exist, and the candle generator
emitted milliseconds where everything else expected seconds — which silently disabled
zooming in every test and example path. All fixed here.

## License

[BUSL-1.1](./LICENSE), the same terms as `kestrel-chartkit`: free for non-commercial
use including production, commercial use requires a license from the licensor, and the
work converts to Apache-2.0 four years after publication.
