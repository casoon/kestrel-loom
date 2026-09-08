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

Kern und WASM-Fassade sind übernommen und lauffähig. Der eigentliche Zeichenablauf
liegt noch in der WASM-Fassade und wird als Nächstes in den Kern gezogen; die
Indikator-Anbindung an `kestrel-chartkit` steht aus. Konzept, Übernahme-Inventar und
Meilensteine liegen im (gitignorierten) `plan/`-Verzeichnis.

## Lizenz

[BUSL-1.1](./LICENSE) — dieselben Parameter wie `kestrel-chartkit`: nicht-kommerzielle
Nutzung frei, kommerzielle Nutzung erfordert eine Lizenz vom Licensor, Umstellung auf
Apache-2.0 vier Jahre nach Veröffentlichung.
