---
title: The time axis
description: Bars are positioned by index, not by timestamp — and time is still Unix seconds on the outside.
order: 2
---

## Positioned by bar index

A market that closes over the weekend leaves a 49-hour hole in a timestamp axis. kestrel-loom
positions bars by their index instead, so that pause takes exactly one bar of width. The
[demo](../../../demo/) contains such a pause after bar 200.

Axis ticks sit on calendar boundaries that actually have a bar. Hit testing — which bar is
under the pointer — is a direct lookup.

## Time on the outside

Everything that leaves or enters the chart speaks Unix **seconds**, UTC: candles, tool anchors,
the crosshair and the exported state. Inside the core, time is a `Seconds` newtype, the same
domain kestrel-chartkit's scene model declares.

```js
chart.view.timeToX(1600030000);   // screen x of a timestamp, for your own markers
chart.view.xToTime(412);          // timestamp at a screen position
```

At the WASM boundary, timestamps travel as `BigInt`; the wrapper converts them.

## Sessions and time zone

```js
chart.view.timezone(60);          // offset in minutes for axis labels
chart.view.showSessions(true);    // session marker lines
chart.view.rightOffset(10);       // empty bars right of the last one, room for tools
```
