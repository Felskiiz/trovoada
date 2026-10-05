//! Grade de consenso e tipos compartilhados entre as fontes.
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;

/// Grade 0,5° sobre o Brasil: lat 6 a -34, lon -74 a -34. Índice i = y*NX + x.
pub const NX: usize = 81;
pub const NY: usize = 81;
pub const N: usize = NX * NY;
pub const LAT0: f32 = 6.0;
pub const LON0: f32 = -74.0;
pub const PASSO: f32 = 0.5;
/// Intervalo entre quadros (s) e horizonte (s).
pub const QUADRO: i64 = 3 * 3600;
pub const HORIZONTE: i64 = 72 * 3600;

pub fn lat(y: usize) -> f32 {
    LAT0 - y as f32 * PASSO
}

pub fn lon(x: usize) -> f32 {
    LON0 + x as f32 * PASSO
}

/// Célula mais próxima de (lat, lon), se estiver dentro da grade.
pub fn celula(la: f32, lo: f32) -> Option<usize> {
    let y = ((LAT0 - la) / PASSO).round();
    let x = ((lo - LON0) / PASSO).round();
    (y >= 0.0 && x >= 0.0 && (y as usize) < NY && (x as usize) < NX)
        .then(|| y as usize * NX + x as usize)
}

/// Campos de um modelo num instante válido. Cada Vec tem N valores; NaN = sem dado.
#[derive(Clone, Debug)]
pub struct Campos {
    /// mm/h, média das 3 h que terminam no instante.
    pub chuva: Vec<f32>,
    /// J/kg.
    pub cape: Vec<f32>,
    /// km/h.
    pub rajada: Vec<f32>,
    /// Vento a 10 m, m/s.
    pub u: Vec<f32>,
    pub v: Vec<f32>,
    /// °C.
    pub t2m: Vec<f32>,
    pub t850: Vec<f32>,
    /// hPa.
    pub pmsl: Vec<f32>,
}

impl Campos {
    pub fn vazio() -> Self {
        let n = || vec![f32::NAN; N];
        Campos {
            chuva: n(),
            cape: n(),
            rajada: n(),
            u: n(),
            v: n(),
            t2m: n(),
            t850: n(),
            pmsl: n(),
        }
    }
}

/// Uma rodada de um modelo, com os passos indexados pelo instante válido (unix s, múltiplo de QUADRO).
#[derive(Clone, Debug)]
pub struct Previsao {
    pub fonte: &'static str,
    /// Início da rodada, unix s.
    pub rodada: i64,
    pub passos: BTreeMap<i64, Campos>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub rodada: Option<i64>,
    pub atualizado: Option<i64>,
    pub erro: Option<String>,
}

/// Campo combinado num instante. Vecs de N valores.
#[derive(Clone, Debug, Serialize)]
pub struct Consenso {
    pub tempo: i64,
    /// mm/h: mediana das fontes que votaram chuva onde a maioria votou; 0 onde não.
    #[serde(serialize_with = "r1")]
    pub chuva: Vec<f32>,
    /// Fontes que votaram chuva e fontes com dado, por célula.
    pub votos_chuva: Vec<u8>,
    pub fontes: Vec<u8>,
    #[serde(serialize_with = "r1")]
    pub cape: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub rajada: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub u: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub v: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub t2m: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub t850: Vec<f32>,
    /// T850 menos a média da latitude: <0 massa fria, >0 massa quente.
    #[serde(serialize_with = "r1")]
    pub anom850: Vec<f32>,
    #[serde(serialize_with = "r1")]
    pub pmsl: Vec<f32>,
    /// -1 frente fria, 0 nada, 1 frente quente.
    pub frente: Vec<i8>,
    /// 0 a 3.
    pub risco: Vec<u8>,
}

/// Uma casa decimal; NaN vira null.
fn r1<S: Serializer>(v: &[f32], s: S) -> Result<S::Ok, S::Error> {
    s.collect_seq(
        v.iter()
            .map(|x| x.is_finite().then(|| (x * 10.0).round() / 10.0)),
    )
}
