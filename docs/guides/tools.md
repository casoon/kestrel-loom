---
title: Drawing tools
description: Trend lines, levels, shapes, Fibonacci retracements and labels — anchored in time and price.
order: 2
---

Tools are anchored in time (Unix seconds) and price, not in pixels, so they stay in place while
the chart is zoomed and panned. Each tool has an id you choose.

## Creating tools

```js
const a = { time: 1600010000, price: 99 };
const b = { time: 1600060000, price: 103 };

chart.tools.trendLine('trend', a, b);
chart.tools.horizontalLine('level', 101.5);
chart.tools.verticalLine('marker', 1600030000);
chart.tools.rectangle('box', a, b);
chart.tools.ellipse('ring', a, b);
chart.tools.fibonacci('fib', a, b);
chart.tools.textLabel('note', a, 'breakout');
```

## Selection and editing

```js
chart.tools.selectAt(x, y);                     // canvas coordinates
chart.tools.selectInRect(x1, y1, x2, y2);
chart.tools.selected();
chart.tools.deleteSelected();
chart.tools.undo();
chart.tools.redo();
```

## Snapping

The magnet pulls anchors to the candle under the pointer: `'off'`, `'weak'` or `'strong'`.

```js
chart.tools.magnet('strong');
chart.tools.snap(time, price);                  // what the magnet would do with this point
```

## Reading them back

`chart.tools.all()` returns the tools on the chart. `chart.state.export()` covers candles,
timeframe and viewport, not the tools — persisting a drawing is up to your application.
