use std::sync::Arc;
use influxdb::{Client, Query, ReadQuery, WriteQuery};
use chrono::{DateTime, Utc, Duration};
use crate::config::AppConfig;
use crate::error::AppError;
use crate::models::{SoilData, CorrosionData, CorrosionTrendPoint, HeatmapPoint, AlarmEvent, ProbeLocation};

#[derive(Clone)]
pub struct InfluxDBStore {
    client: Arc<Client>,
    config: AppConfig,
}

impl InfluxDBStore {
    pub fn new(config: &AppConfig) -> Self {
        let client = Client::new(&config.influxdb_url, &config.influxdb_org)
            .with_token(&config.influxdb_token);
        InfluxDBStore {
            client: Arc::new(client),
            config: config.clone(),
        }
    }

    pub async fn write_soil_data(&self, data: &SoilData) -> Result<(), AppError> {
        let ts = data.timestamp.unwrap_or_else(|| Utc::now());
        let query = format!(
            "soil_environment,sensor_id={},zone={},sensor_type={} temperature={:.4},humidity={:.4},ph={:.4},chloride={:.4} {}",
            data.sensor_id, data.zone, data.sensor_type,
            data.temperature, data.humidity, data.ph, data.chloride,
            ts.timestamp_nanos()
        );
        let write_query = WriteQuery::new(query);
        self.client.query(&self.config.influxdb_bucket, write_query).await?;
        Ok(())
    }

    pub async fn write_corrosion_data(&self, data: &CorrosionData) -> Result<(), AppError> {
        let ts = data.timestamp.unwrap_or_else(|| Utc::now());
        let query = format!(
            "metal_corrosion,probe_id={},zone={},material_type={} resistance={:.4},polarization_resistance={:.4},corrosion_rate={:.6} {}",
            data.probe_id, data.zone, data.material_type,
            data.resistance, data.polarization_resistance, data.corrosion_rate,
            ts.timestamp_nanos()
        );
        let write_query = WriteQuery::new(query);
        self.client.query(&self.config.influxdb_bucket, write_query).await?;
        Ok(())
    }

    pub async fn write_alarm_event(&self, event: &AlarmEvent) -> Result<(), AppError> {
        let query = format!(
            "alarm_events,device_id={},alarm_type={},level={} message=\"{}\",value={:.4},threshold={:.4} {}",
            event.device_id, event.alarm_type, event.level,
            event.message.replace('"', "\\\""),
            event.value, event.threshold,
            event.timestamp.timestamp_nanos()
        );
        let write_query = WriteQuery::new(query);
        self.client.query(&self.config.influxdb_bucket, write_query).await?;
        Ok(())
    }

    pub async fn query_corrosion_trend(
        &self,
        probe_id: &str,
        hours: i64,
    ) -> Result<Vec<CorrosionTrendPoint>, AppError> {
        let start_time = Utc::now() - Duration::hours(hours);
        let flux_query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: {})
                |> filter(fn: (r) => r["_measurement"] == "metal_corrosion")
                |> filter(fn: (r) => r["probe_id"] == "{}")
                |> filter(fn: (r) => r["_field"] == "corrosion_rate")
                |> aggregateWindow(every: 1h, fn: mean, createEmpty: false)
                |> yield(name: "mean")
            "#,
            self.config.influxdb_bucket,
            start_time.to_rfc3339(),
            probe_id
        );

        let read_query = ReadQuery::new(flux_query);
        let result = self.client.json_query(read_query).await?;
        let mut points = Vec::new();

        if let Some(results) = result.get("_results") {
            if let Some(tables) = results.as_array() {
                for table in tables {
                    if let Some(records) = table.get("records").and_then(|r| r.as_array()) {
                        for record in records {
                            if let (Some(ts), Some(val)) = (
                                record.get("_time").and_then(|t| t.as_str()),
                                record.get("_value").and_then(|v| v.as_f64()),
                            ) {
                                if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                                    points.push(CorrosionTrendPoint {
                                        timestamp: dt.with_timezone(&Utc),
                                        corrosion_rate: val,
                                        avg_temperature: None,
                                        avg_humidity: None,
                                        avg_chloride: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(points)
    }

    pub async fn query_heatmap_data(
        &self,
        locations: &[ProbeLocation],
        hours: i64,
    ) -> Result<Vec<HeatmapPoint>, AppError> {
        let start_time = Utc::now() - Duration::hours(hours);
        let mut points = Vec::new();

        for loc in locations.iter().filter(|l| l.device_type == "corrosion_probe") {
            let flux_query = format!(
                r#"
                from(bucket: "{}")
                    |> range(start: {})
                    |> filter(fn: (r) => r["_measurement"] == "metal_corrosion")
                    |> filter(fn: (r) => r["probe_id"] == "{}")
                    |> filter(fn: (r) => r["_field"] == "corrosion_rate")
                    |> mean()
                "#,
                self.config.influxdb_bucket,
                start_time.to_rfc3339(),
                loc.id
            );

            let read_query = ReadQuery::new(flux_query);
            match self.client.json_query(read_query).await {
                Ok(result) => {
                    if let Some(results) = result.get("_results") {
                        if let Some(tables) = results.as_array() {
                            for table in tables {
                                if let Some(records) = table.get("records").and_then(|r| r.as_array()) {
                                    for record in records {
                                        if let Some(val) = record.get("_value").and_then(|v| v.as_f64()) {
                                            points.push(HeatmapPoint {
                                                lat: loc.lat,
                                                lng: loc.lng,
                                                intensity: (val / 0.8).min(1.0).max(0.0),
                                                probe_id: loc.id.clone(),
                                                zone: loc.zone.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(points)
    }

    pub async fn query_latest_corrosion_rate(
        &self,
        probe_id: &str,
    ) -> Result<Option<f64>, AppError> {
        let flux_query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: -24h)
                |> filter(fn: (r) => r["_measurement"] == "metal_corrosion")
                |> filter(fn: (r) => r["probe_id"] == "{}")
                |> filter(fn: (r) => r["_field"] == "corrosion_rate")
                |> last()
            "#,
            self.config.influxdb_bucket,
            probe_id
        );

        let read_query = ReadQuery::new(flux_query);
        let result = self.client.json_query(read_query).await?;

        if let Some(results) = result.get("_results") {
            if let Some(tables) = results.as_array() {
                for table in tables {
                    if let Some(records) = table.get("records").and_then(|r| r.as_array()) {
                        for record in records {
                            if let Some(val) = record.get("_value").and_then(|v| v.as_f64()) {
                                return Ok(Some(val));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    pub async fn query_zone_avg_environment(
        &self,
        zone: &str,
        hours: i64,
    ) -> Result<(f64, f64, f64, f64), AppError> {
        let start_time = Utc::now() - Duration::hours(hours);
        let flux_query = format!(
            r#"
            from(bucket: "{}")
                |> range(start: {})
                |> filter(fn: (r) => r["_measurement"] == "soil_environment")
                |> filter(fn: (r) => r["zone"] == "{}")
                |> mean()
            "#,
            self.config.influxdb_bucket,
            start_time.to_rfc3339(),
            zone
        );

        let read_query = ReadQuery::new(flux_query);
        let result = self.client.json_query(read_query).await?;

        let mut temp = 15.0;
        let mut humidity = 50.0;
        let mut ph = 7.0;
        let mut chloride = 30.0;

        if let Some(results) = result.get("_results") {
            if let Some(tables) = results.as_array() {
                for table in tables {
                    if let Some(records) = table.get("records").and_then(|r| r.as_array()) {
                        for record in records {
                            if let Some(field) = record.get("_field").and_then(|f| f.as_str()) {
                                if let Some(val) = record.get("_value").and_then(|v| v.as_f64()) {
                                    match field {
                                        "temperature" => temp = val,
                                        "humidity" => humidity = val,
                                        "ph" => ph = val,
                                        "chloride" => chloride = val,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok((temp, humidity, ph, chloride))
    }
}
