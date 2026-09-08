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

## Stand

Konzeptphase — noch kein Code. Konzept, Übernahme-Inventar und Meilensteine liegen im
(gitignorierten) `plan/`-Verzeichnis.
