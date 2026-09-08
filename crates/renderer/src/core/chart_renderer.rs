//! Der Renderloop: aus Chart-Zustand werden Zeichenbefehle.
//!
//! Bis 2026-09-08 lag dieser Ablauf in der WASM-Fassade und war damit an
//! `wasm-bindgen` gebunden — nicht ohne Browser ausführbar und nicht testbar.
//! Hier schreibt er in einen `&mut dyn Renderer`: im Browser der Canvas-Renderer,
//! im Test der `BatchRenderer`, der den Befehlsstrom mitschreibt.

use std::collections::HashMap;

use crate::core::indicators::IndicatorSeries;
use crate::core::{Candle, ChartState, FootprintCandle};
use crate::primitives::Color;
use crate::rendering::Renderer;

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

        // Draw horizontal grid lines — log-spaced when log scale is active
        let num_lines = 10;
        let grid_prices: Vec<f64> = if vp.log_scale {
            vp.log_grid_prices(num_lines)
        } else {
            let price_step = (vp.price.max - vp.price.min) / num_lines as f64;
            (0..=num_lines)
                .map(|i| vp.price.min + i as f64 * price_step)
                .collect()
        };

        for price in &grid_prices {
            let y = vp.price_to_y(*price);
            renderer.draw_line(0.0, y, vp.dimensions.width as f64, y, grid_color, 1.0);
        }

        // Draw vertical grid lines (time levels)
        let bar_width = vp.bar_width();
        let step = (vp.dimensions.width as f64 / 10.0).max(bar_width * 5.0);
        let mut x = 0.0;
        while x < vp.dimensions.width as f64 {
            renderer.draw_line(x, 0.0, x, vp.dimensions.height as f64, grid_color, 1.0);
            x += step;
        }
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

        // For Renko: transform candles first, then render as candlestick bricks
        let renko_bricks: Vec<Candle>;
        let owned_visible: Vec<Candle>;
        let render_candles: &[Candle] =
            if let crate::primitives::CandleStyle::Renko { brick_size } = candle_style {
                renko_bricks = crate::core::renko::compute_renko(&state.candles, brick_size);
                &renko_bricks
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
                for candle in render_candles {
                    if candle.time < vp.time.start || candle.time > vp.time.end {
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
                        _ => unreachable!(),
                    }
                }
            }
        }
    }

    // Draw session markers (rendered behind candles — drawn before tools)
    if state.options.show_sessions && !state.options.sessions.is_empty() {
        let vp = &state.viewport;
        let tf_secs = state.timeframe.duration_secs();
        // Only show session markers for timeframes <= 1h (3600s)
        if tf_secs <= 3600 {
            let chart_width = vp.dimensions.width as f64;
            let chart_height = vp.dimensions.height as f64;
            let time_start = vp.time.start;
            let time_end = vp.time.end;

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
                            let x = vp.time_to_x(open_ts);
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
                            let x = vp.time_to_x(close_ts);
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

    // Restore full canvas height for layout (indicator panes, axes, crosshair)
    state.viewport.dimensions.height = full_height;

    // Draw comparison symbols as percent-performance overlays with their
    // own right-side scale.
    renderer.set_clip(
        0.0,
        0.0,
        state.viewport.dimensions.width as f64,
        main_height,
    );
    render_compare_symbols(compare_symbols, state, renderer, main_height);
    renderer.clear_clip();
    render_indicator_panes(indicator_panes, state, renderer);

    // Draw drawing tools
    {
        let vp = &state.viewport;
        let tools = state.tool_manager.tools();

        renderer.set_clip(0.0, 0.0, vp.dimensions.width as f64, main_height);
        for tool in tools {
            tool.render(renderer, vp);
        }
        renderer.clear_clip();
    }

    renderer.set_clip(
        0.0,
        0.0,
        state.viewport.dimensions.width as f64,
        main_height,
    );
    render_selected_tool_highlights(state, renderer);
    renderer.clear_clip();

    // Draw crosshair with pixel-perfect rendering
    if state.options.show_crosshair && state.crosshair.visible {
        let crosshair = &state.crosshair;
        let color = state.options.crosshair_color;
        let vp = &state.viewport;

        // Set line cap to butt for crisp crosshair lines (TradingView style)

        // Vertical line - use optimized method
        renderer.draw_vertical_line(crosshair.x, 0.0, vp.dimensions.height as f64, &color, 1.0);

        // Horizontal line - use optimized method
        renderer.draw_horizontal_line(crosshair.y, 0.0, vp.dimensions.width as f64, &color, 1.0);
    }

    render_axes(state, indicator_panes, renderer);

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
    let time_start = vp.time.start;
    let time_end = vp.time.end;

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

        let values: Vec<(i64, f64)> = visible
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

fn main_chart_height(indicator_panes: &[IndicatorPane], height: f64) -> f64 {
    if indicator_panes.is_empty() {
        return height;
    }
    let indicator_total: f64 = indicator_panes
        .iter()
        .map(|pane| pane.height_fraction)
        .sum();
    height * (1.0 - indicator_total).max(0.38)
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
    let visible: Vec<&FootprintCandle> = footprint_candles
        .iter()
        .filter(|candle| candle.time >= vp.time.start && candle.time <= vp.time.end)
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
    if indicator_panes.is_empty() || state.candles.len() < 3 {
        return;
    }

    let vp = &state.viewport;
    let width = vp.dimensions.width as f64;
    let height = vp.dimensions.height as f64;
    let indicator_total: f64 = indicator_panes
        .iter()
        .map(|pane| pane.height_fraction)
        .sum();
    let mut top = height * (1.0 - indicator_total).max(0.38);
    let bg = state.options.background_color.with_alpha(1.0);
    let border = state.options.grid_color.with_alpha(0.8);
    let text = state.options.text_color.with_alpha(0.82);

    // Price axis column width — oscillator content stops here, scale labels go here.
    let price_axis_w = 64.0;
    let content_w = width - price_axis_w;

    for pane in indicator_panes {
        let pane_h = (height * pane.height_fraction).max(56.0);

        // Pane background: only the content area (not the price-axis column)
        renderer.fill_rect(0.0, top, content_w, pane_h, bg);
        // Price-axis column background for this pane
        renderer.fill_rect(
            content_w,
            top,
            price_axis_w,
            pane_h,
            state.options.background_color.with_alpha(0.82),
        );
        // Top border across full width
        renderer.draw_line(0.0, top, width, top, border, 1.0);
        // Separator between content and scale column
        renderer.draw_line(content_w, top, content_w, top + pane_h, border, 1.0);

        let series = pane.series.values();
        let visible: Vec<(i64, f64)> = series
            .iter()
            .copied()
            .filter(|(time, value)| {
                *time >= vp.time.start && *time <= vp.time.end && value.is_finite()
            })
            .collect();

        if visible.len() >= 2 {
            let mut min_v = visible
                .iter()
                .map(|(_, v)| *v)
                .fold(f64::INFINITY, f64::min);
            let mut max_v = visible
                .iter()
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
            let value_to_y = |value: f64| top + pane_h - ((value - min_v) / span) * pane_h;
            let points: Vec<(f64, f64)> = visible
                .iter()
                .map(|(time, value)| (vp.time_to_x(*time), value_to_y(*value)))
                .collect();
            renderer.draw_polyline(
                &points,
                crate::primitives::Color::rgba(88, 166, 255, 0.95),
                1.6,
            );

            // Scale labels in the price-axis column (right 64px)
            for i in 0..=2 {
                let value = min_v + span * i as f64 / 2.0;
                let y = value_to_y(value);
                renderer.draw_line(0.0, y, content_w, y, state.options.grid_color, 1.0);
                renderer.draw_text(
                    &format!("{:.1}", value),
                    width - 4.0,
                    y,
                    text,
                    9.0,
                    crate::rendering::TextAlign::Right,
                    crate::rendering::TextBaseline::Middle,
                );
            }
        }

        renderer.draw_text(
            &pane.indicator_id.to_uppercase(),
            8.0,
            top + 6.0,
            text,
            10.0,
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
    let axis_color = state.options.text_color.with_alpha(0.78);
    let bg = state.options.background_color.with_alpha(0.82);
    let grid = state.options.grid_color.with_alpha(0.85);

    let price_axis_w = 64.0;
    let time_axis_h = 20.0;
    renderer.fill_rect(width - price_axis_w, 0.0, price_axis_w, main_height, bg);
    renderer.fill_rect(0.0, height - time_axis_h, width, time_axis_h, bg);
    renderer.draw_line(
        width - price_axis_w,
        0.0,
        width - price_axis_w,
        main_height,
        grid,
        1.0,
    );
    renderer.draw_line(
        0.0,
        height - time_axis_h,
        width,
        height - time_axis_h,
        grid,
        1.0,
    );

    let price_lines = 6;
    let price_step = (vp.price.max - vp.price.min) / price_lines as f64;
    for i in 0..=price_lines {
        let price = vp.price.min + price_step * i as f64;
        let y = (vp.price.max - price) / (vp.price.max - vp.price.min) * main_height;
        if y < 0.0 || y > main_height {
            continue;
        }
        renderer.draw_text(
            &format!("{:.2}", price),
            width - 4.0,
            y,
            axis_color,
            10.0,
            crate::rendering::TextAlign::Right,
            crate::rendering::TextBaseline::Middle,
        );
    }

    let time_lines = 5;
    let time_span = (vp.time.end - vp.time.start).max(1);
    for i in 0..=time_lines {
        let time = vp.time.start + time_span * i as i64 / time_lines as i64;
        let x = vp.time_to_x(time);
        if x < 0.0 || x > width - price_axis_w {
            continue;
        }
        renderer.draw_text(
            &format_axis_time(time),
            x,
            height - 4.0,
            axis_color,
            10.0,
            crate::rendering::TextAlign::Center,
            crate::rendering::TextBaseline::Bottom,
        );
    }
}

fn format_axis_time(time: i64) -> String {
    chrono::DateTime::from_timestamp(time, 0)
        .map(|dt| dt.format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|| time.to_string())
}
