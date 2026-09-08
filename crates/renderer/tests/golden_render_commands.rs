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

use kestrel_loom::core::{
    render_chart, CandleGenerator, ChartState, GeneratorConfig, PriceRange, RenderExtras,
    TimeRange, Timeframe, Viewport,
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
