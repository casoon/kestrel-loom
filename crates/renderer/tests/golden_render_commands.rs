//! Golden-Fixtures über den `RenderCommand`-Strom.
//!
//! Das Renderer-Gegenstück zu `kestrel-chartkit`s Golden-Reference-Regel: eine
//! deterministische Eingabe erzeugt einen festgeschriebenen Befehlsstrom. Ändert
//! sich das Zeichenverhalten, schlägt der Test fehl — auch dann, wenn das Ergebnis
//! im Browser weiterhin "irgendwie aussieht".
//!
//! Fixture neu schreiben (nur nach bewusster Verhaltensänderung, Diff prüfen):
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p kestrel-loom --test golden_render_commands
//! ```

use std::collections::HashMap;

use kestrel_loom::core::{
    render_chart, update_indicator_panes, CandleGenerator, ChartState, GeneratorConfig,
    IndicatorPane, IndicatorPlacement, PriceRange, RenderExtras, TimeRange, Timeframe, Viewport,
};
use kestrel_loom::rendering::{cull_candles, DrawStyle};
use kestrel_loom::tools::{ChartTool, HorizontalLine, ToolManager, ToolNode, TrendLine};
use kestrel_loom::{BatchRenderer, RenderCommand};

/// Stabile Textform eines Befehls. Bewusst nicht `Debug`: feste Nachkommastellen,
/// damit belanglose Formatierungsunterschiede keine Fixture brechen.
fn format_command(cmd: &RenderCommand) -> String {
    fn style(s: &DrawStyle) -> String {
        format!(
            "fill={} stroke={} w={:.2}",
            s.fill_color.is_some(),
            s.stroke_color.is_some(),
            s.line_width
        )
    }

    match cmd {
        RenderCommand::Clear { .. } => "clear".to_string(),
        RenderCommand::Line {
            x1,
            y1,
            x2,
            y2,
            width,
            ..
        } => format!("line ({x1:.2},{y1:.2})-({x2:.2},{y2:.2}) w={width:.2}"),
        RenderCommand::Rect {
            x,
            y,
            width,
            height,
            style: s,
        } => format!("rect ({x:.2},{y:.2}) {width:.2}x{height:.2} {}", style(s)),
        RenderCommand::Text {
            text, x, y, size, ..
        } => {
            format!("text ({x:.2},{y:.2}) size={size:.2} {text:?}")
        }
        RenderCommand::Candle {
            x,
            open_y,
            high_y,
            low_y,
            close_y,
            width,
            ..
        } => format!(
            "candle x={x:.2} o={open_y:.2} h={high_y:.2} l={low_y:.2} c={close_y:.2} w={width:.2}"
        ),
        RenderCommand::CandlesBatch { candles, .. } => {
            format!("candles_batch n={}", candles.len())
        }
        RenderCommand::Circle { x, y, radius, .. } => {
            format!("circle ({x:.2},{y:.2}) r={radius:.2}")
        }
        RenderCommand::Ellipse {
            cx,
            cy,
            rx,
            ry,
            style: s,
        } => format!("ellipse ({cx:.2},{cy:.2}) {rx:.2}x{ry:.2} {}", style(s)),
        RenderCommand::IndicatorLine { points, width, .. } => {
            format!("indicator_line n={} w={width:.2}", points.len())
        }
    }
}

fn assert_golden(name: &str, actual: String) {
    let path = format!("tests/fixtures/{name}.txt");

    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&path, &actual).expect("Fixture schreiben");
        return;
    }

    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("Fixture {path} fehlt — einmalig mit UPDATE_GOLDEN=1 erzeugen"));

    assert_eq!(
        expected.trim(),
        actual.trim(),
        "Befehlsstrom weicht von {path} ab"
    );
}

/// Deterministische Ausgangslage: fester Seed, feste Maße, feste Zeitspanne.
fn scene() -> (Viewport, ToolManager) {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(42)
            .with_timeframe(Timeframe::M5),
    );
    let candles = generator.generate(200);

    let mut viewport = Viewport::new(800, 400);
    viewport.timeframe = Timeframe::M5;
    let min = candles.iter().map(|c| c.l).fold(f64::MAX, f64::min);
    let max = candles.iter().map(|c| c.h).fold(f64::MIN, f64::max);
    viewport.fit_to_data(
        TimeRange {
            start: candles[0].time,
            end: candles[candles.len() - 1].time,
        },
        PriceRange { min, max },
    );

    let mut tools = ToolManager::new();

    let mut trend = TrendLine::new("trend-1".to_string());
    trend.nodes_mut().push(ToolNode::new(candles[20].time, min));
    trend
        .nodes_mut()
        .push(ToolNode::new(candles[180].time, max));
    tools.add_tool(Box::new(trend));

    let mut level = HorizontalLine::new("level-1".to_string());
    level
        .nodes_mut()
        .push(ToolNode::new(candles[0].time, (min + max) / 2.0));
    tools.add_tool(Box::new(level));

    (viewport, tools)
}

#[test]
fn tools_render_to_a_stable_command_stream() {
    let (viewport, tools) = scene();

    let mut recorder = BatchRenderer::new(800, 400);
    tools.render_all(&mut recorder, &viewport);

    let actual = recorder
        .commands()
        .iter()
        .map(format_command)
        .collect::<Vec<_>>()
        .join("\n");

    assert_golden("tools_trend_and_level", actual);
}

#[test]
fn the_stream_is_reproducible() {
    let render = || {
        let (viewport, tools) = scene();
        let mut recorder = BatchRenderer::new(800, 400);
        tools.render_all(&mut recorder, &viewport);
        recorder
            .commands()
            .iter()
            .map(format_command)
            .collect::<Vec<_>>()
    };

    assert_eq!(
        render(),
        render(),
        "gleicher Seed muss gleichen Strom geben"
    );
}

#[test]
fn culling_keeps_only_visible_candles() {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(42)
            .with_timeframe(Timeframe::M5),
    );
    let candles = generator.generate(500);

    let mut viewport = Viewport::new(800, 400);
    viewport.timeframe = Timeframe::M5;
    viewport.fit_to_data(
        TimeRange {
            start: candles[0].time,
            end: candles[candles.len() - 1].time,
        },
        PriceRange {
            min: 50.0,
            max: 150.0,
        },
    );

    let all = cull_candles(&candles, &viewport).len();
    viewport.zoom(0.25, None);
    let zoomed = cull_candles(&candles, &viewport).len();

    assert!(
        zoomed < all,
        "nach dem Hineinzoomen müssen weniger Kerzen übrig bleiben ({zoomed} statt {all})"
    );
    assert!(zoomed > 0, "es darf nicht alles weggefiltert werden");
}

/// Ein vollständiger Frame — Hintergrund, Gitter, Kerzen, Achsen.
///
/// Vor der Verlagerung des Renderloops in den Kern war dieser Test nicht
/// formulierbar: der Ablauf hing an `wasm-bindgen` und lief nur im Browser.
#[test]
fn a_full_frame_renders_to_a_stable_command_stream() {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(42)
            .with_timeframe(Timeframe::M5),
    );

    let mut state = ChartState::new(800, 400, Timeframe::M5);
    state.set_candles(generator.generate(150));
    state.fit_to_data();

    let mut recorder = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut recorder);

    assert!(
        !recorder.commands().is_empty(),
        "ein Frame muss Befehle erzeugen"
    );

    let actual = recorder
        .commands()
        .iter()
        .map(format_command)
        .collect::<Vec<_>>()
        .join("\n");

    assert_golden("full_frame_candlestick", actual);
}

#[test]
fn a_frame_is_only_drawn_when_the_state_is_dirty() {
    let mut generator = CandleGenerator::new(GeneratorConfig::crypto().with_seed(42));
    let mut state = ChartState::new(800, 400, Timeframe::M5);
    state.set_candles(generator.generate(50));
    state.fit_to_data();

    let mut first = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut first);
    assert!(!first.commands().is_empty());

    // Ohne Zustandsänderung darf kein zweiter Frame entstehen.
    let mut second = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut second);
    assert!(
        second.commands().is_empty(),
        "sauberer Zustand darf nicht neu zeichnen"
    );

    state.mark_dirty();
    let mut third = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut third);
    assert_eq!(
        first.commands().len(),
        third.commands().len(),
        "nach mark_dirty muss derselbe Frame wieder entstehen"
    );
}

#[test]
fn the_viewport_height_survives_the_frame() {
    let mut generator = CandleGenerator::new(GeneratorConfig::crypto().with_seed(42));
    let mut state = ChartState::new(800, 400, Timeframe::M5);
    state.set_candles(generator.generate(50));
    state.fit_to_data();

    let before = state.viewport.dimensions.height;
    let mut recorder = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut recorder);

    assert_eq!(
        before, state.viewport.dimensions.height,
        "der Loop verkleinert die Viewport-Höhe für Panes nur vorübergehend"
    );
}

/// Ein Frame mit einem RSI-Pane — die Werte stammen aus `kestrel-chartkit`.
#[test]
fn a_frame_with_an_indicator_pane_is_stable() {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(42)
            .with_timeframe(Timeframe::M5),
    );

    let mut state = ChartState::new(800, 400, Timeframe::M5);
    state.set_candles(generator.generate(150));
    state.fit_to_data();

    let mut panes = vec![IndicatorPane::new("pane-rsi", "rsi", HashMap::new(), 0.28)
        .expect("rsi ist im Chartkit-Katalog")];
    update_indicator_panes(&mut panes, &state.candles);

    assert!(
        !panes[0].series.values().is_empty(),
        "der RSI muss nach dem Warmup Werte liefern"
    );

    let extras = RenderExtras {
        indicator_panes: &panes,
        ..Default::default()
    };

    let mut recorder = BatchRenderer::new(800, 400);
    render_chart(&mut state, &extras, &mut recorder);

    let actual = recorder
        .commands()
        .iter()
        .map(format_command)
        .collect::<Vec<_>>()
        .join("\n");

    assert_golden("full_frame_with_rsi_pane", actual);
}

#[test]
fn an_unknown_indicator_is_rejected() {
    assert!(IndicatorPane::new("pane-x", "gibtsnicht", HashMap::new(), 0.28).is_err());
}

/// Ein Overlay-Indikator gehört auf den Preischart, nicht in ein eigenes Pane —
/// und er zeichnet alle seine Bänder, nicht nur die Mittellinie.
#[test]
fn bollinger_renders_as_an_overlay_with_its_bands() {
    let mut generator = CandleGenerator::new(
        GeneratorConfig::crypto()
            .with_seed(42)
            .with_timeframe(Timeframe::M5),
    );

    let mut state = ChartState::new(800, 400, Timeframe::M5);
    state.set_candles(generator.generate(150));
    state.fit_to_data();

    let mut panes =
        vec![IndicatorPane::new("pane-bollinger", "bollinger", HashMap::new(), 0.28).unwrap()];
    update_indicator_panes(&mut panes, &state.candles);

    assert_eq!(panes[0].placement(), IndicatorPlacement::Overlay);
    assert_eq!(
        panes[0].series.lines().count(),
        4,
        "Mittellinie, Basis, oberes und unteres Band"
    );

    let extras = RenderExtras {
        indicator_panes: &panes,
        ..Default::default()
    };

    let mut with_overlay = BatchRenderer::new(800, 400);
    render_chart(&mut state, &extras, &mut with_overlay);

    // Ohne Indikator: gleicher Frame, nur ohne die Overlay-Linien.
    state.mark_dirty();
    let mut without = BatchRenderer::new(800, 400);
    render_chart(&mut state, &RenderExtras::default(), &mut without);

    let count = |r: &BatchRenderer| {
        r.commands()
            .iter()
            .filter(|c| matches!(c, RenderCommand::IndicatorLine { .. }))
            .count()
    };
    assert_eq!(count(&without), 0);
    assert_eq!(
        count(&with_overlay),
        4,
        "vier Linien auf dem Preischart, kein eigenes Pane"
    );

    let actual = with_overlay
        .commands()
        .iter()
        .map(format_command)
        .collect::<Vec<_>>()
        .join("\n");

    assert_golden("full_frame_with_bollinger_overlay", actual);
}

/// Ein Overlay darf dem Hauptchart keine Höhe wegnehmen, ein Pane schon.
#[test]
fn an_overlay_does_not_shrink_the_main_chart() {
    let mut generator = CandleGenerator::new(GeneratorConfig::crypto().with_seed(42));
    let candles = generator.generate(100);

    // Unterster Punkt aller gezeichneten Kerzenkörper — er zeigt, wie viel Höhe
    // dem Hauptchart geblieben ist.
    let lowest_candle_y = |indicator: Option<&str>| {
        let mut state = ChartState::new(800, 400, Timeframe::M5);
        state.set_candles(candles.clone());
        state.fit_to_data();
        let mut panes = match indicator {
            Some(name) => vec![IndicatorPane::new("p", name, HashMap::new(), 0.28).unwrap()],
            None => Vec::new(),
        };
        update_indicator_panes(&mut panes, &state.candles);
        let extras = RenderExtras {
            indicator_panes: &panes,
            ..Default::default()
        };
        let mut r = BatchRenderer::new(800, 400);
        render_chart(&mut state, &extras, &mut r);
        r.commands()
            .iter()
            .filter_map(|c| match c {
                // Kerzenkörper, nicht die vollbreiten Flächen der Panes.
                RenderCommand::Rect {
                    y, height, width, ..
                } if *width < 50.0 => Some(y + height),
                _ => None,
            })
            .fold(f64::NEG_INFINITY, f64::max)
    };

    let plain = lowest_candle_y(None);
    let overlay = lowest_candle_y(Some("bollinger"));
    let pane = lowest_candle_y(Some("rsi"));

    assert!(
        (plain - overlay).abs() < 0.5,
        "Overlay verändert die Höhe des Hauptcharts nicht ({plain} vs {overlay})"
    );
    assert!(
        pane < plain,
        "ein Pane verkleinert den Hauptchart ({pane} statt {plain})"
    );
}
