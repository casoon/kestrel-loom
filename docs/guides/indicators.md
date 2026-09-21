---
title: Indicators and panes
description: How indicators from kestrel-chartkit get onto the chart, where they are drawn and what artifacts are.
order: 1
---

## Adding and removing

```js
const names = chart.indicators.available();        // every name in kestrel-chartkit's catalogue
const pane = chart.indicators.add('bollinger');     // returns the pane id
chart.indicators.add('rsi', { rsi_len: 21 });       // parameters as an object
chart.indicators.remove(pane);
```

Adding an indicator that is already on the chart with different parameters replaces it with a
new instance: an incremental state never continues with half of the old parameters.

## Where an indicator is drawn

Indicators whose values are in price units — Bollinger, Keltner, Supertrend and the like — are
drawn as overlays on the price chart. Oscillators get a pane of their own below it. The
classification is not a hand-kept list: a test probes what each indicator actually computes, so
it stays correct as the catalogue grows.

Multi-line outputs are drawn completely, for example the signal line and histogram of MACD or
the upper and lower band of a channel.

```js
chart.indicators.layout();                // panes with their height fractions
chart.indicators.height(pane, 0.25);      // a quarter of the chart height
```

## Incremental updates

Each indicator is fed with `on_bar` per new bar. Appending a candle does not recompute a
window over the history.

## Artifacts

Some indicators report more than a line: zones, pivots, volume profiles. They are computed
anyway; drawing them is a switch.

```js
chart.indicators.artifacts(true);
```

## Scenes

A `viz::scene::Scene` from kestrel-chartkit can be laid over the chart, as an object or as its
JSON string. Its z-order and opacity are respected.

```js
chart.scene.set(scene);
chart.scene.clear();
```
