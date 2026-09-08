//! Werkzeuge ohne Browser zeichnen.
//!
//! Der Kern führt nichts aus, er beschreibt: Werkzeuge schreiben in einen
//! `&mut dyn Renderer`, und der `BatchRenderer` sammelt daraus einen
//! `RenderCommand`-Strom. Genau dieser Strom ist die Stelle, an der sich
//! Rendering ohne Canvas prüfen lässt — Grundlage für Golden-Fixtures.
//!
//! `cargo run -p kestrel-loom --example tools_to_commands`

use kestrel_loom::core::{PriceRange, TimeRange};
use kestrel_loom::tools::{ChartTool, HorizontalLine, ToolManager, ToolNode, TrendLine};
use kestrel_loom::{BatchRenderer, RenderCommand, Viewport};

fn main() {
    let mut viewport = Viewport::new(800, 400);
    viewport.fit_to_data(
        TimeRange {
            start: 1_600_000_000,
            end: 1_600_036_000,
        },
        PriceRange {
            min: 95.0,
            max: 105.0,
        },
    );

    let mut tools = ToolManager::new();

    // Trendlinie über zwei Stützpunkte in Zeit/Preis — nicht in Pixeln.
    let mut trend = TrendLine::new(tools.generate_id("trend"));
    trend.nodes_mut().push(ToolNode::new(1_600_006_000, 97.0));
    trend.nodes_mut().push(ToolNode::new(1_600_030_000, 103.0));
    tools.add_tool(Box::new(trend));

    // Horizontale Linie auf einem Preisniveau.
    let mut level = HorizontalLine::new(tools.generate_id("level"));
    level.nodes_mut().push(ToolNode::new(0, 100.0));
    tools.add_tool(Box::new(level));

    println!("Werkzeuge: {}", tools.count());

    let mut recorder = BatchRenderer::new(800, 400);
    tools.render_all(&mut recorder, &viewport);

    println!("Zeichenbefehle: {}", recorder.commands().len());
    println!();

    // Nach Art zusammenfassen statt alles auszuschütten.
    let mut counts: Vec<(&str, usize)> = Vec::new();
    for cmd in recorder.commands() {
        let kind = match cmd {
            RenderCommand::Clear { .. } => "Clear",
            RenderCommand::Line { .. } => "Line",
            RenderCommand::Rect { .. } => "Rect",
            RenderCommand::Text { .. } => "Text",
            RenderCommand::Candle { .. } => "Candle",
            RenderCommand::CandlesBatch { .. } => "CandlesBatch",
            RenderCommand::Circle { .. } => "Circle",
            RenderCommand::Ellipse { .. } => "Ellipse",
            RenderCommand::IndicatorLine { .. } => "IndicatorLine",
        };
        match counts.iter_mut().find(|(k, _)| *k == kind) {
            Some((_, n)) => *n += 1,
            None => counts.push((kind, 1)),
        }
    }
    for (kind, n) in &counts {
        println!("  {n:>3} x {kind}");
    }

    println!();
    println!("Die ersten Befehle im Detail:");
    for cmd in recorder.commands().iter().take(4) {
        match cmd {
            RenderCommand::Line {
                x1,
                y1,
                x2,
                y2,
                width,
                ..
            } => println!("  Line   ({x1:.1}, {y1:.1}) -> ({x2:.1}, {y2:.1})  w={width}"),
            RenderCommand::Circle { x, y, radius, .. } => {
                println!("  Circle ({x:.1}, {y:.1})  r={radius}")
            }
            other => println!("  {other:?}"),
        }
    }

    // Auffällig: die gestrichelte Preislinie wird als viele kurze Segmente
    // ausgegeben, nicht als ein Befehl mit Strichmuster. Das ist der in
    // loomcharts Roadmap offene Punkt "Dashed/dotted line styles" — hier wird
    // er im Befehlsstrom sichtbar, statt im Canvas zu verschwinden.

    println!();
    println!("Kein Canvas, kein web-sys — der Strom ist vollständig prüfbar.");

    // Werkzeuge lassen sich verlustfrei sichern und zurückladen.
    let json = tools.to_json().expect("Serialisierung");
    let restored = ToolManager::from_json(&json).expect("Deserialisierung");
    println!("Nach Roundtrip wieder {} Werkzeuge.", restored.count());
}
