//! Observação: radar RainViewer, satélite GIBS (proxy de tiles com cache) e alertas INMET.
use axum::http::StatusCode;
use reqwest::Client;
use serde_json::Value;
use std::path::Path;

pub async fn radar(_http: &Client) -> Result<Value, String> {
    Err("não implementado".into())
}

pub async fn satelite(_http: &Client) -> Result<Value, String> {
    Err("não implementado".into())
}

pub async fn alertas(_http: &Client) -> Result<Value, String> {
    Err("não implementado".into())
}

/// PNG do tile; camada é "radar", "ir" ou "imerg".
pub async fn tile(
    _http: &Client,
    _cache: &Path,
    _camada: &str,
    _t: &str,
    _z: u32,
    _x: u32,
    _y: u32,
) -> Result<Vec<u8>, StatusCode> {
    Err(StatusCode::NOT_IMPLEMENTED)
}
