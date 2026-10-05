//! GFS (NOMADS) e ECMWF IFS (open data) em GRIB2, recortados para a grade.
use crate::modelo::Previsao;
use reqwest::Client;

pub async fn gfs(_http: &Client) -> Result<Previsao, String> {
    Err("não implementado".into())
}

pub async fn ecmwf(_http: &Client) -> Result<Previsao, String> {
    Err("não implementado".into())
}
