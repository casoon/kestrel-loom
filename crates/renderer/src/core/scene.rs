//! Chartkit-Szenen zeichnen.
//!
//! `kestrel-chartkit` beschreibt in `viz::scene` ein renderer-neutrales
//! Szenenmodell und rendert daraus bislang nur statisches SVG. Dieses Modul ist
//! der zweite Renderer dafür: dieselbe `Scene`, gezeichnet über
//! [`crate::rendering::Renderer`].
//!
//! **Koordinatenannahme.** Chartkits `Axis` sagt nicht, worin `min`/`max` gemessen
//! sind. Hier gilt: x ist ein Unix-Zeitstempel in Sekunden, y ein Preis — dieselbe
//! Domäne wie `Candle.time` und `Candle.c`. Solange das nicht im Vertrag steht,
//! ist es eine Annahme dieses Consumers; siehe `plan/02-chartkit-vertrag.md` §3.

use kestrel_chartkit::artifact::Artifact;
use kestrel_chartkit::viz::scene::{LineStyle as SceneLineStyle, Scene, SceneObjectKind};

use crate::core::ChartState;
use crate::primitives::Color;
use crate::rendering::{DrawStyle, Renderer};

/// Übersetzt eine Farbangabe der Szene (`"#rrggbb"` oder `"#rrggbbaa"`).
///
/// Unbekannte Angaben werden neutral grau gezeichnet statt verworfen: ein Objekt
/// stillschweigend wegzulassen wäre schlechter, als es in der falschen Farbe zu
/// zeigen.
fn parse_color(spec: &str) -> Color {
    let hex = spec.trim().trim_start_matches('#');
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        6 => match (byte(0), byte(2), byte(4)) {
            (Some(r), Some(g), Some(b)) => Color::rgb(r, g, b),
            _ => Color::rgb(150, 160, 170),
        },
        8 => match (byte(0), byte(2), byte(4), byte(6)) {
            (Some(r), Some(g), Some(b), Some(a)) => Color::rgba(r, g, b, a as f32 / 255.0),
            _ => Color::rgb(150, 160, 170),
        },
        _ => Color::rgb(150, 160, 170),
    }
}

fn with_opacity(color: Color, opacity: f64) -> Color {
    Color::rgba(
        color.r,
        color.g,
        color.b,
        color.a * opacity.clamp(0.0, 1.0) as f32,
    )
}

/// Zeichnet eine Szene über den Preischart.
///
/// Objekte werden in `z_order`-Reihenfolge gezeichnet; `opacity` multipliziert die
/// Deckkraft der Objektfarbe. Panes der Szene werden **nicht** eigenständig
/// aufgeteilt — alle Objekte landen im Preisbereich (siehe Modul-Doku).
pub fn render_scene(scene: &Scene, state: &ChartState, renderer: &mut dyn Renderer) {
    let vp = &state.viewport;
    let x = |time: f64| vp.time_to_x(time as i64);
    let y = |price: f64| vp.price_to_y(price);

    for pane in scene.panes() {
        for object in pane.objects_z_ordered() {
            let alpha = object.opacity;
            match &object.kind {
                SceneObjectKind::Polyline {
                    points,
                    color,
                    style,
                    width,
                } => {
                    let mapped: Vec<(f64, f64)> =
                        points.iter().map(|(t, p)| (x(*t), y(*p))).collect();
                    if mapped.len() < 2 {
                        continue;
                    }
                    let color = with_opacity(parse_color(color), alpha);
                    match style {
                        SceneLineStyle::Solid => renderer.draw_polyline(&mapped, color, *width),
                        // Strich- und Punktlinien als Segmente — der Renderer kennt
                        // noch kein Strichmuster (loomchart-Roadmap 7.1).
                        SceneLineStyle::Dashed | SceneLineStyle::Dotted => {
                            for pair in mapped.chunks(2) {
                                if let [a, b] = pair {
                                    renderer.draw_line(a.0, a.1, b.0, b.1, color, *width as f32);
                                }
                            }
                        }
                    }
                }
                SceneObjectKind::BoundedBox {
                    x0,
                    y0,
                    x1,
                    y1,
                    fill_color,
                    border_color,
                } => {
                    let (px0, px1) = (x(*x0), x(*x1));
                    let (py0, py1) = (y(*y0), y(*y1));
                    let style = DrawStyle {
                        fill_color: fill_color
                            .as_deref()
                            .map(|c| with_opacity(parse_color(c), alpha)),
                        stroke_color: border_color
                            .as_deref()
                            .map(|c| with_opacity(parse_color(c), alpha)),
                        line_width: 1.0,
                    };
                    if let Some(fill) = style.fill_color {
                        renderer.fill_rect(
                            px0.min(px1),
                            py0.min(py1),
                            (px1 - px0).abs(),
                            (py1 - py0).abs(),
                            fill,
                        );
                    }
                    if let Some(stroke) = style.stroke_color {
                        renderer.stroke_rect(
                            px0.min(px1),
                            py0.min(py1),
                            (px1 - px0).abs(),
                            (py1 - py0).abs(),
                            stroke,
                            style.line_width,
                        );
                    }
                }
                SceneObjectKind::Fill { points, color } => {
                    let mapped: Vec<(f64, f64)> =
                        points.iter().map(|(t, p)| (x(*t), y(*p))).collect();
                    if mapped.len() < 2 {
                        continue;
                    }
                    let color = with_opacity(parse_color(color), alpha);
                    let baseline = mapped
                        .iter()
                        .map(|(_, py)| *py)
                        .fold(f64::NEG_INFINITY, f64::max);
                    renderer.draw_area(&mapped, baseline, color, color, 1.0);
                }
                SceneObjectKind::Text {
                    x: tx,
                    y: ty,
                    content,
                    color,
                } => {
                    renderer.draw_text(
                        content,
                        x(*tx),
                        y(*ty),
                        with_opacity(parse_color(color), alpha),
                        11.0,
                        crate::rendering::TextAlign::Left,
                        crate::rendering::TextBaseline::Middle,
                    );
                }
                SceneObjectKind::Tooltip {
                    x: tx,
                    y: ty,
                    content,
                } => {
                    renderer.draw_text(
                        content,
                        x(*tx),
                        y(*ty),
                        with_opacity(state.options.text_color, alpha),
                        11.0,
                        crate::rendering::TextAlign::Left,
                        crate::rendering::TextBaseline::Bottom,
                    );
                }
                SceneObjectKind::Table { x: tx, y: ty, rows } => {
                    let color = with_opacity(state.options.text_color, alpha);
                    for (row, cells) in rows.iter().enumerate() {
                        renderer.draw_text(
                            &cells.join("  "),
                            x(*tx),
                            y(*ty) + row as f64 * 13.0,
                            color,
                            11.0,
                            crate::rendering::TextAlign::Left,
                            crate::rendering::TextBaseline::Top,
                        );
                    }
                }
            }
        }
    }
}

/// Baut aus Chartkit-Artefakten eine Szene.
///
/// Der Weg, den ein Consumer heute gehen muss: Chartkit liefert Artefakte über
/// `IndicatorOutput::artifacts`, aber **niemand baut daraus eine `Scene`** — im
/// ganzen Crate gibt es kein `Scene::new()` außerhalb der Tests.
///
/// `time_range` ist nötig, weil [`Artifact`]s keine Zeitgrenzen tragen: eine Zone
/// kennt nur `price_top`/`price_bottom`. Sie wird deshalb über die gesamte sichtbare
/// Breite gezeichnet.
pub fn scene_from_artifacts(artifacts: &[Artifact], time_range: (i64, i64)) -> Scene {
    use kestrel_chartkit::viz::scene::{Pane, SceneObject};

    let (from, to) = (time_range.0 as f64, time_range.1 as f64);
    let mut pane = Pane::new("artifacts", 1.0);

    for (index, artifact) in artifacts.iter().enumerate() {
        match artifact {
            Artifact::Pivot(p) => {
                pane.upsert_object(SceneObject::new(
                    format!("pivot-{index}"),
                    10,
                    if p.confirmed { 1.0 } else { 0.5 },
                    SceneObjectKind::Polyline {
                        points: vec![
                            (p.timestamp as f64, p.price),
                            (p.timestamp as f64 + 1.0, p.price),
                        ],
                        color: if p.is_high { "#e5534b" } else { "#3fb950" }.to_string(),
                        style: SceneLineStyle::Solid,
                        width: 2.0,
                    },
                ));
            }
            Artifact::Zone(z) => {
                pane.upsert_object(SceneObject::new(
                    format!("zone-{index}"),
                    1,
                    (0.15 + z.strength.clamp(0.0, 1.0) * 0.35).min(0.5),
                    SceneObjectKind::BoundedBox {
                        x0: from,
                        y0: z.price_top,
                        x1: to,
                        y1: z.price_bottom,
                        fill_color: Some("#58a6ff".to_string()),
                        border_color: None,
                    },
                ));
            }
            Artifact::Profile(p) => {
                for (bin_index, bin) in p.bins.iter().enumerate() {
                    // Ohne Zeitbezug am rechten Rand aufgehängt, Breite nach Wert.
                    let span = to - from;
                    let width = span * 0.12 * bin.value.clamp(0.0, 1.0);
                    pane.upsert_object(SceneObject::new(
                        format!("profile-{index}-{bin_index}"),
                        0,
                        0.35,
                        SceneObjectKind::BoundedBox {
                            x0: to - width,
                            y0: bin.price_high,
                            x1: to,
                            y1: bin.price_low,
                            fill_color: Some("#8b949e".to_string()),
                            border_color: None,
                        },
                    ));
                }
            }
            Artifact::Scenario(s) => {
                pane.upsert_object(SceneObject::new(
                    format!("scenario-{index}"),
                    20,
                    1.0,
                    SceneObjectKind::Table {
                        x: from,
                        y: 0.0,
                        rows: vec![vec![
                            s.name.clone(),
                            s.stage.clone(),
                            format!("{:.0}%", s.progress * 100.0),
                        ]],
                    },
                ));
            }
        }
    }

    let mut scene = Scene::new();
    scene.upsert_pane(pane);
    scene
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{CandleGenerator, GeneratorConfig, Timeframe};
    use crate::rendering::{BatchRenderer, RenderCommand};
    use kestrel_chartkit::artifact::{PivotArtifact, ZoneArtifact};
    use kestrel_chartkit::viz::scene::{Pane, SceneObject};

    fn state_with_candles() -> ChartState {
        let mut generator = CandleGenerator::new(GeneratorConfig::crypto().with_seed(42));
        let mut state = ChartState::new(800, 400, Timeframe::M5);
        state.set_candles(generator.generate(120));
        state.fit_to_data();
        state
    }

    #[test]
    fn colors_parse_with_and_without_alpha() {
        assert_eq!(parse_color("#ff0000"), Color::rgb(255, 0, 0));
        let semi = parse_color("#00ff0080");
        assert_eq!((semi.r, semi.g, semi.b), (0, 255, 0));
        assert!(semi.a > 0.4 && semi.a < 0.6);
        // Unbrauchbare Angabe: neutral statt verworfen.
        assert_eq!(parse_color("rebeccapurple"), Color::rgb(150, 160, 170));
    }

    #[test]
    fn a_scene_polyline_becomes_a_draw_command() {
        let state = state_with_candles();
        let (start, end) = (state.candles[0].time, state.candles[50].time);

        let mut pane = Pane::new("p", 1.0);
        pane.upsert_object(SceneObject::new(
            "line",
            0,
            1.0,
            SceneObjectKind::Polyline {
                points: vec![(start as f64, 100.0), (end as f64, 101.0)],
                color: "#58a6ff".to_string(),
                style: SceneLineStyle::Solid,
                width: 1.5,
            },
        ));
        let mut scene = Scene::new();
        scene.upsert_pane(pane);

        let mut recorder = BatchRenderer::new(800, 400);
        render_scene(&scene, &state, &mut recorder);

        assert_eq!(recorder.commands().len(), 1);
        assert!(matches!(
            recorder.commands()[0],
            RenderCommand::IndicatorLine { .. }
        ));
    }

    #[test]
    fn z_order_decides_the_drawing_order() {
        let state = state_with_candles();
        let mut pane = Pane::new("p", 1.0);
        for (id, z) in [("spaet", 5), ("frueh", 1)] {
            pane.upsert_object(SceneObject::new(
                id,
                z,
                1.0,
                SceneObjectKind::Text {
                    x: state.candles[0].time as f64,
                    y: 100.0,
                    content: id.to_string(),
                    color: "#ffffff".to_string(),
                },
            ));
        }
        let mut scene = Scene::new();
        scene.upsert_pane(pane);

        let mut recorder = BatchRenderer::new(800, 400);
        render_scene(&scene, &state, &mut recorder);

        let texts: Vec<&str> = recorder
            .commands()
            .iter()
            .filter_map(|c| match c {
                RenderCommand::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(texts, ["frueh", "spaet"]);
    }

    #[test]
    fn artifacts_become_scene_objects() {
        let state = state_with_candles();
        let range = (state.candles[0].time, state.candles[119].time);

        let artifacts = vec![
            Artifact::Pivot(PivotArtifact {
                timestamp: state.candles[40].time,
                price: 101.0,
                is_high: true,
                confirmed: true,
            }),
            Artifact::Zone(ZoneArtifact {
                kind: "supply".to_string(),
                price_top: 102.0,
                price_bottom: 101.0,
                strength: 0.8,
                touches: 3,
            }),
        ];

        let scene = scene_from_artifacts(&artifacts, range);
        let mut recorder = BatchRenderer::new(800, 400);
        render_scene(&scene, &state, &mut recorder);

        assert_eq!(
            recorder.commands().len(),
            2,
            "ein Pivot-Strich und eine Zonenfläche"
        );
    }
}
