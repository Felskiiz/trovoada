//! Observação: radar RainViewer, satélite GIBS (proxy de tiles com cache) e alertas INMET.
use axum::http::StatusCode;
use reqwest::Client;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const RV: &str = "https://api.rainviewer.com/public/weather-maps.json";
const GIBS: &str = "https://gibs.earthdata.nasa.gov/wmts/epsg3857/best";
const INMET: &str = "https://apiprevmet3.inmet.gov.br/avisos/ativos";

/// Valor memorizado com validade.
type Memo<T> = Mutex<Option<(Instant, T)>>;

static RADAR: Memo<(String, Vec<(i64, String)>)> = Mutex::new(None);
static SAT: Memo<Value> = Mutex::new(None);
static ALERTAS: Memo<Value> = Mutex::new(None);
static LIMPEZA: AtomicU64 = AtomicU64::new(0);

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

async fn quadros(http: &Client, forcar: bool) -> Result<(String, Vec<(i64, String)>), String> {
    if !forcar && let Some(v) = memo(&RADAR, 120) {
        return Ok(v);
    }
    let j = get_json(http, RV).await?;
    let host = j["host"].as_str().ok_or("sem host")?.to_string();
    let mut l: Vec<(i64, String)> = j["radar"]["past"]
        .as_array()
        .ok_or("sem radar.past")?
        .iter()
        .filter_map(|q| Some((q["time"].as_i64()?, q["path"].as_str()?.to_string())))
        .collect();
    l.sort();
    let v = (host, l);
    guardar(&RADAR, &v);
    Ok(v)
}

pub async fn radar(http: &Client) -> Result<Value, String> {
    let (_, l) = quadros(http, false).await?;
    Ok(Value::Array(
        l.iter().map(|(t, _)| json!({"t": t})).collect(),
    ))
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

/// Apaga pastas de instante com mais de 24 h, no máximo uma vez por hora.
async fn limpar(raiz: &Path) {
    let n = agora() as u64;
    if n - LIMPEZA.load(Ordering::Relaxed) < 3600 {
        return;
    }
    LIMPEZA.store(n, Ordering::Relaxed);
    let Ok(mut camadas) = tokio::fs::read_dir(raiz).await else {
        return;
    };
    while let Ok(Some(c)) = camadas.next_entry().await {
        let Ok(mut ts) = tokio::fs::read_dir(c.path()).await else {
            continue;
        };
        while let Ok(Some(t)) = ts.next_entry().await {
            // ponytail: usa o mtime da pasta, que muda quando entra tile novo
            let velho = t
                .metadata()
                .await
                .and_then(|m| m.modified())
                .is_ok_and(|m| m.elapsed().is_ok_and(|e| e > Duration::from_secs(86400)));
            if velho {
                let _ = tokio::fs::remove_dir_all(t.path()).await;
            }
        }
    }
}

/// PNG do tile; camada é "radar", "ir" ou "imerg".
pub async fn tile(
    http: &Client,
    cache: &Path,
    camada: &str,
    t: &str,
    z: u32,
    x: u32,
    y: u32,
) -> Result<Vec<u8>, StatusCode> {
    let maximo = match camada {
        "radar" => 7,
        "ir" | "imerg" => 6,
        _ => return Err(StatusCode::NOT_FOUND),
    };
    // t vira nome de pasta: só caracteres de tempo
    if z > maximo || t.is_empty() || !t.chars().all(|c| c.is_ascii_digit() || "TZ:-".contains(c)) {
        return Err(StatusCode::NOT_FOUND);
    }
    let raiz = cache.join("tiles");
    let arq = raiz.join(format!("{camada}/{t}/{z}/{x}/{y}.png"));
    if let Ok(b) = tokio::fs::read(&arq).await {
        return Ok(b);
    }
    limpar(&raiz).await;
    let url = if camada == "radar" {
        let t: i64 = t.parse().map_err(|_| StatusCode::NOT_FOUND)?;
        let achar = |l: &[(i64, String)]| l.iter().find(|q| q.0 == t).map(|q| q.1.clone());
        let (mut host, l) = quadros(http, false)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?;
        let mut path = achar(&l);
        if path.is_none() {
            let (h, l) = quadros(http, true)
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?;
            (host, path) = (h, achar(&l));
        }
        format!(
            "{host}{}/256/{z}/{x}/{y}/2/1_1.png",
            path.ok_or(StatusCode::NOT_FOUND)?
        )
    } else {
        url_gibs(camada, t, z, x, y)
    };
    let r = http
        .get(url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    match r.status() {
        StatusCode::NOT_FOUND => return Err(StatusCode::NOT_FOUND),
        s if !s.is_success() => return Err(StatusCode::BAD_GATEWAY),
        _ => {}
    }
    let b = r
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?
        .to_vec();
    if b.is_empty() {
        return Err(StatusCode::NOT_FOUND);
    }
    if let Some(d) = arq.parent()
        && tokio::fs::create_dir_all(d).await.is_ok()
    {
        let tmp = arq.with_extension("tmp");
        if tokio::fs::write(&tmp, &b).await.is_ok() {
            let _ = tokio::fs::rename(&tmp, &arq).await;
        }
    }
    Ok(b)
}
