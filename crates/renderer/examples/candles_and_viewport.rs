//! Kerzen erzeugen, Viewport darauf einpassen, Preis/Zeit in Pixel abbilden.
//!
//! Zeigt den Kern ohne Browser: `cargo run -p kestrel-loom --example candles_and_viewport`

use kestrel_loom::core::{CandleGenerator, GeneratorConfig, PriceRange, TimeRange, Trend};
use kestrel_loom::{Chart, Viewport};

fn main() {
    // Fester Seed — derselbe Lauf ergibt dieselben Kerzen. Voraussetzung für
    // reproduzierbare Tests und Fixtures.
    let config = GeneratorConfig::crypto()
        .with_seed(42)
        .with_trend(Trend::BullishMild);
    let mut generator = CandleGenerator::new(config);

    let mut chart = Chart::new();
    chart.load(generator.generate(120));

    let (low, high) = chart.price_range().expect("Kerzen vorhanden");
    let (start, end) = chart.time_range().expect("Kerzen vorhanden");

    let mut viewport = Viewport::new(800, 400);
    viewport.fit_to_data(
        TimeRange { start, end },
        PriceRange {
            min: low,
            max: high,
        },
    );

    println!("Kerzen:        {}", chart.len());
    println!("Preisspanne:   {low:.2} .. {high:.2}");
    // `visible_bars()` schätzt über die Zeitspanne, nicht über den Datenbestand.
    // Bei durchgehenden Märkten (Krypto) deckt sich das mit der Kerzenzahl; bei
    // Instrumenten mit Handelspausen läuft es auseinander — dafür gibt es
    // `core::BarIndex`.
    println!("Sichtbar:      {} Bars", viewport.visible_bars());
    println!("Balkenbreite:  {:.2} px", viewport.bar_width());
    println!();

    let last = chart.last().expect("Kerzen vorhanden");
    println!(
        "Letzte Kerze   t={} c={:.2}  ->  x={:.1} px, y={:.1} px",
        last.time,
        last.c,
        viewport.time_to_x(last.time),
        viewport.price_to_y(last.c),
    );

    // Die Abbildung ist umkehrbar: Pixel zurück in Preis.
    let y = viewport.price_to_y(last.c);
    println!(
        "Rückrechnung   y={y:.1} px -> {:.2}",
        viewport.y_to_price(y)
    );

    // Zoomen verändert die Abbildung, nicht die Daten.
    println!();
    println!("vor  zoom(0.5): {} Bars sichtbar", viewport.visible_bars());
    viewport.zoom(0.5, None);
    println!("nach zoom(0.5): {} Bars sichtbar", viewport.visible_bars());
    println!("Kerzen unverändert: {}", chart.len());
}
