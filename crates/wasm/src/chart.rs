//! WASM Entry Point - JavaScript API for the chart engine

use wasm_bindgen::prelude::*;

use web_sys::HtmlCanvasElement;

use kestrel_loom::core::{
    Candle, ChartState, EventHandler, FootprintCandle, KeyboardEvent, MouseButton, MouseEvent,
    Timeframe, TouchEvent,
};

use crate::canvas2d::Canvas2DRenderer;
use kestrel_loom::core::{render_chart, CompareSymbol, IndicatorPane, RenderExtras};

/// Main WASM Chart instance that can be controlled from JavaScript
#[wasm_bindgen]
pub struct WasmChart {
    state: ChartState,
    event_handler: EventHandler,
    renderer: Option<Canvas2DRenderer>,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    compare_symbols: Vec<CompareSymbol>,
    footprint_candles: Vec<FootprintCandle>,
    footprint_enabled: bool,
    drawing_drag_anchor: Option<(i64, f64)>,
    indicator_panes: Vec<IndicatorPane>,
}

#[wasm_bindgen]
impl WasmChart {
    /// Create a new chart instance
    #[wasm_bindgen(constructor)]
    pub fn new(width: u32, height: u32, timeframe: &str) -> Result<WasmChart, JsValue> {
        // Set panic hook for better error messages
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        let tf =
            Timeframe::from_str(timeframe).ok_or_else(|| JsValue::from_str("Invalid timeframe"))?;

        Ok(WasmChart {
            state: ChartState::new(width, height, tf),
            event_handler: EventHandler::new(),
            renderer: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            compare_symbols: Vec::new(),
            footprint_candles: Vec::new(),
            footprint_enabled: false,
            drawing_drag_anchor: None,
            indicator_panes: Vec::new(),
        })
    }

    /// Attach a canvas element for rendering
    #[wasm_bindgen(js_name = attachCanvas)]
    pub fn attach_canvas(&mut self, canvas: HtmlCanvasElement) -> Result<(), JsValue> {
        let mut renderer = Canvas2DRenderer::new(canvas)?;

        // Get pixel ratio from renderer
        let pixel_ratio = renderer.pixel_ratio();

        // Resize canvas to match chart dimensions
        let width = self.state.viewport.dimensions.width;
        let height = self.state.viewport.dimensions.height;
        renderer.resize(width, height)?;

        // Update viewport with pixel ratio
        self.state
            .viewport
            .set_dimensions(width, height, pixel_ratio);

        self.renderer = Some(renderer);
        Ok(())
    }

    /// Set candle data from JavaScript array
    #[wasm_bindgen(js_name = setCandles)]
    pub fn set_candles(&mut self, candles_json: &str) -> Result<(), JsValue> {
        let candles: Vec<Candle> = serde_json::from_str(candles_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse candles: {}", e)))?;

        self.state.set_candles(candles);
        Ok(())
    }

    /// Replace all candles (delegates to CandleBuffer::snapshot).
    ///
    /// Backward-compatible alias for `setCandles`; both methods accept the
    /// same JSON format.
    #[wasm_bindgen(js_name = setCandlesBatch)]
    pub fn set_candles_batch(&mut self, candles_json: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::CandleBuffer;

        let incoming: Vec<Candle> = serde_json::from_str(candles_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse candles: {}", e)))?;

        let mut buf = CandleBuffer::new();
        buf.snapshot(incoming);
        self.state.set_candles(buf.candles().to_vec());
        Ok(())
    }

    /// Merge new candles into the existing dataset (delegates to CandleBuffer::append).
    ///
    /// Existing candles are kept; incoming candles are sorted and deduped.
    /// Duplicate timestamps are overwritten by the incoming value.
    #[wasm_bindgen(js_name = appendCandles)]
    pub fn append_candles(&mut self, candles_json: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::CandleBuffer;

        let incoming: Vec<Candle> = serde_json::from_str(candles_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse candles: {}", e)))?;

        let mut buf = CandleBuffer::new();
        buf.snapshot(self.state.candles.clone());
        buf.append(&incoming);
        self.state.set_candles(buf.candles().to_vec());
        Ok(())
    }

    /// Upsert a single candle by timestamp (delegates to CandleBuffer::update_running).
    ///
    /// Accepts a JSON object representing one candle.  If a candle with the
    /// same `time` already exists it is replaced in-place; otherwise it is
    /// inserted at the correct sorted position.
    #[wasm_bindgen(js_name = updateRunningCandle)]
    pub fn update_running_candle(&mut self, candle_json: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::CandleBuffer;

        let candle: Candle = serde_json::from_str(candle_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse candle: {}", e)))?;

        let mut buf = CandleBuffer::new();
        buf.snapshot(self.state.candles.clone());
        buf.update_running(candle);
        self.state.set_candles(buf.candles().to_vec());
        Ok(())
    }

    /// Add a single candle
    #[wasm_bindgen(js_name = addCandle)]
    pub fn add_candle(&mut self, time: i64, o: f64, h: f64, l: f64, c: f64, v: f64) {
        let candle = Candle::new(time, o, h, l, c, v);
        self.state.add_candle(candle);
    }

    /// Get all candles as JSON (for indicator calculations)
    #[wasm_bindgen(js_name = getCandles)]
    pub fn get_candles(&self) -> String {
        serde_json::to_string(&self.state.candles).unwrap_or_else(|_| "[]".to_string())
    }

    /// Add or replace a comparison symbol rendered as normalized percent performance.
    #[wasm_bindgen(js_name = addCompareSymbol)]
    pub fn add_compare_symbol(
        &mut self,
        symbol: &str,
        candles_json: &str,
        color: &str,
    ) -> Result<(), JsValue> {
        let symbol = symbol.trim();
        if symbol.is_empty() {
            return Err(JsValue::from_str("Compare symbol must not be empty"));
        }

        let candles: Vec<Candle> = serde_json::from_str(candles_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse compare candles: {}", e)))?;
        if candles.is_empty() {
            return Err(JsValue::from_str("Compare candles must not be empty"));
        }

        let color = kestrel_loom::primitives::Color::from_hex(color)
            .map_err(|e| JsValue::from_str(&format!("Invalid compare color: {}", e)))?;

        if let Some(existing) = self
            .compare_symbols
            .iter_mut()
            .find(|entry| entry.symbol == symbol)
        {
            existing.candles = candles;
            existing.color = color;
        } else {
            if self.compare_symbols.len() >= 3 {
                return Err(JsValue::from_str(
                    "A maximum of 3 compare symbols is supported",
                ));
            }
            self.compare_symbols.push(CompareSymbol {
                symbol: symbol.to_string(),
                candles,
                color,
            });
        }

        self.state.mark_dirty();
        Ok(())
    }

    /// Remove a comparison symbol.
    #[wasm_bindgen(js_name = removeCompareSymbol)]
    pub fn remove_compare_symbol(&mut self, symbol: &str) {
        self.compare_symbols
            .retain(|entry| entry.symbol != symbol.trim());
        self.state.mark_dirty();
    }

    /// Return active comparison symbols as JSON.
    #[wasm_bindgen(js_name = getCompareSymbols)]
    pub fn get_compare_symbols(&self) -> String {
        let symbols: Vec<&str> = self
            .compare_symbols
            .iter()
            .map(|entry| entry.symbol.as_str())
            .collect();
        serde_json::to_string(&symbols).unwrap_or_else(|_| "[]".to_string())
    }

    /// Replace footprint candle data.
    #[wasm_bindgen(js_name = setFootprintData)]
    pub fn set_footprint_data(&mut self, candles_json: &str) -> Result<(), JsValue> {
        let candles: Vec<FootprintCandle> = serde_json::from_str(candles_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse footprint data: {}", e)))?;
        self.footprint_candles = candles;
        self.state.mark_dirty();
        Ok(())
    }

    /// Append or replace one footprint candle by timestamp.
    #[wasm_bindgen(js_name = addFootprintCandle)]
    pub fn add_footprint_candle(&mut self, candle_json: &str) -> Result<(), JsValue> {
        let candle: FootprintCandle = serde_json::from_str(candle_json)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse footprint candle: {}", e)))?;
        if let Some(existing) = self
            .footprint_candles
            .iter_mut()
            .find(|entry| entry.time == candle.time)
        {
            *existing = candle;
        } else {
            self.footprint_candles.push(candle);
            self.footprint_candles.sort_by_key(|entry| entry.time);
        }
        self.state.mark_dirty();
        Ok(())
    }

    /// Enable or disable footprint rendering.
    #[wasm_bindgen(js_name = setFootprintEnabled)]
    pub fn set_footprint_enabled(&mut self, enabled: bool) {
        self.footprint_enabled = enabled;
        if enabled {
            self.state.options.candle_style = kestrel_loom::primitives::CandleStyle::Footprint;
        } else if self.state.options.candle_style
            == kestrel_loom::primitives::CandleStyle::Footprint
        {
            self.state.options.candle_style = kestrel_loom::primitives::CandleStyle::Candlestick;
        }
        self.state.mark_dirty();
    }

    /// Resize the chart
    #[wasm_bindgen(js_name = resize)]
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), JsValue> {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(width, height)?;

            // Get pixel ratio from renderer and update viewport
            let pixel_ratio = renderer.pixel_ratio();
            self.state
                .viewport
                .set_dimensions(width, height, pixel_ratio);
            // Ohne dieses mark_dirty bleibt der Chart nach einer Größenänderung
            // leer: der Browser löscht den Canvas beim Setzen von width/height,
            // und der nächste render()-Aufruf kehrt sofort zurück, weil der
            // Zustand als sauber gilt. `ChartState::resize` (der Zweig ohne
            // Renderer) tat das schon, der angehängte Fall nicht.
            self.state.mark_dirty();
        } else {
            // No renderer, just update state dimensions
            self.state.resize(width, height);
        }

        Ok(())
    }

    /// Handle mouse down event
    #[wasm_bindgen(js_name = onMouseDown)]
    pub fn on_mouse_down(&mut self, x: f64, y: f64, button: u8) {
        let mouse_button = match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        };

        let event = MouseEvent::Down {
            x,
            y,
            button: mouse_button,
        };

        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle mouse up event
    #[wasm_bindgen(js_name = onMouseUp)]
    pub fn on_mouse_up(&mut self, x: f64, y: f64, button: u8) {
        let mouse_button = match button {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::Left,
        };

        let event = MouseEvent::Up {
            x,
            y,
            button: mouse_button,
        };

        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle mouse move event
    #[wasm_bindgen(js_name = onMouseMove)]
    pub fn on_mouse_move(&mut self, x: f64, y: f64) {
        let event = MouseEvent::Move { x, y };
        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle mouse wheel event
    #[wasm_bindgen(js_name = onMouseWheel)]
    pub fn on_mouse_wheel(&mut self, x: f64, y: f64, delta_y: f64) {
        let event = MouseEvent::Wheel { x, y, delta_y };
        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle mouse leave event
    #[wasm_bindgen(js_name = onMouseLeave)]
    pub fn on_mouse_leave(&mut self) {
        let event = MouseEvent::Leave;
        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle double click event
    #[wasm_bindgen(js_name = onDoubleClick)]
    pub fn on_double_click(&mut self, x: f64, y: f64) {
        let event = MouseEvent::DoubleClick { x, y };
        self.event_handler
            .handle_mouse_event(event, &mut self.state);
    }

    /// Handle touch start
    #[wasm_bindgen(js_name = onTouchStart)]
    pub fn on_touch_start(&mut self, x: f64, y: f64) {
        let event = TouchEvent::Start { x, y };
        self.event_handler
            .handle_touch_event(event, &mut self.state);
    }

    /// Handle touch move
    #[wasm_bindgen(js_name = onTouchMove)]
    pub fn on_touch_move(&mut self, x: f64, y: f64) {
        let event = TouchEvent::Move { x, y };
        self.event_handler
            .handle_touch_event(event, &mut self.state);
    }

    /// Handle touch end
    #[wasm_bindgen(js_name = onTouchEnd)]
    pub fn on_touch_end(&mut self, x: f64, y: f64) {
        let event = TouchEvent::End { x, y };
        self.event_handler
            .handle_touch_event(event, &mut self.state);
    }

    /// Handle keyboard event
    #[wasm_bindgen(js_name = onKeyDown)]
    pub fn on_key_down(&mut self, key: String) {
        let event = KeyboardEvent::KeyDown { key };
        self.event_handler
            .handle_keyboard_event(event, &mut self.state);
    }

    /// Fit viewport to data
    #[wasm_bindgen(js_name = fitToData)]
    pub fn fit_to_data(&mut self) {
        self.state.fit_to_data();
    }

    /// Set candle rendering style
    #[wasm_bindgen(js_name = setCandleStyle)]
    pub fn set_candle_style(&mut self, style: &str) -> Result<(), JsValue> {
        let candle_style = match style {
            "candlestick" => kestrel_loom::primitives::CandleStyle::Candlestick,
            "ohlc" => kestrel_loom::primitives::CandleStyle::OHLC,
            "hollow" => kestrel_loom::primitives::CandleStyle::Hollow,
            "line" => kestrel_loom::primitives::CandleStyle::Line,
            "area" => kestrel_loom::primitives::CandleStyle::Area,
            "footprint" => kestrel_loom::primitives::CandleStyle::Footprint,
            s if s.starts_with("renko") => {
                // Accept "renko" (default brick) or "renko:5.0"
                let brick_size = if let Some(rest) = s.strip_prefix("renko:") {
                    rest.parse::<f64>().unwrap_or(10.0)
                } else {
                    10.0
                };
                kestrel_loom::primitives::CandleStyle::Renko { brick_size }
            }
            _ => {
                return Err(JsValue::from_str(
                    "Invalid candle style. Use: candlestick, ohlc, hollow, line, area, footprint, or renko[:brick_size]",
                ))
            }
        };

        self.state.options.candle_style = candle_style;
        self.footprint_enabled = candle_style == kestrel_loom::primitives::CandleStyle::Footprint;
        self.state.mark_dirty();
        Ok(())
    }

    /// Toggle logarithmic price scale
    #[wasm_bindgen(js_name = setLogScale)]
    pub fn set_log_scale(&mut self, enabled: bool) {
        self.state.viewport.log_scale = enabled;
        self.state.mark_dirty();
    }

    /// Query current log scale mode
    #[wasm_bindgen(js_name = isLogScale)]
    pub fn is_log_scale(&self) -> bool {
        self.state.viewport.log_scale
    }

    /// Lock or unlock the price axis. When locked, fit_to_data() and reloading
    /// candles will leave the price range unchanged.
    #[wasm_bindgen(js_name = setPriceLocked)]
    pub fn set_price_locked(&mut self, locked: bool) {
        self.state.viewport.price_locked = locked;
        self.state.mark_dirty();
    }

    /// Query current price axis lock state
    #[wasm_bindgen(js_name = isPriceLocked)]
    pub fn is_price_locked(&self) -> bool {
        self.state.viewport.price_locked
    }

    /// Switch between dark and light theme
    #[wasm_bindgen(js_name = setTheme)]
    pub fn set_theme(&mut self, dark: bool) {
        use kestrel_loom::primitives::Color;
        let opts = &mut self.state.options;
        if dark {
            opts.background_color = Color::rgba(10, 14, 18, 1.0);
            opts.grid_color = Color::rgba(45, 54, 64, 0.3);
            opts.text_color = Color::rgba(231, 233, 234, 1.0);
            opts.crosshair_color = Color::rgba(139, 152, 165, 0.5);
        } else {
            opts.background_color = Color::rgba(255, 255, 255, 1.0);
            opts.grid_color = Color::rgba(180, 180, 180, 0.3);
            opts.text_color = Color::rgba(20, 20, 20, 1.0);
            opts.crosshair_color = Color::rgba(80, 80, 80, 0.5);
        }
        self.state.mark_dirty();
    }

    /// Get crosshair position as JSON
    #[wasm_bindgen(js_name = getCrosshairInfo)]
    pub fn get_crosshair_info(&self) -> JsValue {
        let crosshair = &self.state.crosshair;

        if !crosshair.visible {
            return JsValue::NULL;
        }

        let ohlcv = self.state.get_ohlcv_at_crosshair();

        let info = serde_json::json!({
            "time": crosshair.time,
            "price": crosshair.price,
            "x": crosshair.x,
            "y": crosshair.y,
            "ohlcv": ohlcv.map(|(o, h, l, c, v)| {
                serde_json::json!({
                    "open": o,
                    "high": h,
                    "low": l,
                    "close": c,
                    "volume": v,
                })
            })
        });

        JsValue::from_str(&info.to_string())
    }

    /// Get viewport info as JSON
    #[wasm_bindgen(js_name = getViewportInfo)]
    pub fn get_viewport_info(&self) -> JsValue {
        use kestrel_loom::core::ViewportScaleMode;
        let vp = &self.state.viewport;

        let scale_mode_str = match vp.scale_mode {
            ViewportScaleMode::Price => "price",
            ViewportScaleMode::Log => "log",
            ViewportScaleMode::Percent => "percent",
            ViewportScaleMode::Indexed => "indexed",
        };

        let info = serde_json::json!({
            "time": {
                "start": vp.time.start,
                "end": vp.time.end,
            },
            "price": {
                "min": vp.price.min,
                "max": vp.price.max,
            },
            "dimensions": {
                "width": vp.dimensions.width,
                "height": vp.dimensions.height,
                "pixelRatio": vp.dimensions.pixel_ratio,
            },
            "visibleBars": vp.visible_bars(),
            "barWidth": vp.bar_width(),
            "timezoneOffsetMinutes": vp.timezone_offset_minutes,
            "scaleMode": scale_mode_str,
            "scaleBasePrice": vp.scale_base_price,
        });

        JsValue::from_str(&info.to_string())
    }

    /// Get candle at position (with hit-testing)
    #[wasm_bindgen(js_name = getCandleAtPosition)]
    pub fn get_candle_at_position(&self, x: f64, y: f64) -> JsValue {
        match self.state.candle_at_position(x, y) {
            Some(candle) => {
                let info = serde_json::json!({
                    "time": candle.time,
                    "open": candle.o,
                    "high": candle.h,
                    "low": candle.l,
                    "close": candle.c,
                    "volume": candle.v,
                    "ohlc": candle.format_ohlc(),
                });
                JsValue::from_str(&info.to_string())
            }
            None => JsValue::NULL,
        }
    }

    /// Get OHLC formatted string at crosshair
    #[wasm_bindgen(js_name = getOHLCFormatted)]
    pub fn get_ohlc_formatted(&self) -> JsValue {
        match self.state.get_ohlc_formatted() {
            Some(formatted) => JsValue::from_str(&formatted),
            None => JsValue::NULL,
        }
    }

    /// Export chart state to JSON
    #[wasm_bindgen(js_name = exportState)]
    pub fn export_state(&self) -> Result<String, JsValue> {
        self.state
            .export()
            .map_err(|e| JsValue::from_str(&format!("Export error: {}", e)))
    }

    /// Import chart state from JSON
    #[wasm_bindgen(js_name = importState)]
    pub fn import_state(&mut self, json: &str) -> Result<(), JsValue> {
        self.state
            .import(json)
            .map_err(|e| JsValue::from_str(&format!("Import error: {}", e)))?;
        self.state.mark_dirty();
        Ok(())
    }

    /// Zeichnet einen Frame auf den angehängten Canvas.
    ///
    /// Der Ablauf selbst liegt in `kestrel_loom::core::render_chart`; hier wird nur
    /// der Canvas-Renderer beschafft und der Zustand hineingereicht.
    #[wasm_bindgen(js_name = render)]
    pub fn render(&mut self) -> Result<(), JsValue> {
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| JsValue::from_str("No renderer attached"))?;

        let extras = RenderExtras {
            indicator_panes: &self.indicator_panes,
            compare_symbols: &self.compare_symbols,
            footprint_candles: &self.footprint_candles,
        };

        render_chart(&mut self.state, &extras, renderer);
        Ok(())
    }

    /// Check if chart needs redraw
    #[wasm_bindgen(js_name = isDirty)]
    pub fn is_dirty(&self) -> bool {
        self.state.is_dirty()
    }

    // ========== Undo/Redo API ==========

    fn _snapshot_tools(&self) -> String {
        self.state.tool_manager.to_json().unwrap_or_default()
    }

    fn _push_undo(&mut self) {
        let snap = self._snapshot_tools();
        self.undo_stack.push(snap);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Undo the last drawing action. Returns true if there was something to undo.
    #[wasm_bindgen(js_name = undo)]
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo_stack.pop() else {
            return false;
        };
        let current = self._snapshot_tools();
        self.redo_stack.push(current);
        if self.redo_stack.len() > 50 {
            self.redo_stack.remove(0);
        }
        if let Ok(manager) = kestrel_loom::tools::ToolManager::from_json(&snap) {
            self.state.tool_manager = manager;
            self.state.mark_dirty();
        }
        true
    }

    /// Redo the last undone drawing action. Returns true if there was something to redo.
    #[wasm_bindgen(js_name = redo)]
    pub fn redo(&mut self) -> bool {
        let Some(snap) = self.redo_stack.pop() else {
            return false;
        };
        let current = self._snapshot_tools();
        self.undo_stack.push(current);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        if let Ok(manager) = kestrel_loom::tools::ToolManager::from_json(&snap) {
            self.state.tool_manager = manager;
            self.state.mark_dirty();
        }
        true
    }

    // ========== Drawing Tools API ==========

    /// Create a new trend line tool
    #[wasm_bindgen(js_name = createTrendLine)]
    pub fn create_trend_line(
        &mut self,
        id: &str,
        start_time: i64,
        start_price: f64,
        end_time: i64,
        end_price: f64,
    ) -> Result<(), JsValue> {
        use kestrel_loom::tools::{ToolNode, TrendLine};

        self._push_undo();

        let start_node = ToolNode {
            time: start_time,
            price: start_price,
        };

        let end_node = ToolNode {
            time: end_time,
            price: end_price,
        };

        let tool = TrendLine::with_nodes(id.to_string(), start_node, end_node);
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();

        Ok(())
    }

    /// Create a new horizontal line tool
    #[wasm_bindgen(js_name = createHorizontalLine)]
    pub fn create_horizontal_line(&mut self, id: &str, price: f64) -> Result<(), JsValue> {
        use kestrel_loom::tools::HorizontalLine;

        self._push_undo();
        let tool = HorizontalLine::with_price(id.to_string(), 0, price);
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();

        Ok(())
    }

    /// Create a new vertical line tool
    #[wasm_bindgen(js_name = createVerticalLine)]
    pub fn create_vertical_line(&mut self, id: &str, time: i64) -> Result<(), JsValue> {
        use kestrel_loom::tools::VerticalLine;

        self._push_undo();
        let tool = VerticalLine::with_time(id.to_string(), time, 0.0);
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();

        Ok(())
    }

    /// Remove a tool by ID
    #[wasm_bindgen(js_name = removeTool)]
    pub fn remove_tool(&mut self, id: &str) -> Result<(), JsValue> {
        self._push_undo();
        self.state.tool_manager.remove_tool(id);
        self.state.mark_dirty();
        Ok(())
    }

    /// Clear all tools
    #[wasm_bindgen(js_name = clearTools)]
    pub fn clear_tools(&mut self) -> Result<(), JsValue> {
        self._push_undo();
        self.state.tool_manager.clear();
        self.state.selected_tools.clear();
        self.state.mark_dirty();
        Ok(())
    }

    /// Create a rectangle drawing tool
    #[wasm_bindgen(js_name = createRectangle)]
    pub fn create_rectangle(
        &mut self,
        id: &str,
        t1: i64,
        p1: f64,
        t2: i64,
        p2: f64,
    ) -> Result<(), JsValue> {
        use kestrel_loom::tools::{Rectangle, ToolNode};
        self._push_undo();
        let tool = Rectangle::with_corners(
            id.to_string(),
            ToolNode {
                time: t1,
                price: p1,
            },
            ToolNode {
                time: t2,
                price: p2,
            },
        );
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();
        Ok(())
    }

    /// Create a Fibonacci retracement drawing tool
    #[wasm_bindgen(js_name = createFibonacci)]
    pub fn create_fibonacci(
        &mut self,
        id: &str,
        t1: i64,
        p1: f64,
        t2: i64,
        p2: f64,
    ) -> Result<(), JsValue> {
        use kestrel_loom::tools::{FibonacciRetracement, ToolNode};
        self._push_undo();
        let tool = FibonacciRetracement::with_points(
            id.to_string(),
            ToolNode {
                time: t1,
                price: p1,
            },
            ToolNode {
                time: t2,
                price: p2,
            },
        );
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();
        Ok(())
    }

    /// Create a text label drawing tool
    #[wasm_bindgen(js_name = createTextLabel)]
    pub fn create_text_label(
        &mut self,
        id: &str,
        time: i64,
        price: f64,
        text: &str,
    ) -> Result<(), JsValue> {
        use kestrel_loom::tools::{TextLabel, ToolNode};
        self._push_undo();
        let tool = TextLabel::new(id.to_string(), ToolNode { time, price }, text);
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();
        Ok(())
    }

    // ========== Bar Spacing API ==========

    /// Set additional bar spacing in CSS pixels (positive = wider bars, negative = narrower)
    #[wasm_bindgen(js_name = setBarSpacing)]
    pub fn set_bar_spacing(&mut self, extra_px: f64) {
        self.state.viewport.bar_spacing_extra = extra_px.clamp(-40.0, 200.0);
        self.state.mark_dirty();
    }

    /// Set bar width ratio (0.0 = auto, 0.1–0.95 = explicit fraction of slot)
    #[wasm_bindgen(js_name = setBarWidthRatio)]
    pub fn set_bar_width_ratio(&mut self, ratio: f64) {
        self.state.viewport.bar_width_ratio = ratio.clamp(0.0, 0.95);
        self.state.mark_dirty();
    }

    /// Get current bar spacing extra value
    #[wasm_bindgen(js_name = getBarSpacing)]
    pub fn get_bar_spacing(&self) -> f64 {
        self.state.viewport.bar_spacing_extra
    }

    /// Get all tools as JSON
    #[wasm_bindgen(js_name = getTools)]
    pub fn get_tools(&self) -> String {
        // Return array of tool JSON strings
        let tools: Vec<String> = self
            .state
            .tool_manager
            .tools()
            .iter()
            .filter_map(|tool| tool.to_json().ok())
            .collect();

        format!("[{}]", tools.join(","))
    }

    /// Select drawing at canvas position. If additive is true, toggles membership.
    #[wasm_bindgen(js_name = selectDrawingAt)]
    pub fn select_drawing_at(&mut self, x: f64, y: f64, additive: bool) -> bool {
        let hit_id = self
            .state
            .tool_manager
            .hit_test(x, y, &self.state.viewport)
            .map(|id| id.to_string());

        let Some(id) = hit_id else {
            if !additive {
                self.state.selected_tools.clear();
                self.state.mark_dirty();
            }
            return false;
        };

        if additive {
            if let Some(pos) = self
                .state
                .selected_tools
                .iter()
                .position(|selected| selected == &id)
            {
                self.state.selected_tools.remove(pos);
            } else {
                self.state.selected_tools.push(id);
            }
        } else {
            self.state.selected_tools = vec![id];
        }
        self.state.mark_dirty();
        true
    }

    /// Select all drawings whose nodes are fully inside a screen-space rectangle.
    #[wasm_bindgen(js_name = selectDrawingsInRect)]
    pub fn select_drawings_in_rect(
        &mut self,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        additive: bool,
    ) -> String {
        let left = x1.min(x2);
        let right = x1.max(x2);
        let top = y1.min(y2);
        let bottom = y1.max(y2);

        let mut selected = if additive {
            self.state.selected_tools.clone()
        } else {
            Vec::new()
        };

        for tool in self.state.tool_manager.tools() {
            if !tool.is_complete() || tool.nodes().is_empty() {
                continue;
            }
            let inside = tool.nodes().iter().all(|node| {
                let x = self.state.viewport.time_to_x(node.time);
                let y = self.state.viewport.price_to_y(node.price);
                x >= left && x <= right && y >= top && y <= bottom
            });
            if inside && !selected.iter().any(|id| id == tool.id()) {
                selected.push(tool.id().to_string());
            }
        }

        self.state.selected_tools = selected;
        self.state.mark_dirty();
        self.get_selected_tools()
    }

    /// Start bulk-dragging selected drawings from a canvas position.
    #[wasm_bindgen(js_name = startSelectedDrawingsDrag)]
    pub fn start_selected_tools_drag(&mut self, x: f64, y: f64) -> bool {
        if self.state.selected_tools.is_empty() {
            return false;
        }
        self._push_undo();
        self.drawing_drag_anchor = Some((
            self.state.viewport.x_to_time(x),
            self.state.viewport.y_to_price(y),
        ));
        true
    }

    /// Move all selected drawings to follow the current canvas position.
    #[wasm_bindgen(js_name = dragSelectedDrawingsTo)]
    pub fn drag_selected_tools_to(&mut self, x: f64, y: f64) {
        let Some((last_time, last_price)) = self.drawing_drag_anchor else {
            return;
        };
        let next_time = self.state.viewport.x_to_time(x);
        let next_price = self.state.viewport.y_to_price(y);
        let dt = next_time - last_time;
        let dp = next_price - last_price;
        self.state
            .tool_manager
            .move_many(&self.state.selected_tools, dt, dp);
        self.drawing_drag_anchor = Some((next_time, next_price));
        self.state.mark_dirty();
    }

    /// End a bulk drawing drag.
    #[wasm_bindgen(js_name = endSelectedDrawingsDrag)]
    pub fn end_selected_tools_drag(&mut self) {
        self.drawing_drag_anchor = None;
    }

    /// Delete all selected drawings as one undoable operation.
    #[wasm_bindgen(js_name = deleteSelectedDrawings)]
    pub fn delete_selected_tools(&mut self) -> usize {
        if self.state.selected_tools.is_empty() {
            return 0;
        }
        self._push_undo();
        let deleted = self
            .state
            .tool_manager
            .remove_many(&self.state.selected_tools);
        self.state.selected_tools.clear();
        self.state.mark_dirty();
        deleted
    }

    /// Return selected drawing IDs as JSON.
    #[wasm_bindgen(js_name = getSelectedDrawings)]
    pub fn get_selected_tools(&self) -> String {
        serde_json::to_string(&self.state.selected_tools).unwrap_or_else(|_| "[]".to_string())
    }

    /// Create or replace an indicator pane. Returns the pane ID.
    #[wasm_bindgen(js_name = addIndicatorPane)]
    pub fn add_indicator_pane(&mut self, indicator_id: &str, params_json: &str) -> String {
        let pane_id = format!("pane-{}", indicator_id.trim());
        if let Some(pane) = self
            .indicator_panes
            .iter_mut()
            .find(|pane| pane.indicator_id == indicator_id)
        {
            pane.params_json = params_json.to_string();
            self.state.mark_dirty();
            return pane.pane_id.clone();
        }

        self.indicator_panes.push(IndicatorPane {
            pane_id: pane_id.clone(),
            indicator_id: indicator_id.to_string(),
            params_json: params_json.to_string(),
            height_fraction: 0.28,
        });
        self.normalize_indicator_panes();
        self.state.mark_dirty();
        pane_id
    }

    /// Remove an indicator pane by pane ID.
    #[wasm_bindgen(js_name = removePane)]
    pub fn remove_pane(&mut self, pane_id: &str) -> bool {
        let before = self.indicator_panes.len();
        self.indicator_panes
            .retain(|pane| pane.pane_id != pane_id && pane.indicator_id != pane_id);
        let changed = before != self.indicator_panes.len();
        if changed {
            self.normalize_indicator_panes();
            self.state.mark_dirty();
        }
        changed
    }

    /// Set one pane height fraction, then normalize all panes.
    #[wasm_bindgen(js_name = setPaneHeightFraction)]
    pub fn set_pane_height_fraction(&mut self, pane_id: &str, fraction: f64) {
        if let Some(pane) = self
            .indicator_panes
            .iter_mut()
            .find(|pane| pane.pane_id == pane_id || pane.indicator_id == pane_id)
        {
            pane.height_fraction = fraction.clamp(0.14, 0.5);
            self.normalize_indicator_panes();
            self.state.mark_dirty();
        }
    }

    /// Return pane layout as JSON with main + indicator fractions.
    #[wasm_bindgen(js_name = getPaneLayout)]
    pub fn get_pane_layout(&self) -> String {
        serde_json::to_string(&self.pane_layout_json()).unwrap_or_else(|_| "[]".to_string())
    }

    fn normalize_indicator_panes(&mut self) {
        if self.indicator_panes.is_empty() {
            return;
        }
        let max_indicator_total = 0.62_f64;
        let total: f64 = self
            .indicator_panes
            .iter()
            .map(|pane| pane.height_fraction)
            .sum();
        if total <= max_indicator_total {
            return;
        }
        for pane in &mut self.indicator_panes {
            pane.height_fraction = pane.height_fraction / total * max_indicator_total;
        }
    }

    fn pane_layout_json(&self) -> Vec<serde_json::Value> {
        let indicator_total: f64 = self
            .indicator_panes
            .iter()
            .map(|pane| pane.height_fraction)
            .sum();
        let mut panes = vec![serde_json::json!({
            "id": "main",
            "indicatorId": "price",
            "heightFraction": (1.0 - indicator_total).max(0.38),
        })];
        panes.extend(self.indicator_panes.iter().map(|pane| {
            serde_json::json!({
                "id": pane.pane_id,
                "indicatorId": pane.indicator_id,
                "heightFraction": pane.height_fraction,
            })
        }));
        panes
    }

    // ========== Price Scale Interaction API ==========

    /// Start price scaling - user pressed mouse on price axis
    #[wasm_bindgen(js_name = startPriceScale)]
    pub fn start_price_scale(&mut self, y: f64) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        // Capture inverted Y and snapshot price range
        let start_y = self.state.viewport.start_price_scale(y);
        let initial_price_range = self.state.viewport.price;

        self.state.interaction = InteractionState::ScalingPrice {
            start_y,
            initial_price_range,
        };

        Ok(())
    }

    /// Apply price scaling - user is dragging on price axis
    #[wasm_bindgen(js_name = scalePriceTo)]
    pub fn scale_price_to(&mut self, y: f64) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        // Only apply if we're in scaling mode
        if let InteractionState::ScalingPrice {
            start_y,
            ref initial_price_range,
        } = self.state.interaction
        {
            self.state
                .viewport
                .apply_price_scale(start_y, y, initial_price_range);
            self.state.mark_dirty();
        }

        Ok(())
    }

    /// End price scaling - user released mouse
    #[wasm_bindgen(js_name = endPriceScale)]
    pub fn end_price_scale(&mut self) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        self.state.interaction = InteractionState::Idle;
        Ok(())
    }

    /// Reset price scale to auto-fit data (double-click)
    #[wasm_bindgen(js_name = resetPriceScale)]
    pub fn reset_price_scale(&mut self) -> Result<(), JsValue> {
        // Re-fit to current candle data
        if !self.state.candles.is_empty() {
            let visible_candles = self.state.visible_candles();
            if !visible_candles.is_empty() {
                let mut min_price = f64::MAX;
                let mut max_price = f64::MIN;

                for candle in visible_candles {
                    min_price = min_price.min(candle.l);
                    max_price = max_price.max(candle.h);
                }

                // Add 5% padding
                let range = max_price - min_price;
                let padding = range * 0.05;

                self.state.viewport.price.min = min_price - padding;
                self.state.viewport.price.max = max_price + padding;
                self.state.mark_dirty();
            }
        }

        Ok(())
    }

    /// Start time scaling - user clicked on time axis
    #[wasm_bindgen(js_name = startTimeScale)]
    pub fn start_time_scale(&mut self, x: f64) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        // Capture X and snapshot time range
        let start_x = self.state.viewport.start_time_scale(x);
        let initial_time_range = self.state.viewport.time;

        self.state.interaction = InteractionState::ScalingTime {
            start_x,
            initial_time_range,
        };

        Ok(())
    }

    /// Apply time scaling - user is dragging on time axis
    #[wasm_bindgen(js_name = scaleTimeTo)]
    pub fn scale_time_to(&mut self, x: f64) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        // Only apply if we're in scaling mode
        if let InteractionState::ScalingTime {
            start_x,
            ref initial_time_range,
        } = self.state.interaction
        {
            self.state
                .viewport
                .apply_time_scale(start_x, x, initial_time_range);
            self.state.mark_dirty();
        }

        Ok(())
    }

    /// End time scaling - user released mouse
    #[wasm_bindgen(js_name = endTimeScale)]
    pub fn end_time_scale(&mut self) -> Result<(), JsValue> {
        use kestrel_loom::core::InteractionState;

        self.state.interaction = InteractionState::Idle;
        Ok(())
    }

    /// Reset time scale to fit all data (double-click)
    #[wasm_bindgen(js_name = resetTimeScale)]
    pub fn reset_time_scale(&mut self) -> Result<(), JsValue> {
        // Re-fit to all candle data
        if !self.state.candles.is_empty() {
            let first_time = self.state.candles[0].time;
            let last_time = self.state.candles[self.state.candles.len() - 1].time;

            // Add 5% padding
            let range = (last_time - first_time) as f64;
            let padding = (range * 0.05) as i64;

            self.state.viewport.time.start = first_time - padding;
            self.state.viewport.time.end = last_time + padding;
            self.state.mark_dirty();
        }

        Ok(())
    }

    // ========== Ellipse Drawing Tool ==========

    /// Create an ellipse drawing tool (bounding box defined by two corner points)
    #[wasm_bindgen(js_name = createEllipse)]
    pub fn create_ellipse(
        &mut self,
        id: &str,
        t1: i64,
        p1: f64,
        t2: i64,
        p2: f64,
    ) -> Result<(), JsValue> {
        use kestrel_loom::tools::{Ellipse, ToolNode};
        self._push_undo();
        let tool = Ellipse::with_corners(
            id.to_string(),
            ToolNode {
                time: t1,
                price: p1,
            },
            ToolNode {
                time: t2,
                price: p2,
            },
        );
        self.state.tool_manager.add_tool(Box::new(tool));
        self.state.mark_dirty();
        Ok(())
    }

    // ========== Magnet / Snap Mode ==========

    /// Set the magnet/snap mode for drawing tool placement.
    /// `mode` must be one of: "off", "weak", "strong"
    #[wasm_bindgen(js_name = setMagnetMode)]
    pub fn set_magnet_mode(&mut self, mode: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::MagnetMode;
        self.state.magnet_mode = match mode {
            "off" => MagnetMode::Off,
            "weak" => MagnetMode::Weak,
            "strong" => MagnetMode::Strong,
            _ => {
                return Err(JsValue::from_str(
                    "Invalid magnet mode. Use: off, weak, strong",
                ))
            }
        };
        Ok(())
    }

    /// Get current magnet mode as string
    #[wasm_bindgen(js_name = getMagnetMode)]
    pub fn get_magnet_mode(&self) -> String {
        use kestrel_loom::core::MagnetMode;
        match self.state.magnet_mode {
            MagnetMode::Off => "off".to_string(),
            MagnetMode::Weak => "weak".to_string(),
            MagnetMode::Strong => "strong".to_string(),
        }
    }

    /// Snap a (time, price) coordinate to the nearest OHLC point when magnet is active.
    /// Returns JSON: `{ time: i64, price: f64, snapped: bool }`
    #[wasm_bindgen(js_name = snapToCandle)]
    pub fn snap_to_candle_wasm(&self, time: i64, price: f64) -> JsValue {
        use kestrel_loom::core::MagnetMode;
        let threshold_px = 20.0;
        let (snapped_time, snapped_price) = match self.state.magnet_mode {
            MagnetMode::Off => (time, price),
            MagnetMode::Weak | MagnetMode::Strong => self.state.tool_manager.snap_to_candle(
                time,
                price,
                &self.state.candles,
                threshold_px,
                &self.state.viewport,
            ),
        };
        let snapped = snapped_time != time || snapped_price != price;
        let info = serde_json::json!({
            "time": snapped_time,
            "price": snapped_price,
            "snapped": snapped,
        });
        JsValue::from_str(&info.to_string())
    }

    // ========== Session Markers ==========

    /// Set trading session configurations from JSON array.
    /// Each session: `{ name, open_utc: [h,m], close_utc: [h,m], color: [r,g,b,a], show_open, show_close }`
    /// Pass an empty array `[]` to clear sessions.
    /// Pass `"default"` as the string to load NYSE, London, Tokyo, Sydney presets.
    #[wasm_bindgen(js_name = setSessions)]
    pub fn set_sessions(&mut self, sessions_json: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::SessionConfig;
        if sessions_json == "default" {
            self.state.options.sessions = vec![
                SessionConfig::nyse(),
                SessionConfig::london(),
                SessionConfig::tokyo(),
                SessionConfig::sydney(),
            ];
        } else {
            let sessions: Vec<SessionConfig> = serde_json::from_str(sessions_json)
                .map_err(|e| JsValue::from_str(&format!("Invalid sessions JSON: {}", e)))?;
            self.state.options.sessions = sessions;
        }
        self.state.mark_dirty();
        Ok(())
    }

    /// Show or hide session marker lines
    #[wasm_bindgen(js_name = setShowSessions)]
    pub fn set_show_sessions(&mut self, show: bool) {
        self.state.options.show_sessions = show;
        self.state.mark_dirty();
    }

    // ========== Timezone ==========

    /// Set timezone offset in minutes from UTC.
    /// Examples: 60 = UTC+1, -300 = UTC-5, 540 = UTC+9, 0 = UTC
    #[wasm_bindgen(js_name = setTimezone)]
    pub fn set_timezone(&mut self, offset_minutes: i32) {
        self.state.viewport.timezone_offset_minutes = offset_minutes;
        self.state.mark_dirty();
    }

    /// Get current timezone offset in minutes
    #[wasm_bindgen(js_name = getTimezoneOffset)]
    pub fn get_timezone_offset(&self) -> i32 {
        self.state.viewport.timezone_offset_minutes
    }

    // ========== Price Scale Mode ==========

    /// Set price scale display mode.
    /// `mode` must be one of: "price", "log", "percent", "indexed"
    #[wasm_bindgen(js_name = setScaleMode)]
    pub fn set_scale_mode(&mut self, mode: &str) -> Result<(), JsValue> {
        use kestrel_loom::core::ViewportScaleMode;
        self.state.viewport.scale_mode = match mode {
            "price" => {
                self.state.viewport.log_scale = false;
                ViewportScaleMode::Price
            }
            "log" => {
                self.state.viewport.log_scale = true;
                ViewportScaleMode::Log
            }
            "percent" => {
                self.state.viewport.log_scale = false;
                // Set base price from first visible candle
                if let Some(first) = self.state.visible_candles().first() {
                    self.state.viewport.scale_base_price = first.c;
                }
                ViewportScaleMode::Percent
            }
            "indexed" => {
                self.state.viewport.log_scale = false;
                if let Some(first) = self.state.visible_candles().first() {
                    self.state.viewport.scale_base_price = first.c;
                }
                ViewportScaleMode::Indexed
            }
            _ => {
                return Err(JsValue::from_str(
                    "Invalid scale mode. Use: price, log, percent, indexed",
                ))
            }
        };
        self.state.mark_dirty();
        Ok(())
    }

    /// Get current scale mode as string
    #[wasm_bindgen(js_name = getScaleMode)]
    pub fn get_scale_mode(&self) -> String {
        use kestrel_loom::core::ViewportScaleMode;
        match self.state.viewport.scale_mode {
            ViewportScaleMode::Price => "price".to_string(),
            ViewportScaleMode::Log => "log".to_string(),
            ViewportScaleMode::Percent => "percent".to_string(),
            ViewportScaleMode::Indexed => "indexed".to_string(),
        }
    }

    // ========== Renko ==========

    /// Set candle style, including Renko with brick size.
    /// For renko: pass "renko" and provide brick_size > 0.
    #[wasm_bindgen(js_name = setRenkoBrickSize)]
    pub fn set_renko_brick_size(&mut self, brick_size: f64) -> Result<(), JsValue> {
        if brick_size <= 0.0 {
            return Err(JsValue::from_str("brick_size must be > 0"));
        }
        self.state.options.candle_style =
            kestrel_loom::primitives::CandleStyle::Renko { brick_size };
        self.state.mark_dirty();
        Ok(())
    }
}
