//! Der Renderloop: aus Chart-Zustand werden Zeichenbefehle.
//!
//! Bis 2026-09-08 lag dieser Ablauf in der WASM-Fassade und war damit an
//! `wasm-bindgen` gebunden — nicht ohne Browser ausführbar und nicht testbar.
//! Hier schreibt er in einen `&mut dyn Renderer`: im Browser der Canvas-Renderer,
//! im Test der `BatchRenderer`, der den Befehlsstrom mitschreibt.

use std::collections::HashMap;

use crate::core::indicators::{IndicatorPlacement, IndicatorSeries};
use crate::core::scrollbar::{ScrollbarGeometry, SCROLLBAR_HEIGHT};
use crate::core::types::Seconds;
use crate::core::{Candle, ChartState, FootprintCandle};
use crate::primitives::Color;
use crate::rendering::Renderer;
use crate::utils::ticks::{self, TARGET_INTERVALS};

/// Ein Vergleichsinstrument, das über den Hauptchart gelegt wird.
pub struct CompareSymbol {
    pub symbol: String,
    pub candles: Vec<Candle>,
    pub color: Color,
}

/// Ein Indikator-Pane unterhalb des Hauptcharts.
///
/// Trägt seine eigene, laufende Indikator-Instanz — die Werte werden inkrementell
/// fortgeschrieben (`update_indicator_panes`), nicht bei jedem Frame neu gerechnet.
pub struct IndicatorPane {
    pub pane_id: String,
    pub indicator_id: String,
    pub params_json: String,
    pub height_fraction: f64,
    pub series: IndicatorSeries,
}

impl IndicatorPane {
    /// Wohin dieser Indikator gezeichnet wird.
    pub fn placement(&self) -> IndicatorPlacement {
        self.series.placement()
    }

    /// Belegt dieser Indikator eigene Höhe? Overlays liegen auf dem Preischart.
    pub fn has_own_pane(&self) -> bool {
        self.placement() == IndicatorPlacement::Pane
    }

    /// Legt ein Pane für einen Indikator aus dem Chartkit-Katalog an.
    pub fn new(
        pane_id: impl Into<String>,
        indicator_id: &str,
        params: HashMap<String, f64>,
        height_fraction: f64,
    ) -> Result<Self, String> {
        let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());
        Ok(Self {
            pane_id: pane_id.into(),
            indicator_id: indicator_id.to_string(),
            params_json,
            height_fraction,
            series: IndicatorSeries::new(indicator_id, params)?,
        })
    }
}

/// Schreibt die Indikatorwerte aller Panes fort.
///
/// Vor `render_chart` aufzurufen: der Renderloop selbst rechnet nicht, er liest nur.
pub fn update_indicator_panes(panes: &mut [IndicatorPane], candles: &[Candle]) {
    for pane in panes {
        pane.series.feed(candles);
    }
}

/// Alles, was der Loop außer dem Chart-Zustand braucht.
#[derive(Default)]
pub struct RenderExtras<'a> {
    pub indicator_panes: &'a [IndicatorPane],
    pub compare_symbols: &'a [CompareSymbol],
    pub footprint_candles: &'a [FootprintCandle],
    /// Eine von außen gereichte Chartkit-Szene, die über den Preischart gelegt wird.
    /// Siehe [`crate::core::scene`].
    pub scene: Option<&'a kestrel_chartkit::viz::scene::Scene>,
    /// Wenn gesetzt, werden zusätzlich die Artefakte der Indikatoren gezeichnet —
    /// Zonen, Pivots und Profile, die die Indikatoren ohnehin melden.
    pub draw_indicator_artifacts: bool,
}

/// Sammelt die Artefakte aller Panes zu einer Szene.
///
/// Die Abbildung Artefakt → Szenenobjekt liegt in `kestrel-chartkit`; hier wird nur
/// eingesammelt. `fallback_span` fängt Artefakte auf, die keine eigene Zeitspanne
/// tragen — seit Chartkit 0.2.0 die Ausnahme.
/// Platz, den eine Achsenbeschriftung mindestens braucht. Bestimmt, welche
/// Tick-Stufe die Achse wählt.
const AXIS_LABEL_WIDTH_PX: f64 = 64.0;
/// Breite der Preisachsenspalte rechts.
pub const PRICE_AXIS_WIDTH_PX: f64 = 64.0;
/// Höhe der Zeitachse unten.
pub const TIME_AXIS_HEIGHT_PX: f64 = 20.0;
/// Schriftgröße der Achsenbeschriftung und der Pane-Titel.
const LABEL_FONT_PX: f32 = 11.0;
/// Preismarken, die dichter als so viele Pixel liegen, werden nur jede n-te beschriftet.
const MIN_LABEL_GAP_PX: f64 = 14.0;
/// Anteil der Preis-Pane, den das Volumen unten einnimmt.
const VOLUME_BAND_SHARE: f64 = 0.22;
/// Höhe des Titelstreifens über jedem Indikator-Pane.
const PANE_TITLE_BAND_PX: f64 = 18.0;

/// Lage der Zeitleiste: über der Zeitachse, über die Inhaltsbreite.
pub fn scrollbar_geometry(width: f64, height: f64) -> ScrollbarGeometry {
    ScrollbarGeometry {
        x: 0.0,
        y: height - TIME_AXIS_HEIGHT_PX - SCROLLBAR_HEIGHT,
        width: (width - PRICE_AXIS_WIDTH_PX).max(0.0),
        height: SCROLLBAR_HEIGHT,
    }
}

/// Zeichnet die Zeitleiste.
///
/// Zurückhaltend, solange der Zeiger woanders ist, und deutlich, sobald er sie
/// berührt — statt eines Timers, den der Kern ohne hereingereichte Zeit gar
/// nicht führen könnte.
fn render_scrollbar(state: &ChartState, renderer: &mut dyn Renderer) {
    let vp = &state.viewport;
    let width = vp.dimensions.width as f64;
    let height = vp.dimensions.height as f64;
    let geometry = scrollbar_geometry(width, height);
    if geometry.width <= 0.0 || vp.bar_count() == 0 {
        return;
    }

    let thumb = geometry.thumb(vp.bars(), vp.bar_count());
    let hovered = state.crosshair.visible
        && geometry
            .hit(
                vp.bars(),
                vp.bar_count(),
                state.crosshair.x,
                state.crosshair.y,
            )
            .is_some();
    let active = hovered || state.is_scrolling();

    let track = state
        .options
        .grid_color
        .with_alpha(if active { 0.55 } else { 0.28 });
    let handle = state
        .options
        .text_color
        .with_alpha(if active { 0.72 } else { 0.34 });

    // Bahn als dünner Strich in der Mitte, Griff als volle Höhe: der Griff
    // bleibt dadurch auch bei geringem Kontrast als Objekt erkennbar.
    let mid = geometry.y + geometry.height / 2.0;
    renderer.draw_line(
        geometry.x,
        mid,
        geometry.x + geometry.width,
        mid,
        track,
        1.0,
    );
    renderer.fill_rect(
        thumb.x,
        geometry.y + 2.0,
        thumb.width,
        geometry.height - 4.0,
        handle,
    );
}

pub fn scene_from_indicator_panes(
    panes: &[IndicatorPane],
    fallback_span: Option<(i64, i64)>,
) -> Option<kestrel_chartkit::viz::scene::Scene> {
    let artifacts: Vec<_> = panes
        .iter()
        .flat_map(|pane| pane.series.artifacts().iter().cloned())
        .collect();
    if artifacts.is_empty() {
        return None;
    }
    Some(kestrel_chartkit::viz::scene_from_artifacts(
        &artifacts,
        fallback_span,
    ))
}

/// Zeichnet einen vollständigen Frame.
///
/// Verändert `state` nur vorübergehend (die Viewport-Höhe wird für die
/// Panes angepasst und danach zurückgesetzt) und markiert ihn am Ende als sauber.
pub fn render_chart(state: &mut ChartState, extras: &RenderExtras, renderer: &mut dyn Renderer) {
    let RenderExtras {
        indicator_panes,
        compare_symbols,
        footprint_candles,
        scene,
        draw_indicator_artifacts,
    } = *extras;

    if !state.is_dirty() {
        return;
    }

    renderer.begin_frame();

    // Clear background
    let bg_color = state.options.background_color;
    renderer.clear(bg_color);

    // ── Compute main chart height once (shared by all rendering passes) ──
    // main_height excludes indicator pane area at the bottom.
    // We shrink the viewport height to main_height so that price_to_y and
    // all grid/candle Y calculations map correctly to the main chart area.
    let full_height = state.viewport.dimensions.height;
    let main_height = main_chart_height(indicator_panes, full_height as f64);
    state.viewport.dimensions.height = main_height as u32;

    // Draw grid
    if state.options.show_grid {
        let grid_color = state.options.grid_color;
        let vp = &state.viewport;

        // Das Gitter liegt auf denselben glatten Marken wie die Preisbeschriftung.
        // Im Log-Modus bleibt die logarithmische Teilung.
        let plot_w = vp.plot_width();
        let grid_prices: Vec<f64> = if vp.log_scale {
            vp.log_grid_prices(10)
        } else {
            ticks::price_ticks(vp.price.min, vp.price.max, TARGET_INTERVALS).values
        };

        for price in &grid_prices {
            let y = vp.price_to_y(*price);
            renderer.draw_line(0.0, y, plot_w, y, grid_color, 1.0);
        }

        // Vertikale Gitterlinien stehen an denselben Kalendersprüngen wie die
        // Achsenbeschriftung — nicht in gleichen Pixelabständen. Sonst liefe das
        // Gitter neben den Beschriftungen her.
        for tick in crate::core::time_axis::axis_ticks(
            vp.bar_index(),
            vp.bars(),
            vp.plot_width(),
            vp.timezone_offset_minutes,
            AXIS_LABEL_WIDTH_PX,
        ) {
            let x = vp.time_to_x(tick.time);
            renderer.draw_line(x, 0.0, x, vp.dimensions.height as f64, grid_color, 1.0);
        }
    }

    // Kerzen und Volumen bleiben im Hauptbereich: Pane-Hintergründe und Achsen
    // überdecken sonst, was darüber hinausragt.
    renderer.set_clip(0.0, 0.0, state.viewport.plot_width(), main_height);

    // Chartkit-Szenen in zwei Durchgängen: Flächen (`z_order` ≤ 0, etwa Zonen)
    // liegen hinter den Kerzen, Linien, Marker und Beschriftungen davor.
    let artifact_scene = if draw_indicator_artifacts {
        let fallback = match (state.candles().first(), state.candles().last()) {
            (Some(first), Some(last)) => Some((first.time.get(), last.time.get())),
            _ => None,
        };
        scene_from_indicator_panes(indicator_panes, fallback)
    } else {
        None
    };
    for layer_scene in artifact_scene.iter().chain(scene) {
        crate::core::scene::render_scene_layer(
            layer_scene,
            state,
            renderer,
            crate::core::scene::SceneLayer::Behind,
        );
    }

    if state.options.show_volume {
        render_volume(state, renderer, main_height);
    }

    // Draw candles with TradingView-style optimal width calculation
    {
        use crate::utils::bar_width::{optimal_candlestick_width, symmetric_bar_width};

        let vp = &state.viewport;
        let visible_candles = state.visible_candles();
        let bar_spacing = vp.bar_width();
        let pixel_ratio = vp.dimensions.pixel_ratio;
        let bullish_color = state.options.bullish_color;
        let bearish_color = state.options.bearish_color;
        let unchanged_color = state.options.unchanged_color;

        // Calculate optimal candlestick width using TradingView algorithm
        let optimal_width = if vp.bar_width_ratio > 0.0 {
            bar_spacing * pixel_ratio * vp.bar_width_ratio
        } else {
            optimal_candlestick_width(bar_spacing, pixel_ratio)
        };
        let (bar_width, _line_width) = symmetric_bar_width(optimal_width, pixel_ratio, false);

        let candle_style = state.options.candle_style;

        // Abgeleitete Serien (Renko, Heikin Ashi) werden vor dem Zeichnen
        // umgerechnet; der Renderpfad danach ist derselbe.
        let renko_bricks: Vec<Candle>;
        let heikin_ashi: Vec<Candle>;
        let owned_visible: Vec<Candle>;
        let render_candles: &[Candle] =
            if let crate::primitives::CandleStyle::Renko { brick_size } = candle_style {
                renko_bricks = crate::core::renko::compute_renko(state.candles(), brick_size);
                &renko_bricks
            } else if candle_style == crate::primitives::CandleStyle::HeikinAshi {
                heikin_ashi = crate::core::heikin_ashi::compute_heikin_ashi(state.candles());
                &heikin_ashi
            } else {
                owned_visible = visible_candles.iter().map(|c| (*c).clone()).collect();
                &owned_visible
            };

        match candle_style {
            crate::primitives::CandleStyle::Line | crate::primitives::CandleStyle::Area => {
                // Render as polyline / filled area through close prices
                let points: Vec<(f64, f64)> = render_candles
                    .iter()
                    .map(|c| (vp.time_to_x(c.time), vp.price_to_y(c.c)))
                    .collect();

                let line_color = bullish_color;
                if candle_style == crate::primitives::CandleStyle::Area {
                    // Build a semi-transparent fill from the line color
                    let fill = crate::primitives::Color {
                        r: line_color.r,
                        g: line_color.g,
                        b: line_color.b,
                        a: 0.15,
                    };
                    let baseline_y = vp.dimensions.height as f64;
                    renderer.draw_area(&points, baseline_y, fill, line_color, 2.0);
                } else {
                    renderer.draw_polyline(&points, line_color, 2.0);
                }
            }
            crate::primitives::CandleStyle::Renko { brick_size: _ } => {
                // Renko bricks rendered as filled rectangles
                let visible_time = vp.time_range();
                for candle in render_candles {
                    if candle.time < visible_time.start || candle.time > visible_time.end {
                        continue;
                    }
                    let x = vp.time_to_x(candle.time);
                    let top_y = vp.price_to_y(candle.h);
                    let bot_y = vp.price_to_y(candle.l);
                    let height = (bot_y - top_y).abs().max(1.0);
                    let w = bar_width / pixel_ratio;
                    let color = if candle.c >= candle.o {
                        bullish_color
                    } else {
                        bearish_color
                    };
                    renderer.fill_rect(x - w / 2.0, top_y, w, height, color);
                    renderer.stroke_rect(x - w / 2.0, top_y, w, height, unchanged_color, 0.5);
                }
            }
            crate::primitives::CandleStyle::Footprint => {
                render_footprint_candles(
                    footprint_candles,
                    state,
                    renderer,
                    bar_width / pixel_ratio,
                );
            }
            _ => {
                for candle in render_candles {
                    let x = vp.time_to_x(candle.time);
                    let open_y = vp.price_to_y(candle.o);
                    let high_y = vp.price_to_y(candle.h);
                    let low_y = vp.price_to_y(candle.l);
                    let close_y = vp.price_to_y(candle.c);
                    let width = bar_width / pixel_ratio; // Convert back to CSS pixels

                    match candle_style {
                        crate::primitives::CandleStyle::Candlestick => {
                            renderer.draw_candle(
                                x,
                                open_y,
                                high_y,
                                low_y,
                                close_y,
                                width,
                                bullish_color,
                                bearish_color,
                                unchanged_color,
                            );
                        }
                        crate::primitives::CandleStyle::OHLC => {
                            renderer.draw_ohlc(
                                x,
                                open_y,
                                high_y,
                                low_y,
                                close_y,
                                width,
                                bullish_color,
                                bearish_color,
                                unchanged_color,
                            );
                        }
                        crate::primitives::CandleStyle::Hollow => {
                            renderer.draw_hollow_candle(
                                x,
                                open_y,
                                high_y,
                                low_y,
                                close_y,
                                width,
                                bullish_color,
                                bearish_color,
                                unchanged_color,
                            );
                        }
                        // Heikin Ashi ist eine abgeleitete Serie: die Kerzen sind
                        // bereits umgerechnet und werden ganz normal gezeichnet.
                        // Vorher stand hier `unreachable!()` — eine neue Variante
                        // von `CandleStyle` ließ den Renderer damit abstürzen.
                        _ => {
                            renderer.draw_candle(
                                x,
                                open_y,
                                high_y,
                                low_y,
                                close_y,
                                width,
                                bullish_color,
                                bearish_color,
                                unchanged_color,
                            );
                        }
                    }
                }
            }
        }
    }

    renderer.clear_clip();

    // Draw session markers (rendered behind candles — drawn before tools)
    if state.options.show_sessions && !state.options.sessions.is_empty() {
        let vp = &state.viewport;
        let tf_secs = state.timeframe.duration_secs();
        // Only show session markers for timeframes <= 1h (3600s)
        if tf_secs <= 3600 {
            let chart_width = vp.plot_width();
            let chart_height = vp.dimensions.height as f64;
            let visible_time = vp.time_range();
            let time_start = visible_time.start.get();
            let time_end = visible_time.end.get();

            // Iterate over each day in the visible range (±1 day buffer)
            let day_secs: i64 = 86400;
            let first_day = ((time_start - day_secs) / day_secs) * day_secs;
            let last_day = ((time_end + day_secs) / day_secs) * day_secs;

            for session in &state.options.sessions.clone() {
                let line_color = crate::primitives::Color::rgba(
                    session.color.0,
                    session.color.1,
                    session.color.2,
                    session.color.3 as f32 / 255.0,
                );

                let mut day = first_day;
                while day <= last_day {
                    if session.show_open {
                        let open_ts =
                            day + session.open_utc.0 as i64 * 3600 + session.open_utc.1 as i64 * 60;
                        if open_ts >= time_start && open_ts <= time_end {
                            let x = vp.time_to_x(crate::core::Seconds::new(open_ts));
                            if x >= 0.0 && x <= chart_width {
                                renderer.draw_line(x, 0.0, x, chart_height, line_color, 1.0);
                            }
                        }
                    }
                    if session.show_close {
                        let close_ts = day
                            + session.close_utc.0 as i64 * 3600
                            + session.close_utc.1 as i64 * 60;
                        if close_ts >= time_start && close_ts <= time_end {
                            let x = vp.time_to_x(crate::core::Seconds::new(close_ts));
                            if x >= 0.0 && x <= chart_width {
                                let dashed_color = crate::primitives::Color::rgba(
                                    session.color.0,
                                    session.color.1,
                                    session.color.2,
                                    (session.color.3 as f32 / 255.0) * 0.5,
                                );
                                renderer.draw_line(x, 0.0, x, chart_height, dashed_color, 1.0);
                            }
                        }
                    }
                    day += day_secs;
                }
            }
        }
    }

    // Bis hierher bleibt die Viewport-Höhe die des Hauptbereichs: Overlays, Szenen,
    // Vergleiche und Werkzeuge bilden Preise über dieselbe Skala ab wie die Kerzen.
    // Mit der vollen Höhe lägen sie bei jedem Indikator-Pane neben den Kerzen.
    renderer.set_clip(0.0, 0.0, state.viewport.plot_width(), main_height);
    render_overlay_indicators(indicator_panes, state, renderer);

    for layer_scene in artifact_scene.iter().chain(scene) {
        crate::core::scene::render_scene_layer(
            layer_scene,
            state,
            renderer,
            crate::core::scene::SceneLayer::Front,
        );
    }

    render_compare_symbols(compare_symbols, state, renderer, main_height);
    renderer.clear_clip();

    // Draw drawing tools
    {
        let vp = &state.viewport;
        let tools = state.tool_manager.tools();

        renderer.set_clip(0.0, 0.0, vp.plot_width(), main_height);
        for tool in tools {
            tool.render(renderer, vp);
        }
        renderer.clear_clip();
    }

    renderer.set_clip(0.0, 0.0, state.viewport.plot_width(), main_height);
    render_selected_tool_highlights(state, renderer);
    renderer.clear_clip();

    // Volle Zeichenhöhe für Panes, Achsen und Fadenkreuz.
    state.viewport.dimensions.height = full_height;
    render_indicator_panes(indicator_panes, state, renderer);

    // Draw crosshair with pixel-perfect rendering
    if state.options.show_crosshair && state.crosshair.visible {
        let crosshair = &state.crosshair;
        let color = state.options.crosshair_color;
        let vp = &state.viewport;

        // Das Fadenkreuz rastet auf der Kerze unter dem Zeiger ein: die Ablesung
        // gehört zu einer Kerze, nicht zu einem Pixel dazwischen.
        let x = if vp.bar_count() > 0 {
            let bar = vp.x_to_bar(crosshair.x).round();
            vp.bar_to_x(bar.clamp(0.0, vp.bar_count() as f64 - 1.0))
        } else {
            crosshair.x
        };
        let plot_w = vp.plot_width();
        if x >= 0.0 && x <= plot_w {
            renderer.draw_vertical_line(x, 0.0, vp.dimensions.height as f64, &color, 1.0);
        }
        if crosshair.x <= plot_w {
            renderer.draw_horizontal_line(crosshair.y, 0.0, plot_w, &color, 1.0);
        }
    }

    render_axes(state, indicator_panes, renderer);

    if state.options.show_scrollbar {
        render_scrollbar(state, renderer);
    }

    renderer.end_frame();

    state.clear_dirty();
}

fn render_compare_symbols(
    compare_symbols: &[CompareSymbol],
    state: &ChartState,
    renderer: &mut dyn Renderer,
    chart_height: f64,
) {
    if compare_symbols.is_empty() {
        return;
    }

    let vp = &state.viewport;
    let chart_width = vp.dimensions.width as f64;
    let visible_time = vp.time_range();
    let time_start = visible_time.start;
    let time_end = visible_time.end;

    let mut series = Vec::new();
    let mut percent_min = 0.0_f64;
    let mut percent_max = 0.0_f64;

    for entry in compare_symbols {
        let visible: Vec<&Candle> = entry
            .candles
            .iter()
            .filter(|candle| candle.time >= time_start && candle.time <= time_end)
            .collect();
        if visible.len() < 2 {
            continue;
        }

        let base_close = visible
            .iter()
            .find(|candle| candle.c.is_finite() && candle.c > 0.0)
            .map(|candle| candle.c);
        let Some(base_close) = base_close else {
            continue;
        };

        let values: Vec<(Seconds, f64)> = visible
            .iter()
            .filter_map(|candle| {
                if candle.c.is_finite() {
                    Some((candle.time, ((candle.c / base_close) - 1.0) * 100.0))
                } else {
                    None
                }
            })
            .collect();

        if values.len() < 2 {
            continue;
        }

        for (_, percent) in &values {
            percent_min = percent_min.min(*percent);
            percent_max = percent_max.max(*percent);
        }

        series.push((entry.symbol.as_str(), entry.color, values));
    }

    if series.is_empty() {
        return;
    }

    let padding = ((percent_max - percent_min).abs() * 0.12).max(1.0);
    percent_min -= padding;
    percent_max += padding;
    let percent_range = (percent_max - percent_min).max(1.0);
    let percent_to_y =
        |percent: f64| chart_height - ((percent - percent_min) / percent_range) * chart_height;

    let axis_color = state.options.text_color.with_alpha(0.65);
    let grid_color = state.options.grid_color.with_alpha(0.45);

    let axis_x = chart_width - 1.0;
    renderer.draw_line(axis_x, 0.0, axis_x, chart_height, axis_color, 1.0);

    for i in 0..=4 {
        let percent = percent_min + (percent_range * i as f64 / 4.0);
        let y = percent_to_y(percent);
        renderer.draw_horizontal_line(y, chart_width - 42.0, chart_width, &grid_color, 1.0);
        renderer.draw_text(
            &format!("{:+.1}%", percent),
            chart_width - 4.0,
            y,
            axis_color,
            10.0,
            crate::rendering::TextAlign::Right,
            crate::rendering::TextBaseline::Middle,
        );
    }

    for (idx, (symbol, color, values)) in series.iter().enumerate() {
        // Über den Bar-Index des **Hauptinstruments** abgebildet, nicht über
        // Zeitstempel: `time_to_x` fragt den Hauptindex. Eine Bar des zweiten
        // Instruments, die in eine Handelspause des ersten fällt, landet damit
        // zwischen dessen Nachbarbars, statt eine Position vorzutäuschen; außer-
        // halb des Bestands wird über die Bar-Dauer extrapoliert.
        let points: Vec<(f64, f64)> = values
            .iter()
            .map(|(time, percent)| (vp.time_to_x(*time), percent_to_y(*percent)))
            .collect();
        renderer.draw_polyline(&points, *color, 1.8);

        if let Some((_, latest_percent)) = values.last() {
            let legend_y = 8.0 + idx as f64 * 15.0;
            renderer.draw_text(
                &format!("{} {:+.2}%", symbol, latest_percent),
                8.0,
                legend_y,
                *color,
                11.0,
                crate::rendering::TextAlign::Left,
                crate::rendering::TextBaseline::Top,
            );
        }
    }
}

/// Breite eines Kerzenkörpers in CSS-Pixeln — dieselbe für Kerzen und Volumen.
fn body_width_css(vp: &crate::core::viewport::Viewport) -> f64 {
    use crate::utils::bar_width::{optimal_candlestick_width, symmetric_bar_width};
    let pixel_ratio = vp.dimensions.pixel_ratio;
    let bar_spacing = vp.bar_width();
    let optimal = if vp.bar_width_ratio > 0.0 {
        bar_spacing * pixel_ratio * vp.bar_width_ratio
    } else {
        optimal_candlestick_width(bar_spacing, pixel_ratio)
    };
    let (width, _) = symmetric_bar_width(optimal, pixel_ratio, false);
    width / pixel_ratio
}

/// Volumen als Histogramm im unteren Teil der Preis-Pane, ab null, gedämpft.
///
/// Es zeichnet, was in den Kerzen steht. Ob dieses Volumen echt ist, entscheidet der
/// Aufrufer: er schaltet `show_volume` nur dort ein, wo eine echte Quelle vorliegt.
fn render_volume(state: &ChartState, renderer: &mut dyn Renderer, main_height: f64) {
    let vp = &state.viewport;
    let visible = state.visible_candles();
    let max_volume = visible.iter().map(|c| c.v).fold(0.0_f64, f64::max);
    if max_volume.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
        return;
    }
    let band = main_height * VOLUME_BAND_SHARE;
    let width = body_width_css(vp).max(1.0);
    let up = state.options.bullish_color.with_alpha(0.3);
    let down = state.options.bearish_color.with_alpha(0.3);
    for candle in visible {
        if candle.v <= 0.0 || !candle.v.is_finite() {
            continue;
        }
        let height = candle.v / max_volume * band;
        let x = vp.time_to_x(candle.time) - width / 2.0;
        let color = if candle.c >= candle.o { up } else { down };
        renderer.fill_rect(x, main_height - height, width, height, color);
    }
}

/// Höhenanteil aller Indikatoren mit eigenem Pane. Overlays zählen nicht mit —
/// sie liegen auf dem Preischart und kosten keine Höhe.
pub fn pane_height_total(indicator_panes: &[IndicatorPane]) -> f64 {
    indicator_panes
        .iter()
        .filter(|pane| pane.has_own_pane())
        .map(|pane| pane.height_fraction)
        .sum()
}

fn main_chart_height(indicator_panes: &[IndicatorPane], height: f64) -> f64 {
    let indicator_total = pane_height_total(indicator_panes);
    if indicator_total <= 0.0 {
        return height;
    }
    height * (1.0 - indicator_total).max(0.38)
}

/// Farbe der n-ten Linie eines Indikators — vier ruhige Töne je Theme, wie im
/// ruhigen Chart (chartlet). Höchstens vier Farben: mehr trennt das Auge nicht, und
/// die Kerzen sollen führen.
fn line_color(index: usize, dark: bool) -> Color {
    let (r, g, b) = match (dark, index) {
        (true, 0) => (126, 166, 255),
        (true, 1) => (255, 156, 114),
        (true, 2) => (154, 167, 189),
        (true, _) => (226, 232, 244),
        (false, 0) => (37, 99, 235),
        (false, 1) => (194, 65, 12),
        (false, 2) => (71, 85, 105),
        (false, _) => (17, 24, 39),
    };
    Color::rgba(r, g, b, if index == 0 { 0.95 } else { 0.85 })
}

/// Ist der Hintergrund dunkel? Entscheidet über die Linienpalette.
fn is_dark(state: &ChartState) -> bool {
    let bg = state.options.background_color;
    u16::from(bg.r) + u16::from(bg.g) + u16::from(bg.b) < 384
}

/// Zeichnet Indikatoren, deren Werte in Preiseinheiten liegen, auf den Preischart.
fn render_overlay_indicators(
    indicator_panes: &[IndicatorPane],
    state: &ChartState,
    renderer: &mut dyn Renderer,
) {
    let vp = &state.viewport;
    let visible_time = vp.time_range();
    for pane in indicator_panes
        .iter()
        .filter(|pane| pane.placement() == IndicatorPlacement::Overlay)
    {
        for (index, line) in pane.series.lines().enumerate() {
            let points: Vec<(f64, f64)> = line
                .points
                .iter()
                .filter(|(time, _)| *time >= visible_time.start && *time <= visible_time.end)
                .map(|(time, value)| (vp.time_to_x(*time), vp.price_to_y(*value)))
                .collect();
            if points.len() >= 2 {
                renderer.draw_polyline(&points, line_color(index, is_dark(state)), 1.4);
            }
        }
    }
}

fn render_footprint_candles(
    footprint_candles: &[FootprintCandle],
    state: &ChartState,
    renderer: &mut dyn Renderer,
    candle_width: f64,
) {
    if footprint_candles.is_empty() || candle_width < 20.0 {
        return;
    }

    let vp = &state.viewport;
    let visible_time = vp.time_range();
    let visible: Vec<&FootprintCandle> = footprint_candles
        .iter()
        .filter(|candle| candle.time >= visible_time.start && candle.time <= visible_time.end)
        .collect();
    if visible.is_empty() {
        return;
    }

    let price_per_pixel = ((vp.price.max - vp.price.min) / vp.dimensions.height as f64)
        .abs()
        .max(0.0000001);
    let bid_color = crate::primitives::Color::rgba(248, 81, 73, 0.72);
    let ask_color = crate::primitives::Color::rgba(63, 185, 80, 0.72);
    let poc_color = crate::primitives::Color::rgba(227, 179, 65, 0.32);
    let text_color = state.options.text_color.with_alpha(0.78);
    let slot_w = candle_width * 0.9;
    let half_slot = slot_w / 2.0;
    let show_delta_text = candle_width >= 42.0;

    for candle in visible {
        if candle.levels.is_empty() {
            continue;
        }

        let cx = vp.time_to_x(candle.time);
        let max_vol = candle.max_level_volume().max(1.0);
        let poc_price = candle.poc_price();
        let row_height = if candle.levels.len() >= 2 {
            let step = (candle.levels[1].price - candle.levels[0].price).abs();
            (step / price_per_pixel).clamp(3.0, 18.0)
        } else {
            8.0
        };

        for level in &candle.levels {
            let y = vp.price_to_y(level.price);
            let y_top = y - row_height / 2.0;

            if poc_price == Some(level.price) {
                renderer.fill_rect(cx - half_slot, y_top, slot_w, row_height, poc_color);
            }

            if level.bid_volume > 0.0 {
                let width = (level.bid_volume / max_vol) * half_slot;
                renderer.fill_rect(
                    cx - width,
                    y_top + 1.0,
                    width,
                    (row_height - 2.0).max(1.0),
                    bid_color,
                );
            }

            if level.ask_volume > 0.0 {
                let width = (level.ask_volume / max_vol) * half_slot;
                renderer.fill_rect(
                    cx,
                    y_top + 1.0,
                    width,
                    (row_height - 2.0).max(1.0),
                    ask_color,
                );
            }

            if show_delta_text {
                let delta = crate::core::FootprintCandle::level_delta(level);
                renderer.draw_text(
                    &format!("{:+.0}", delta),
                    cx,
                    y,
                    text_color,
                    8.0,
                    crate::rendering::TextAlign::Center,
                    crate::rendering::TextBaseline::Middle,
                );
            }
        }
    }
}

fn render_selected_tool_highlights(state: &ChartState, renderer: &mut dyn Renderer) {
    if state.selected_tools.is_empty() {
        return;
    }

    let highlight = crate::primitives::Color::rgba(88, 166, 255, 0.95);
    let fill = crate::primitives::Color::rgba(88, 166, 255, 0.22);

    for tool in state.tool_manager.tools() {
        if !state.selected_tools.iter().any(|id| id == tool.id()) {
            continue;
        }

        let mut min_x = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        for node in tool.nodes() {
            let x = state.viewport.time_to_x(node.time);
            let y = state.viewport.price_to_y(node.price);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            renderer.fill_rect(x - 3.0, y - 3.0, 6.0, 6.0, fill);
            renderer.stroke_rect(x - 3.0, y - 3.0, 6.0, 6.0, highlight, 1.0);
        }

        if min_x.is_finite() {
            let pad = 5.0;
            renderer.stroke_rect(
                min_x - pad,
                min_y - pad,
                (max_x - min_x).max(1.0) + pad * 2.0,
                (max_y - min_y).max(1.0) + pad * 2.0,
                highlight,
                1.0,
            );
        }
    }
}

fn render_indicator_panes(
    indicator_panes: &[IndicatorPane],
    state: &ChartState,
    renderer: &mut dyn Renderer,
) {
    if indicator_panes.is_empty() || state.candles().len() < 3 {
        return;
    }

    let vp = &state.viewport;
    let width = vp.dimensions.width as f64;
    let height = vp.dimensions.height as f64;
    let indicator_total = pane_height_total(indicator_panes);
    let mut top = height * (1.0 - indicator_total).max(0.38);
    let bg = state.options.background_color.with_alpha(1.0);
    let text = state.options.text_color.with_alpha(0.82);

    // Price axis column width — oscillator content stops here, scale labels go here.
    let price_axis_w = 64.0;
    let content_w = width - price_axis_w;

    for pane in indicator_panes
        .iter()
        .filter(|pane| pane.placement() == IndicatorPlacement::Pane)
    {
        let pane_h = (height * pane.height_fraction).max(56.0);

        // Pane und Achsenspalte deckend, ohne Trennlinien: Abstand und Titel genügen.
        renderer.fill_rect(0.0, top, content_w, pane_h, bg);
        renderer.fill_rect(content_w, top, price_axis_w, pane_h, bg);
        // Der Titel steht über dem Pane; darunter beginnt die Zeichenfläche.
        let plot_top = top + PANE_TITLE_BAND_PX;
        let plot_h = (pane_h - PANE_TITLE_BAND_PX).max(24.0);

        // Alle Linien des Indikators, auf eine gemeinsame Skala gebracht.
        let visible_time = vp.time_range();
        let lines: Vec<Vec<(Seconds, f64)>> = pane
            .series
            .lines()
            .map(|line| {
                line.points
                    .iter()
                    .copied()
                    .filter(|(time, value)| {
                        *time >= visible_time.start
                            && *time <= visible_time.end
                            && value.is_finite()
                    })
                    .collect()
            })
            .collect();

        if lines.first().map(|l| l.len()).unwrap_or(0) >= 2 {
            let mut min_v = lines
                .iter()
                .flatten()
                .map(|(_, v)| *v)
                .fold(f64::INFINITY, f64::min);
            let mut max_v = lines
                .iter()
                .flatten()
                .map(|(_, v)| *v)
                .fold(f64::NEG_INFINITY, f64::max);
            if matches!(pane.indicator_id.as_str(), "rsi" | "stochastic") {
                min_v = 0.0;
                max_v = 100.0;
            } else if pane.indicator_id == "williams_r" {
                min_v = -100.0;
                max_v = 0.0;
            }
            let pad = ((max_v - min_v).abs() * 0.1).max(1.0);
            min_v -= pad;
            max_v += pad;
            let span = (max_v - min_v).max(1.0);
            let value_to_y = |value: f64| plot_top + plot_h - ((value - min_v) / span) * plot_h;

            for (index, line) in lines.iter().enumerate() {
                let points: Vec<(f64, f64)> = line
                    .iter()
                    .map(|(time, value)| (vp.time_to_x(*time), value_to_y(*value)))
                    .collect();
                if points.len() >= 2 {
                    renderer.draw_polyline(
                        &points,
                        line_color(index, is_dark(state)),
                        if index == 0 { 1.6 } else { 1.2 },
                    );
                }
            }

            // Gitter und Beschriftung auf glatten Marken des Panes.
            let pane_ticks = ticks::price_ticks(min_v, max_v, (plot_h / 40.0).clamp(2.0, 4.0));
            for value in &pane_ticks.values {
                let y = value_to_y(*value);
                renderer.draw_line(0.0, y, content_w, y, state.options.grid_color, 1.0);
                if y < plot_top + 3.0 || y > plot_top + plot_h - 3.0 {
                    continue;
                }
                renderer.draw_text(
                    &ticks::format_price(*value, pane_ticks.decimals),
                    width - 6.0,
                    y,
                    text,
                    LABEL_FONT_PX,
                    crate::rendering::TextAlign::Right,
                    crate::rendering::TextBaseline::Middle,
                );
            }
        }

        renderer.draw_text(
            &pane.indicator_id.to_uppercase(),
            8.0,
            top + 4.0,
            text,
            LABEL_FONT_PX,
            crate::rendering::TextAlign::Left,
            crate::rendering::TextBaseline::Top,
        );
        top += pane_h;
    }
}

fn render_axes(state: &ChartState, indicator_panes: &[IndicatorPane], renderer: &mut dyn Renderer) {
    let vp = &state.viewport;
    let width = vp.dimensions.width as f64;
    let height = vp.dimensions.height as f64;
    let main_height = main_chart_height(indicator_panes, height);
    let axis_color = state.options.text_color.with_alpha(0.7);
    // Die Achsenspalten sind deckend: der Datenbereich endet vor ihnen.
    let bg = state.options.background_color.with_alpha(1.0);

    let price_axis_w = PRICE_AXIS_WIDTH_PX;
    let time_axis_h = TIME_AXIS_HEIGHT_PX;
    renderer.fill_rect(width - price_axis_w, 0.0, price_axis_w, main_height, bg);
    renderer.fill_rect(0.0, height - time_axis_h, width, time_axis_h, bg);

    // Preismarken: glatte Schritte, gleich viele Nachkommastellen, und bei zu
    // dichter Lage nur jede n-te beschriftet — das Gitter bleibt an allen.
    let (min, max) = (vp.price.min, vp.price.max);
    if max > min {
        let (values, decimals): (Vec<f64>, usize) = if vp.log_scale {
            let step = ticks::nice_step(max - min, TARGET_INTERVALS);
            (vp.log_grid_prices(6), ticks::step_decimals(step))
        } else {
            let t = ticks::price_ticks(min, max, TARGET_INTERVALS);
            (t.values, t.decimals)
        };
        let price_y = |price: f64| {
            if vp.log_scale {
                let (lo, hi) = (min.max(1e-10).ln(), max.max(1e-10).ln());
                (hi - price.max(1e-10).ln()) / (hi - lo) * main_height
            } else {
                (max - price) / (max - min) * main_height
            }
        };
        let gap = match values.as_slice() {
            [a, b, ..] => (price_y(*a) - price_y(*b)).abs(),
            _ => 0.0,
        };
        let stride = ticks::label_stride(gap, MIN_LABEL_GAP_PX).max(1);
        for (index, price) in values.iter().enumerate() {
            let y = price_y(*price);
            // Eine Beschriftung, die am Rand halb abgeschnitten würde, entfällt.
            let edge = f64::from(LABEL_FONT_PX) / 2.0 + 1.0;
            if index % stride != 0 || y < edge || y > main_height - edge {
                continue;
            }
            renderer.draw_text(
                &ticks::format_price(*price, decimals),
                width - 6.0,
                y,
                axis_color,
                LABEL_FONT_PX,
                crate::rendering::TextAlign::Right,
                crate::rendering::TextBaseline::Middle,
            );
        }
    }

    // Ticks stehen an Bars mit Kalendersprung. In gleichen Zeitabständen ginge
    // es auf der Bar-Achse nicht mehr: zwischen zwei benachbarten Bars können
    // fünf Minuten oder ein Wochenende liegen.
    let plot_w = vp.plot_width();
    for tick in crate::core::time_axis::axis_ticks(
        vp.bar_index(),
        vp.bars(),
        plot_w,
        vp.timezone_offset_minutes,
        AXIS_LABEL_WIDTH_PX,
    ) {
        let x = vp.time_to_x(tick.time);
        if x < 0.0 || x > plot_w {
            continue;
        }
        renderer.draw_text(
            &tick.label,
            x,
            height - 4.0,
            axis_color,
            LABEL_FONT_PX,
            crate::rendering::TextAlign::Center,
            crate::rendering::TextBaseline::Bottom,
        );
    }
}
