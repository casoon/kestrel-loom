use crate::core::{ChartState, Viewport};
use crate::Candle;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ViewportExport {
    pub time_start: i64,
    pub time_end: i64,
    pub price_min: f64,
    pub price_max: f64,
}

impl Viewport {
    /// Exportiert wird in **Zeit**, nicht in Bar-Indizes.
    ///
    /// Bar-Indizes sind nicht stabil: ein Export, der auf „Bar 412" zeigt,
    /// zeigt nach der nächsten Datenlieferung woanders hin. Zeitstempel tun
    /// das nicht — und alte Exporte bleiben lesbar.
    pub fn export(&self) -> ViewportExport {
        let time = self.time_range();
        ViewportExport {
            time_start: time.start,
            time_end: time.end,
            price_min: self.price.min,
            price_max: self.price.max,
        }
    }

    pub fn import(&mut self, export: ViewportExport) {
        self.set_time_range(crate::core::TimeRange {
            start: export.time_start,
            end: export.time_end,
        });
        self.price.min = export.price_min;
        self.price.max = export.price_max;
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChartStateExport {
    pub version: String,
    pub timestamp: i64,
    pub timeframe: String,
    pub candles: Vec<Candle>,
    pub viewport: ViewportExport,
}

impl ChartState {
    pub fn export(&self) -> Result<String, String> {
        let export = ChartStateExport {
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            timeframe: format!("{:?}", self.timeframe),
            candles: self.candles().to_vec(),
            viewport: self.viewport.export(),
        };

        serde_json::to_string(&export).map_err(|e| format!("Serialization error: {}", e))
    }

    pub fn import(&mut self, json: &str) -> Result<(), String> {
        let export: ChartStateExport =
            serde_json::from_str(json).map_err(|e| format!("Deserialization error: {}", e))?;

        // Versionsabweichung wird toleriert, nicht gemeldet: der Kern kennt keine
        // Ausgabekanäle. Wer warnen will, prüft `export.version` vor dem Import.

        // Restore candles
        self.set_candles(export.candles);

        // Restore viewport
        self.viewport.import(export.viewport);

        Ok(())
    }
}
