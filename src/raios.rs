//! Raios do GLM (GOES-19), últimos 30 min.
use reqwest::Client;

#[derive(Default)]
pub struct Raios {}

impl Raios {
    pub async fn atualizar(&self, _http: &Client) -> Result<(), String> {
        Err("não implementado".into())
    }

    /// [lat, lon, idade em s].
    pub fn lista(&self) -> Vec<[f32; 3]> {
        Vec::new()
    }
}
