---
title: Quickstart
description: Put a chart on a page — a canvas, the wrapper, candles in Unix seconds.
order: 2
---

## A canvas with a size

The wrapper sizes the canvas from its layout box and follows it with a `ResizeObserver`, so give
it a size in CSS:

```html
<canvas id="chart" style="width: 100%; height: 480px"></canvas>
```

## Create the chart

```js
import { createChart } from './js/kestrel-loom.js';

const canvas = document.getElementById('chart');
const chart = await createChart(canvas, { timeframe: '5m', dark: true });
```

`createChart` loads the WebAssembly module, wires mouse, touch and keyboard input and starts a
render loop that only draws when something changed. Each of these can be turned off with
`autoInput`, `autoResize` and `autoRender`.

## Hand it candles

```js
chart.data.set([
  { time: 1600000000, o: 100, h: 100.6, l: 99.7, c: 100.3, v: 420 },
  { time: 1600000300, o: 100.3, h: 100.9, l: 100.1, c: 100.8, v: 610 },
  // …
]);
chart.view.fit();
```

`time` is Unix **seconds**, UTC. The core keeps it in a `Seconds` newtype, so a millisecond
value does not slip through silently.

New bars go through `chart.data.add(candle)`; a bar that is still forming is updated with
`chart.data.updateLast(candle)`.

## Add an indicator and a tool

```js
chart.indicators.add('rsi');
chart.tools.trendLine('t1', { time: 1600010000, price: 99 },
                            { time: 1600060000, price: 103 });
```

`chart.indicators.available()` lists every indicator name the build knows.

## Clean up

```js
chart.destroy();   // detaches events, stops the loop, frees the WASM object
```
