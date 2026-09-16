//! Tests der WASM-Fassade.
//!
//! Laufen über `wasm-pack test --node`: alles hier kommt ohne Canvas und ohne DOM
//! aus, also ohne Browser. Was einen angehängten Canvas braucht — Zeichnen,
//! Pixelverhältnis —, bleibt der Demo und der Einbindung vorbehalten.
//!
//! Bis 2026-09-09 hatte die Fassade gar keine Tests; der `resize`-Fehler
//! (`plan/spezifikation/04-befunde.md` B4) fiel deshalb erst im Browser auf.

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

/// Zeitfenster aus `getViewportInfo` — die Fassade liefert JSON als String.
fn time_span(chart: &WasmChart) -> i64 {
    let json = chart
        .get_viewport_info()
        .as_string()
        .expect("getViewportInfo liefert einen String");
    let value: serde_json::Value = serde_json::from_str(&json).expect("gültiges JSON");
    value["time"]["end"].as_i64().unwrap() - value["time"]["start"].as_i64().unwrap()
}

/// Der Befund vom 2026-09-16: jedes Rad-Ereignis zoomte um feste 10 %, also
/// zoomte ein Wischer der Magic Mouse dutzendfach. Ein Wischer darf den Zoom
/// gar nicht anfassen.
#[wasm_bindgen_test]
fn a_trackpad_swipe_does_not_zoom() {
    let mut chart = chart_with_data(200);
    let before = time_span(&chart);

    for step in 0..25 {
        chart.on_wheel(400.0, 200.0, 6.0, 0.0, false, 0, step as f64 * 16.0);
    }

    assert_eq!(
        before,
        time_span(&chart),
        "ein horizontaler Wischer verschiebt, er zoomt nicht"
    );
}

#[wasm_bindgen_test]
fn a_classic_wheel_notch_zooms() {
    let mut chart = chart_with_data(200);
    let before = time_span(&chart);

    chart.on_wheel(400.0, 200.0, 0.0, -100.0, false, 0, 0.0);

    assert!(
        time_span(&chart) < before,
        "hochscrollen mit dem Rad zoomt hinein"
    );
}

#[wasm_bindgen_test]
fn a_pinch_zooms_despite_a_small_delta() {
    let mut chart = chart_with_data(200);
    let before = time_span(&chart);

    chart.on_wheel(400.0, 200.0, 0.0, -4.0, true, 0, 0.0);

    assert!(
        time_span(&chart) < before,
        "ctrlKey ist das Pinch-Signal des Browsers"
    );
}

/// Kerzen mit einer 49-Stunden-Handelspause in der Mitte.
fn candles_with_a_weekend(before: usize, after: usize) -> String {
    let hour = 3600i64;
    let mut times: Vec<i64> = (0..before as i64)
        .map(|i| 1_789_084_800 + i * hour)
        .collect();
    let close = *times.last().unwrap();
    times.extend((1..=after as i64).map(|i| close + 49 * hour + i * hour));

    let mut out = String::from("[");
    for (i, time) in times.iter().enumerate() {
        out.push_str(&format!(
            "{}{{\"time\":{},\"o\":100,\"h\":101,\"l\":99,\"c\":100.5,\"v\":100}}",
            if i == 0 { "" } else { "," },
            time
        ));
    }
    out.push(']');
    out
}

/// Der Befund aus `plan/spezifikation/02-bar-index-achse.md`: auf der Zeitachse bekam eine
/// 49-Stunden-Pause 49 Bar-Breiten Platz, im Chart klaffte eine Lücke.
#[wasm_bindgen_test]
fn a_trading_break_does_not_open_a_gap() {
    let mut chart = WasmChart::new(800, 400, "1h").expect("Chart baubar");
    chart
        .set_candles(&candles_with_a_weekend(10, 10))
        .expect("Kerzen annehmbar");

    let hour = 3600i64;
    let friday_close = 1_789_084_800 + 9 * hour;

    let across_the_break =
        chart.time_to_x(friday_close + 50 * hour) - chart.time_to_x(friday_close);
    let between_two_bars = chart.time_to_x(hour) - chart.time_to_x(0);

    assert!(
        (across_the_break - between_two_bars).abs() < 0.001,
        "über die Pause {across_the_break} px, zwischen zwei Bars {between_two_bars} px"
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

#[wasm_bindgen_test]
fn two_finger_pinch_zooms_in() {
    let mut chart = chart_with_data(200);
    let before = time_span(&chart);

    chart.on_touch_pinch(400.0, 200.0, 1.5, 0.0);

    assert!(
        time_span(&chart) < before,
        "auseinanderziehen muss hineinzoomen"
    );
}

/// Ein Trackpad-Wischer endet abrupt; `tick` holt den Nachlauf nach.
#[wasm_bindgen_test]
fn a_released_swipe_glides_after_the_gesture() {
    let mut chart = chart_with_data(200);

    for step in 0..3 {
        chart.on_wheel(400.0, 200.0, 8.0, 0.0, false, 0, step as f64 * 16.0);
    }
    let at_rest = chart.x_to_time(400.0);

    // Geste setzt aus, der Nachlauf startet und schreitet fort.
    let _ = chart.tick(300.0);
    assert!(
        chart.tick(316.0),
        "der Nachlauf muss den Ausschnitt weiterziehen"
    );
    assert_ne!(
        chart.x_to_time(400.0),
        at_rest,
        "der sichtbare Ausschnitt hat sich bewegt"
    );
}

#[wasm_bindgen_test]
fn the_scrollbar_reports_a_cursor() {
    let chart = chart_with_data(200);

    // Die Leiste liegt über der Zeitachse am unteren Rand (Höhe 400,
    // Zeitachse 20 px, Leiste 10 px hoch → y ≈ 370–380).
    let cursor = chart.scrollbar_cursor_at(400.0, 375.0);
    assert!(
        !cursor.is_empty(),
        "über der Zeitleiste muss eine Cursor-Form gemeldet werden"
    );
    assert_eq!(
        chart.scrollbar_cursor_at(400.0, 10.0),
        "",
        "außerhalb der Leiste keine Form"
    );
}
