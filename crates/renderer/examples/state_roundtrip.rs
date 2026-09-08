//! Chart-Zustand sichern und wiederherstellen.
//!
//! `ChartState` hält Kerzen, Viewport und Werkzeuge. Export/Import gehen über
//! JSON — dieselbe Form, die die WASM-Fassade als `export_state`/`import_state`
//! nach JavaScript durchreicht.
//!
//! `cargo run -p kestrel-loom --example state_roundtrip`

use kestrel_loom::core::{CandleGenerator, GeneratorConfig, Timeframe};
use kestrel_loom::ChartState;

fn main() {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(7)
            .with_timeframe(Timeframe::M15),
    );

    // Zeiteinheit angleichen — siehe plan/05-befunde-zeiteinheit.md.
    let candles = generator
        .generate(200)
        .into_iter()
        .map(|mut c| {
            c.time /= 1000;
            c
        })
        .collect();

    let mut state = ChartState::new(1200, 600, Timeframe::M15);
    state.set_candles(candles);
    state.fit_to_data();
    state.zoom(0.5, None);

    println!("vorher:");
    println!("  Kerzen:   {}", state.candles.len());
    println!(
        "  Zeitfenster: {} .. {}",
        state.viewport.time_start(),
        state.viewport.time_end()
    );

    let json = state.export().expect("Export");
    println!("  JSON:     {} Bytes", json.len());

    // Frischer Zustand, nur aus dem JSON aufgebaut.
    let mut restored = ChartState::new(1200, 600, Timeframe::M15);
    restored.import(&json).expect("Import");

    println!();
    println!("nachher:");
    println!("  Kerzen:   {}", restored.candles.len());
    println!(
        "  Zeitfenster: {} .. {}",
        restored.viewport.time_start(),
        restored.viewport.time_end()
    );

    assert_eq!(state.candles.len(), restored.candles.len());
    assert_eq!(
        state.viewport.time_start(),
        restored.viewport.time_start(),
        "Viewport muss den Roundtrip überleben"
    );
    println!();
    println!("Roundtrip identisch.");
}
