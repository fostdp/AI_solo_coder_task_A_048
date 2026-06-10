use std::env;
use dotenv::dotenv;

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub influxdb_url: String,
    pub influxdb_token: String,
    pub influxdb_org: String,
    pub influxdb_bucket: String,
    pub server_host: String,
    pub server_port: u16,
    pub corrosion_rate_threshold: f64,
    pub chloride_threshold: f64,
    pub wecom_webhook_url: String,
    pub sms_access_key_id: String,
    pub sms_access_key_secret: String,
    pub sms_sign_name: String,
    pub sms_template_code: String,
    pub sms_phone_numbers: Vec<String>,
}

impl AppConfig {
    pub fn load() -> Self {
        dotenv().ok();

        AppConfig {
            influxdb_url: env::var("INFLUXDB_URL").unwrap_or_else(|_| "http://localhost:8086".to_string()),
            influxdb_token: env::var("INFLUXDB_TOKEN").unwrap_or_else(|_| "corrosion-monitor-token-2026".to_string()),
            influxdb_org: env::var("INFLUXDB_ORG").unwrap_or_else(|_| "archaeology".to_string()),
            influxdb_bucket: env::var("INFLUXDB_BUCKET").unwrap_or_else(|_| "corrosion_data".to_string()),
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            server_port: env::var("SERVER_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(8080),
            corrosion_rate_threshold: env::var("CORROSION_RATE_THRESHOLD").ok().and_then(|s| s.parse().ok()).unwrap_or(0.5),
            chloride_threshold: env::var("CHLORIDE_THRESHOLD").ok().and_then(|s| s.parse().ok()).unwrap_or(100.0),
            wecom_webhook_url: env::var("WECOM_WEBHOOK_URL").unwrap_or_default(),
            sms_access_key_id: env::var("SMS_ACCESS_KEY_ID").unwrap_or_default(),
            sms_access_key_secret: env::var("SMS_ACCESS_KEY_SECRET").unwrap_or_default(),
            sms_sign_name: env::var("SMS_SIGN_NAME").unwrap_or_else(|_| "考古监测".to_string()),
            sms_template_code: env::var("SMS_TEMPLATE_CODE").unwrap_or_default(),
            sms_phone_numbers: env::var("SMS_PHONE_NUMBERS").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
        }
    }
}
