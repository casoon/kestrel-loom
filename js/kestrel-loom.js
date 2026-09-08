/**
 * Ergonomische Hülle um die WASM-Fassade.
 *
 * `WasmChart` exportiert 86 flache Methoden — das ist die Form, die
 * `wasm-bindgen` gut kann: Gruppierung auf Rust-Seite bräuchte je Bereich einen
 * eigenen exportierten Typ mit `Rc<RefCell<…>>` auf denselben Zustand, also
 * Laufzeit-Borrow-Prüfung für einen rein kosmetischen Gewinn.
 *
 * Deshalb liegt die Gruppierung hier, in einer dünnen JS-Schicht. Sie nimmt
 * zugleich den Kram ab, den sonst jeder Aufrufer selbst schreibt:
 *
 * - Canvas-Größe samt `devicePixelRatio`, inklusive `ResizeObserver`
 * - Maus-, Touch- und Tastaturereignisse
 * - eine Zeichenschleife, die nur rendert, wenn sich etwas geändert hat
 * - JSON hin und zurück (die Fassade tauscht Strings aus)
 * - `BigInt` für Zeitstempel (i64 kommt als BigInt in JavaScript an)
 *
 * Die flache API bleibt über `chart.raw` erreichbar — diese Hülle versteckt
 * nichts, sie ordnet nur.
 */

import init, { WasmChart } from '../pkg/kestrel_loom_wasm.js';

/** Sekunden-Zeitstempel in das BigInt, das die WASM-Grenze erwartet. */
const t = (time) => BigInt(Math.trunc(Number(time)));

const parse = (json, fallback) => {
  try {
    return JSON.parse(json);
  } catch {
    return fallback;
  }
};

/**
 * Baut einen Chart auf dem gegebenen Canvas.
 *
 * @param {HTMLCanvasElement} canvas
 * @param {object} [options]
 * @param {string} [options.timeframe='5m']
 * @param {boolean} [options.dark=true]
 * @param {boolean} [options.autoResize=true]  ResizeObserver anhängen
 * @param {boolean} [options.autoInput=true]   Maus/Touch/Tastatur verdrahten
 * @param {boolean} [options.autoRender=true]  Zeichenschleife starten
 */
export async function createChart(canvas, options = {}) {
  const {
    timeframe = '5m',
    dark = true,
    autoResize = true,
    autoInput = true,
    autoRender = true,
  } = options;

  await init();

  const rect = canvas.getBoundingClientRect();
  const chart = new WasmChart(rect.width || canvas.width, rect.height || canvas.height, timeframe);

  const applySize = () => {
    const r = canvas.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(r.width * dpr);
    canvas.height = Math.round(r.height * dpr);
    // resize() markiert den Zustand als verändert — der Browser hat den Canvas
    // beim Setzen von width/height geleert.
    chart.resize(r.width, r.height);
  };

  applySize();
  chart.attachCanvas(canvas);
  chart.setTheme(dark);

  const teardown = [];
  const on = (target, type, handler, opts) => {
    target.addEventListener(type, handler, opts);
    teardown.push(() => target.removeEventListener(type, handler, opts));
  };
  const local = (e) => {
    const r = canvas.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  if (autoInput) {
    on(canvas, 'mousedown', (e) => chart.onMouseDown(...local(e), e.button));
    on(canvas, 'mousemove', (e) => chart.onMouseMove(...local(e)));
    on(window, 'mouseup', (e) => chart.onMouseUp(...local(e), e.button));
    on(canvas, 'mouseleave', () => chart.onMouseLeave());
    on(canvas, 'dblclick', (e) => chart.onDoubleClick(...local(e)));
    on(
      canvas,
      'wheel',
      (e) => {
        e.preventDefault();
        chart.onMouseWheel(...local(e), e.deltaY);
      },
      { passive: false },
    );
    on(canvas, 'touchstart', (e) => {
      const touch = e.touches[0];
      if (touch) chart.onTouchStart(...local(touch));
    });
    on(canvas, 'touchmove', (e) => {
      const touch = e.touches[0];
      if (touch) chart.onTouchMove(...local(touch));
    });
    on(canvas, 'touchend', (e) => {
      const touch = e.changedTouches[0];
      if (touch) chart.onTouchEnd(...local(touch));
    });
    on(window, 'keydown', (e) => chart.onKeyDown(e.key));
  }

  let observer = null;
  if (autoResize && typeof ResizeObserver !== 'undefined') {
    observer = new ResizeObserver(applySize);
    observer.observe(canvas);
    teardown.push(() => observer.disconnect());
  }

  let frame = null;
  let running = false;
  const draw = () => chart.render();
  const loop = () => {
    if (!running) return;
    if (chart.isDirty()) draw();
    frame = requestAnimationFrame(loop);
  };
  if (autoRender) {
    running = true;
    frame = requestAnimationFrame(loop);
  } else {
    draw();
  }

  return {
    /** Die flache WASM-Fassade, falls etwas fehlt. */
    raw: chart,

    /** Sofort zeichnen, unabhängig von der Schleife. */
    render: draw,

    data: {
      set: (candles) => chart.setCandles(JSON.stringify(candles)),
      append: (candles) => chart.appendCandles(JSON.stringify(candles)),
      add: (c) => chart.addCandle(t(c.time), c.o, c.h, c.l, c.c, c.v),
      updateLast: (c) => chart.updateRunningCandle(JSON.stringify(c)),
      all: () => parse(chart.getCandles(), []),
      at: (x, y) => chart.getCandleAtPosition(x, y),
      ohlcText: () => chart.getOHLCFormatted(),
    },

    view: {
      fit: () => chart.fitToData(),
      resize: applySize,
      theme: (isDark) => chart.setTheme(isDark),
      info: () => parse(chart.getViewportInfo(), null),
      crosshair: () => chart.getCrosshairInfo(),
      logScale: (on) => (on === undefined ? chart.isLogScale() : chart.setLogScale(on)),
      priceLocked: (on) => (on === undefined ? chart.isPriceLocked() : chart.setPriceLocked(on)),
      scaleMode: (mode) => (mode === undefined ? chart.getScaleMode() : chart.setScaleMode(mode)),
      barSpacing: (px) => (px === undefined ? chart.getBarSpacing() : chart.setBarSpacing(px)),
      barWidthRatio: (ratio) => chart.setBarWidthRatio(ratio),
      timezone: (offsetMinutes) =>
        offsetMinutes === undefined
          ? chart.getTimezoneOffset()
          : chart.setTimezone(offsetMinutes),
      sessions: (sessions) => chart.setSessions(JSON.stringify(sessions)),
      showSessions: (on) => chart.setShowSessions(on),
      resetPriceScale: () => chart.resetPriceScale(),
      resetTimeScale: () => chart.resetTimeScale(),
    },

    style: {
      set: (style) => chart.setCandleStyle(style),
      renkoBrick: (size) => chart.setRenkoBrickSize(size),
    },

    indicators: {
      /** Namen aller Indikatoren aus `kestrel-chartkit`. */
      available: () => parse(WasmChart.availableIndicators(), []),
      add: (name, params = {}) => chart.addIndicatorPane(name, JSON.stringify(params)),
      remove: (paneId) => chart.removePane(paneId),
      height: (paneId, fraction) => chart.setPaneHeightFraction(paneId, fraction),
      layout: () => parse(chart.getPaneLayout(), []),
    },

    tools: {
      trendLine: (id, from, to) => chart.createTrendLine(id, t(from.time), from.price, t(to.time), to.price),
      horizontalLine: (id, price) => chart.createHorizontalLine(id, price),
      verticalLine: (id, time) => chart.createVerticalLine(id, t(time)),
      rectangle: (id, from, to) => chart.createRectangle(id, t(from.time), from.price, t(to.time), to.price),
      ellipse: (id, from, to) => chart.createEllipse(id, t(from.time), from.price, t(to.time), to.price),
      fibonacci: (id, from, to) => chart.createFibonacci(id, t(from.time), from.price, t(to.time), to.price),
      textLabel: (id, at, text) => chart.createTextLabel(id, t(at.time), at.price, text),
      remove: (id) => chart.removeTool(id),
      clear: () => chart.clearTools(),
      all: () => parse(chart.getTools(), []),
      selectAt: (x, y, additive = false) => chart.selectDrawingAt(x, y, additive),
      selectInRect: (x1, y1, x2, y2, additive = false) =>
        parse(chart.selectDrawingsInRect(x1, y1, x2, y2, additive), []),
      selected: () => parse(chart.getSelectedDrawings(), []),
      deleteSelected: () => chart.deleteSelectedDrawings(),
      magnet: (mode) => (mode === undefined ? chart.getMagnetMode() : chart.setMagnetMode(mode)),
      snap: (time, price) => chart.snapToCandle(t(time), price),
      undo: () => chart.undo(),
      redo: () => chart.redo(),
    },

    compare: {
      add: (symbol, candles, color) =>
        chart.addCompareSymbol(symbol, JSON.stringify(candles), color),
      remove: (symbol) => chart.removeCompareSymbol(symbol),
      all: () => parse(chart.getCompareSymbols(), []),
    },

    state: {
      export: () => chart.exportState(),
      import: (json) => chart.importState(json),
    },

    /** Ereignisse abhängen, Schleife stoppen, WASM-Objekt freigeben. */
    destroy() {
      running = false;
      if (frame !== null) cancelAnimationFrame(frame);
      for (const off of teardown) off();
      chart.free();
    },
  };
}
