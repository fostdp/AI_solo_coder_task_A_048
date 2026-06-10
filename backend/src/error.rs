use thiserror::Error;
use actix_web::{HttpResponse, ResponseError};
use serde_json::json;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("InfluxDB 错误: {0}")]
    InfluxDB(String),
    #[error("HTTP 请求错误: {0}")]
    Http(String),
    #[error("数据解析错误: {0}")]
    Parse(String),
    #[error("未找到资源: {0}")]
    NotFound(String),
    #[error("内部错误: {0}")]
    Internal(String),
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let status_code = match self {
            AppError::NotFound(_) => 404,
            AppError::Parse(_) => 400,
            _ => 500,
        };

        HttpResponse::build(actix_web::http::StatusCode::from_u16(status_code).unwrap())
            .json(json!({
                "success": false,
                "data": null,
                "message": self.to_string()
            }))
    }
}

impl From<influxdb::Error> for AppError {
    fn from(e: influxdb::Error) -> Self {
        AppError::InfluxDB(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Parse(e.to_string())
    }
}

impl From<chrono::ParseError> for AppError {
    fn from(e: chrono::ParseError) -> Self {
        AppError::Parse(e.to_string())
    }
}
