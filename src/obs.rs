//! Observação: instantes do satélite GIBS e alertas INMET. Radar e tiles o navegador busca direto.
use reqwest::Client;
use serde_json::{Value, json};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GIBS: &str = "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best";
const INMET: &str = "https://apiprevmet3.inmet.gov.br/avisos/ativos";

/// Valor memorizado com validade.
type Memo<T> = Mutex<Option<(Instant, T)>>;

static SAT: Memo<Value> = Mutex::new(None);
static ALERTAS: Memo<Value> = Mutex::new(None);

fn memo<T: Clone>(m: &Memo<T>, ttl: u64) -> Option<T> {
    let g = m.lock().unwrap();
    let (quando, v) = g.as_ref()?;
    (quando.elapsed() < Duration::from_secs(ttl)).then(|| v.clone())
}

fn guardar<T: Clone>(m: &Memo<T>, v: &T) {
    *m.lock().unwrap() = Some((Instant::now(), v.clone()));
}

fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Unix → "2026-10-05T12:00:00Z" (calendário civil, sem dependência).
fn iso(t: i64) -> String {
    let (d, s) = (t.div_euclid(86400), t.rem_euclid(86400));
    let z = d + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + (mes <= 2) as i64;
    format!(
        "{ano:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s % 3600 / 60,
        s % 60
    )
}

async fn get_json(http: &Client, url: &str) -> Result<Value, String> {
    http.get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Passo em segundos e atraso do GIBS por camada.
fn camada_gibs(c: &str) -> Option<(&'static str, i64)> {
    match c {
        "ir" => Some(("GOES-East_ABI_Band13_Clean_Infrared", 600)),
        "imerg" => Some(("IMERG_Precipitation_Rate_30min", 1800)),
        _ => None,
    }
}

fn url_gibs(c: &str, t: &str, z: u32, x: u32, y: u32) -> String {
    format!(
        "{GIBS}/{}/default/{t}/GoogleMapsCompatible_Level6/{z}/{y}/{x}.png",
        camada_gibs(c).unwrap().0
    )
}

/// Instantes do GIBS: o tempo inexistente responde 404.
async fn instantes(http: &Client, c: &str, n: i64) -> Result<Vec<String>, String> {
    let passo = camada_gibs(c).unwrap().1;
    let mut t = agora() / passo * passo;
    // ponytail: recua no máximo 48 h; se o GIBS ficar mais atrás que isso, dá erro
    for _ in 0..(172800 / passo) {
        let r = http
            .get(url_gibs(c, &iso(t), 0, 0, 0))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if r.status().is_success() && r.content_length() != Some(0) {
            return Ok((0..n).rev().map(|i| iso(t - i * passo)).collect());
        }
        t -= passo;
    }
    Err(format!("sem instante recente em {c}"))
}

pub async fn satelite(http: &Client) -> Result<Value, String> {
    if let Some(v) = memo(&SAT, 300) {
        return Ok(v);
    }
    let (ir, imerg) = tokio::join!(instantes(http, "ir", 12), instantes(http, "imerg", 6));
    let v = json!({"ir": ir?, "imerg": imerg?});
    guardar(&SAT, &v);
    Ok(v)
}

fn inmet() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        Client::builder()
            .http1_only()
            .user_agent("Mozilla/5.0")
            .build()
            .unwrap()
    })
}

pub async fn alertas(_http: &Client) -> Result<Value, String> {
    if let Some(v) = memo(&ALERTAS, 600) {
        return Ok(v);
    }
    let j = get_json(inmet(), INMET).await?;
    let mut features = vec![];
    for quando in ["hoje", "futuro"] {
        for a in j[quando].as_array().into_iter().flatten() {
            let Some(geometry) = a["poligono"]
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
            else {
                continue;
            };
            features.push(json!({"type":"Feature","geometry":geometry,"properties":{
                "evento":a["descricao"],"severidade":a["severidade"],"cor":a["aviso_cor"],
                "riscos":a["riscos"],"instrucoes":a["instrucoes"],"inicio":a["inicio"],
                "fim":a["fim"],"estados":a["estados"],"quando":quando}}));
        }
    }
    let v = json!({"type":"FeatureCollection","features":features});
    guardar(&ALERTAS, &v);
    Ok(v)
}
