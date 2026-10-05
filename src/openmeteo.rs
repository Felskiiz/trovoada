//! ICON, UKMO e CMA pelo Open-Meteo, mais o detalhe de um ponto.
use crate::modelo::Previsao;
use reqwest::Client;
use serde_json::Value;
use std::path::Path;

/// Uma Previsao por modelo (fonte "icon", "ukmo", "cma").
pub async fn grades(_http: &Client, _cache: &Path) -> Result<Vec<Previsao>, String> {
    Err("não implementado".into())
}

pub async fn ponto(_http: &Client, _lat: f32, _lon: f32) -> Result<Value, String> {
    Err("não implementado".into())
}
