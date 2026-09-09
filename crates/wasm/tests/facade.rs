//! Tests der WASM-Fassade.
//!
//! Laufen über `wasm-pack test --node`: alles hier kommt ohne Canvas und ohne DOM
//! aus, also ohne Browser. Was einen angehängten Canvas braucht — Zeichnen,
//! Pixelverhältnis —, bleibt der Demo und der Einbindung vorbehalten.
//!
//! Bis 2026-09-09 hatte die Fassade gar keine Tests; der `resize`-Fehler
//! (`plan/04-befunde.md` B4) fiel deshalb erst im Browser auf.

use kestrel_loom_wasm::WasmChart;
use wasm_bindgen_test::*;

fn candles(n: usize) -> String {
    let mut out = String::from("[");
    let mut price = 100.0f64;
    for i in 0..n {
        let time = 1_600_000_000i64 + i as i64 * 300;
        let close = price + ((i % 7) as f64 - 3.0) * 0.2;
        out.push_str(&format!(
            "{}{{\"time\":{},\"o\":{},\"h\":{},\"l\":{},\"c\":{},\"v\":100}}",
            if i == 0 { "" } else { "," },
            time,
            price,
            price.max(close) + 0.3,
            price.min(close) - 0.3,
            close
        ));
        price = close;
    }
    out.push(']');
    out
}

fn chart_with_data(n: usize) -> WasmChart {
    let mut chart = WasmChart::new(800, 400, "5m").expect("Chart baubar");
    chart.set_candles(&candles(n)).expect("Kerzen annehmbar");
    chart
}

#[wasm_bindgen_test]
fn a_fresh_chart_needs_a_first_frame() {
    let chart = WasmChart::new(800, 400, "5m").unwrap();
    assert!(chart.is_dirty(), "der erste Frame muss gezeichnet werden");
}

#[wasm_bindgen_test]
fn candles_arrive_and_come_back() {
    let chart = chart_with_data(50);
    let json = chart.get_candles();
    assert!(json.contains("1600000000"), "Zeitstempel bleiben erhalten");
}

#[wasm_bindgen_test]
fn malformed_candles_are_rejected() {
    let mut chart = WasmChart::new(800, 400, "5m").unwrap();
    assert!(chart.set_candles("kein json").is_err());
}

/// Regression zu B4: Nach einer Größenänderung muss neu gezeichnet werden.
///
/// Der Browser leert den Canvas beim Setzen von `width`/`height`; bliebe der
/// Zustand sauber, käme `render()` sofort zurück und das Bild bliebe schwarz.
#[wasm_bindgen_test]
fn resizing_requires_a_redraw() {
    let mut chart = chart_with_data(50);
    chart.render().ok(); // ohne Canvas ein No-op, setzt den Zustand aber sauber
    chart.fit_to_data();
    let _ = chart.export_state();

    chart.resize(1000, 500).unwrap();
    assert!(
        chart.is_dirty(),
        "eine Größenänderung verlangt einen neuen Frame"
    );

    let viewport = format!("{:?}", chart.get_viewport_info());
    assert!(
        viewport.contains("1000"),
        "neue Breite im Viewport: {viewport}"
    );
}

#[wasm_bindgen_test]
fn known_candle_styles_are_accepted_and_unknown_ones_refused() {
    let mut chart = chart_with_data(20);
    for style in [
        "candlestick",
        "ohlc",
        "hollow",
        "line",
        "area",
        "heikinashi",
        "renko:0.5",
        "footprint",
    ] {
        assert!(
            chart.set_candle_style(style).is_ok(),
            "{style} sollte bekannt sein"
        );
    }
    assert!(chart.set_candle_style("gibtsnicht").is_err());
}

#[wasm_bindgen_test]
fn the_indicator_catalogue_is_reachable_from_javascript() {
    let json = WasmChart::available_indicators();
    assert!(json.starts_with('['));
    assert!(json.contains("\"rsi\""), "Katalog enthält rsi: {json}");
}

#[wasm_bindgen_test]
fn an_unknown_indicator_is_refused_instead_of_ignored() {
    let mut chart = chart_with_data(60);
    assert!(chart.add_indicator_pane("rsi", "{}").is_ok());
    assert!(
        chart.add_indicator_pane("gibtsnicht", "{}").is_err(),
        "ein stillschweigend weggelassener Indikator wäre schlimmer als ein Fehler"
    );
}

#[wasm_bindgen_test]
fn indicator_parameters_must_be_numbers() {
    let mut chart = chart_with_data(60);
    assert!(chart.add_indicator_pane("rsi", "{\"rsi_len\": 21}").is_ok());
    assert!(chart
        .add_indicator_pane("rsi", "{\"rsi_len\": \"lang\"}")
        .is_err());
    assert!(
        chart.add_indicator_pane("rsi", "").is_ok(),
        "leer = Standard"
    );
}

#[wasm_bindgen_test]
fn a_scene_can_be_set_and_cleared() {
    let mut chart = chart_with_data(20);
    let scene = r#"{"panes":[{"id":"p","height_ratio":1.0,"axes":[],"objects":[]}]}"#;
    assert!(chart.set_scene(scene).is_ok());
    assert!(chart.set_scene("").is_ok(), "leer entfernt sie wieder");
    assert!(chart.set_scene("{kaputt").is_err());
}

#[wasm_bindgen_test]
fn state_survives_an_export_import_roundtrip() {
    let mut chart = chart_with_data(100);
    chart.fit_to_data();
    let before = format!("{:?}", chart.get_viewport_info());
    let json = chart.export_state().unwrap();

    let mut restored = WasmChart::new(800, 400, "5m").unwrap();
    restored.import_state(&json).unwrap();

    assert_eq!(format!("{:?}", restored.get_viewport_info()), before);
}

#[wasm_bindgen_test]
fn tools_with_the_same_id_replace_each_other() {
    let mut chart = chart_with_data(20);
    chart.create_horizontal_line("dup", 100.0).unwrap();
    chart.create_horizontal_line("dup", 102.0).unwrap();

    let tools = chart.get_tools();
    assert_eq!(
        tools.matches("\"dup\"").count(),
        1,
        "eine ID, ein Werkzeug: {tools}"
    );
}
