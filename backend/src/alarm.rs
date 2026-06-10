use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::influxdb_store::InfluxDBStore;
use crate::models::AlarmEvent;

#[derive(Clone)]
pub struct AlarmService {
    config: AppConfig,
    store: InfluxDBStore,
    http_client: Client,
    last_alarms: Arc<RwLock<HashMap<String, chrono::DateTime<Utc>>>>,
}

impl AlarmService {
    pub fn new(config: &AppConfig, store: &InfluxDBStore) -> Self {
        AlarmService {
            config: config.clone(),
            store: store.clone(),
            http_client: Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|_| Client::new()),
            last_alarms: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    async fn should_send_alarm(&self, key: &str, min_interval_minutes: i64) -> bool {
        let now = Utc::now();
        let alarms = self.last_alarms.read().await;
        if let Some(last_time) = alarms.get(key) {
            let diff = now.signed_duration_since(*last_time).num_minutes();
            diff >= min_interval_minutes
        } else {
            true
        }
    }

    async fn mark_alarm_sent(&self, key: &str) {
        let mut alarms = self.last_alarms.write().await;
        alarms.insert(key.to_string(), Utc::now());
    }

    pub async fn check_and_alert_corrosion(
        &self,
        probe_id: &str,
        zone: &str,
        material_type: &str,
        corrosion_rate: f64,
    ) -> Result<bool, AppError> {
        if corrosion_rate <= self.config.corrosion_rate_threshold {
            return Ok(false);
        }

        let key = format!("corrosion_{}", probe_id);
        if !self.should_send_alarm(&key, 30).await {
            return Ok(false);
        }

        let level = if corrosion_rate >= 1.0 { "critical" } else { "warning" };
        let material_name = if material_type == "iron" { "铁器" } else { "铜器" };
        let message = format!(
            "【腐蚀告警】{}探头 {} (区域{}) 腐蚀速率 {:.4} mm/年，超过阈值 {:.2} mm/年",
            material_name, probe_id, zone, corrosion_rate, self.config.corrosion_rate_threshold
        );

        let event = AlarmEvent {
            device_id: probe_id.to_string(),
            device_type: "corrosion_probe".to_string(),
            zone: zone.to_string(),
            alarm_type: "corrosion_rate".to_string(),
            level: level.to_string(),
            message: message.clone(),
            value: corrosion_rate,
            threshold: self.config.corrosion_rate_threshold,
            timestamp: Utc::now(),
        };

        self.store.write_alarm_event(&event).await?;
        self.send_wecom(&message).await.ok();
        self.send_sms(&message).await.ok();
        self.mark_alarm_sent(&key).await;

        tracing::warn!("{}", message);
        Ok(true)
    }

    pub async fn check_and_alert_chloride(
        &self,
        sensor_id: &str,
        zone: &str,
        chloride: f64,
    ) -> Result<bool, AppError> {
        if chloride <= self.config.chloride_threshold {
            return Ok(false);
        }

        let key = format!("chloride_{}", sensor_id);
        if !self.should_send_alarm(&key, 60).await {
            return Ok(false);
        }

        let level = if chloride >= 200.0 { "critical" } else { "warning" };
        let message = format!(
            "【环境告警】土壤传感器 {} (区域{}) 氯离子含量 {:.2} ppm，超过阈值 {:.0} ppm",
            sensor_id, zone, chloride, self.config.chloride_threshold
        );

        let event = AlarmEvent {
            device_id: sensor_id.to_string(),
            device_type: "soil_sensor".to_string(),
            zone: zone.to_string(),
            alarm_type: "chloride".to_string(),
            level: level.to_string(),
            message: message.clone(),
            value: chloride,
            threshold: self.config.chloride_threshold,
            timestamp: Utc::now(),
        };

        self.store.write_alarm_event(&event).await?;
        self.send_wecom(&message).await.ok();
        self.send_sms(&message).await.ok();
        self.mark_alarm_sent(&key).await;

        tracing::warn!("{}", message);
        Ok(true)
    }

    async fn send_wecom(&self, message: &str) -> Result<(), AppError> {
        if self.config.wecom_webhook_url.is_empty() {
            tracing::info!("企业微信未配置，跳过发送: {}", message);
            return Ok(());
        }

        let payload = json!({
            "msgtype": "markdown",
            "markdown": {
                "content": format!(
                    "## 🚨 古代战地医院遗址腐蚀监测告警\n\n{}\n\n**时间**: {}\n**系统**: 古代战地医院遗址腐蚀监测系统",
                    message,
                    Utc::now().format("%Y-%m-%d %H:%M:%S").to_string()
                )
            }
        });

        let resp = self.http_client
            .post(&self.config.wecom_webhook_url)
            .json(&payload)
            .send()
            .await;

        match resp {
            Ok(r) => {
                if r.status().is_success() {
                    tracing::info!("企业微信告警发送成功");
                } else {
                    tracing::error!("企业微信告警发送失败: {:?}", r.text().await);
                }
            }
            Err(e) => tracing::error!("企业微信请求失败: {}", e),
        }

        Ok(())
    }

    async fn send_sms(&self, message: &str) -> Result<(), AppError> {
        if self.config.sms_access_key_id.is_empty() || self.config.sms_phone_numbers.is_empty() {
            tracing::info!("短信未配置，跳过发送: {}", message);
            return Ok(());
        }

        let phones = self.config.sms_phone_numbers.join(",");
        tracing::info!(
            "模拟发送短信至 [{}]: {}",
            phones,
            message
        );

        Ok(())
    }
}
