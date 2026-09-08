# kestrel-loom

Interaktiver Chart-Renderer der `kestrel`-Familie: Rust → WebAssembly → Canvas 2D.

`kestrel-chartkit` beschreibt in `viz::scene` ein bewusst renderer-neutrales Szenenmodell
(Panes, Achsen, z-geordnete Objekte, identity-keyed Updates) und rendert daraus heute nur
statisches SVG. Dieses Repo ist der fehlende Renderer: es nimmt Chartkit-Szenen plus eine
OHLC-Serie entgegen und zeichnet sie interaktiv — Zoom, Pan, Crosshair, Multi-Pane,
Zeichenwerkzeuge.

Der Code stammt aus dem Rendering-Kern von
[`loomchart`](https://github.com/casoon/loomchart); die dortige eigene Indikator-Schicht
wird bei der Übernahme **nicht** mitgenommen, sondern durch `kestrel-chartkit` ersetzt.

## Rolle in der Familie

| Repo | Rolle |
|---|---|
| `kestrel-chartkit` | Rechenschicht (Indikatoren, Scoring, Szenenmodell) |
| `kestrel-connector` | Anbieter-Clients |
| `kestrel-marketdata` | Instrument-Registry, Archiv, Store |
| `kestrel` | Dashboard (Tauri) |
| `kestrel-report` | Batch-Berichte |
| **`kestrel-loom`** | **interaktiver Renderer für Chartkit-Szenen** |

## Aufbau

```
crates/renderer/   kestrel-loom        Kern: Zustand, Viewport, Skalen, Panes, Werkzeuge,
                                       RenderCommand-Modell — ohne Browser-API
crates/wasm/       kestrel-loom-wasm   wasm-bindgen-Fassade + Canvas-2D-Ausführung
```

Dass der Kern browserfrei ist, ist erzwungen und nicht bloß Absicht:
`cargo tree -p kestrel-loom` enthält weder `web-sys` noch `js-sys` oder `wasm-bindgen`.

## Bauen und prüfen

```sh
cargo test -p kestrel-loom          # Kern, ohne Browser
./build-wasm.sh                     # WASM-Paket nach ./pkg
```

## Demo

```sh
./build-wasm.sh
python3 -m http.server 8777      # dann http://localhost:8777/demo/
```

Eine kleine Seite mit deterministischen Kerzen: Darstellungswechsel
(Candlestick, OHLC, Hollow, Line, Area, Renko), Zoom per Mausrad, Pan, Crosshair,
Theme-Umschaltung. Sie ist bewusst klein gehalten — sie soll zeigen, dass die
Kette Kern → WASM → Canvas trägt, und als Vorlage für die Einbindung dienen.

## Einbinden

```js
import { createChart } from 'kestrel-loom/js/kestrel-loom.js';

const chart = await createChart(canvas, { timeframe: '5m', dark: true });
chart.data.set(candles);                 // [{ time, o, h, l, c, v }] in Unix-Sekunden
chart.view.fit();
chart.indicators.add('rsi');             // 91 Indikatoren aus kestrel-chartkit
chart.tools.trendLine('t1', { time: 1600010000, price: 99 }, { time: 1600060000, price: 103 });
```

`js/kestrel-loom.js` gruppiert die 86 flachen Methoden der WASM-Fassade nach
`data`, `view`, `style`, `indicators`, `tools`, `compare` und `state` und nimmt
gleich das ab, was sonst jeder Aufrufer selbst schreibt: Canvas-Größe samt
`devicePixelRatio`, `ResizeObserver`, Maus-/Touch-/Tastatureingaben, eine
Zeichenschleife, die nur bei Änderungen rendert, JSON hin und zurück sowie die
`BigInt`-Zeitstempel an der WASM-Grenze. Die flache API bleibt über `chart.raw`
erreichbar.

## Examples

Laufen alle ohne Browser:

```sh
cargo run -p kestrel-loom --example candles_and_viewport   # Kerzen, Viewport, Pixelabbildung
cargo run -p kestrel-loom --example tools_to_commands      # Werkzeuge -> RenderCommand-Strom
cargo run -p kestrel-loom --example state_roundtrip        # Zustand sichern und laden
```

`tools_to_commands` zeigt den Kern der Architektur: Werkzeuge schreiben in einen
`&mut dyn Renderer`, der `BatchRenderer` sammelt daraus einen prüfbaren
`RenderCommand`-Strom — ohne Canvas, ohne `web-sys`.

## Stand

Kern und WASM-Fassade sind übernommen, der Renderloop liegt im Kern und ein
vollständiger Frame ist als Golden-Fixture ohne Browser prüfbar. Die Demo läuft.
Offen ist die Indikator-Anbindung an `kestrel-chartkit`. Konzept, Übernahme-Inventar und
Meilensteine liegen im (gitignorierten) `plan/`-Verzeichnis.

## Lizenz

[BUSL-1.1](./LICENSE) — dieselben Parameter wie `kestrel-chartkit`: nicht-kommerzielle
Nutzung frei, kommerzielle Nutzung erfordert eine Lizenz vom Licensor, Umstellung auf
Apache-2.0 vier Jahre nach Veröffentlichung.
