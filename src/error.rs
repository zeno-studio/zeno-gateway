//! Custom error types for the application using thiserror and anyhow.

use thiserror::Error;
use std::net::AddrParseError;

/// Custom error types for the application
#[derive(Error, Debug)]
pub enum AppError {
    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Tonic transport error
    #[error("Transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error
    #[error("HTTP request error: {0}")]
    HttpRequest(#[from] reqwest::Error),

    /// Database error
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Parse integer error
    #[error("Parse integer error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    /// Address parse error
    #[error("Address parse error: {0}")]
    AddrParse(#[from] AddrParseError),

    /// Tonic status error
    #[error("gRPC status error: {0}")]
    Status(#[from] tonic::Status),

    /// Rustls error
    #[error("TLS error: {0}")]
    Tls(#[from] rustls::Error),

    /// Custom error with message
    #[error("Application error: {0}")]
    Custom(String),
}

// Type alias for convenience
pub type Result<T> = std::result::Result<T, AppError>;