mod consenso;
mod grib;
mod modelo;
mod obs;
mod openmeteo;
mod raios;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use modelo::{Consenso, Previsao, Status};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;

const ENDERECO: &str = "0.0.0.0:8077";

struct App {
    http: reqwest::Client,
    casa: PathBuf,
    cache: PathBuf,
    previsoes: RwLock<BTreeMap<&'static str, Previsao>>,
    status: RwLock<BTreeMap<&'static str, Status>>,
    consenso: RwLock<Vec<Arc<Consenso>>>,
    raios: raios::Raios,
    pedido_raios: AtomicI64,
}

type St = State<Arc<App>>;

fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

#[tokio::main]
async fn main() {
    let casa = PathBuf::from(std::env::var("HOME").unwrap());
    let cache = casa.join(".cache/trovoada");
    std::fs::create_dir_all(&cache).unwrap();
    let app = Arc::new(App {
        http: reqwest::Client::builder()
            .user_agent("trovoada/0.1 (radar local)")
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap(),
        casa,
        cache,
        previsoes: Default::default(),
        status: Default::default(),
        consenso: Default::default(),
        raios: Default::default(),
        pedido_raios: AtomicI64::new(0),
    });
    tokio::spawn(ciclo(app.clone(), "gfs", &["gfs"], 3600));
    tokio::spawn(ciclo(app.clone(), "ecmwf", &["ecmwf"], 3600));
    tokio::spawn(ciclo(
        app.clone(),
        "openmeteo",
        &["icon", "ukmo", "cma"],
        1800,
    ));
    tokio::spawn(ciclo_raios(app.clone()));
    let web = concat!(env!("CARGO_MANIFEST_DIR"), "/web");
    let rotas = Router::new()
        .route("/api/tempos", get(tempos))
        .route("/api/campo", get(campo))
        .route("/api/ponto", get(ponto))
        .route("/api/radar", get(radar))
        .route("/api/satelite", get(satelite))
        .route("/api/alertas", get(alertas))
        .route("/api/raios", get(raios_))
        .route("/api/cores", get(cores))
        .route("/api/lugar", get(lugar))
        .route("/tile/{camada}/{t}/{z}/{x}/{y}", get(tile))
        .fallback_service(ServeDir::new(web))
        .with_state(app);
    let ouvinte = tokio::net::TcpListener::bind(ENDERECO).await.unwrap();
    println!("trovoada em http://{ENDERECO}");
    axum::serve(ouvinte, rotas).await.unwrap();
}

async fn buscar(app: &App, qual: &str) -> Result<Vec<Previsao>, String> {
    match qual {
        "gfs" => grib::gfs(&app.http).await.map(|p| vec![p]),
        "ecmwf" => grib::ecmwf(&app.http).await.map(|p| vec![p]),
        _ => openmeteo::grades(&app.http, &app.cache).await,
    }
}

/// Busca uma família de fontes a cada `intervalo` s; em erro mantém o dado anterior.
async fn ciclo(app: Arc<App>, qual: &'static str, nomes: &'static [&'static str], intervalo: u64) {
    loop {
        match buscar(&app, qual).await {
            Ok(ps) => {
                let mut st = app.status.write().await;
                let mut pv = app.previsoes.write().await;
                for p in ps {
                    st.insert(
                        p.fonte,
                        Status {
                            rodada: Some(p.rodada),
                            atualizado: Some(agora()),
                            erro: None,
                        },
                    );
                    pv.insert(p.fonte, p);
                }
                drop((st, pv));
                recalcular(&app).await;
            }
            Err(e) => {
                eprintln!("{qual}: {e}");
                let mut st = app.status.write().await;
                for n in nomes {
                    st.entry(n).or_default().erro = Some(e.clone());
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(intervalo)).await;
    }
}

async fn recalcular(app: &App) {
    let pv = app.previsoes.read().await;
    let v: Vec<&Previsao> = pv.values().collect();
    let c = consenso::tempos(&v, agora())
        .into_iter()
        .map(|t| Arc::new(consenso::calcular(&v, t)))
        .collect();
    *app.consenso.write().await = c;
}

/// GLM só é buscado enquanto alguém pediu raios nos últimos 5 min.
async fn ciclo_raios(app: Arc<App>) {
    loop {
        if agora() - app.pedido_raios.load(Ordering::Relaxed) < 300 {
            if let Err(e) = app.raios.atualizar(&app.http).await {
                eprintln!("raios: {e}");
            }
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

fn resposta(r: Result<Value, String>) -> Response {
    match r {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::BAD_GATEWAY, e).into_response(),
    }
}

async fn tempos(State(a): St) -> Json<Value> {
    let c = a.consenso.read().await;
    let st = a.status.read().await;
    Json(json!({
        "agora": agora(),
        "tempos": c.iter().map(|c| c.tempo).collect::<Vec<_>>(),
        "fontes": *st,
        "grade": {"nx": modelo::NX, "ny": modelo::NY, "lat0": modelo::LAT0, "lon0": modelo::LON0, "passo": modelo::PASSO},
    }))
}

#[derive(Deserialize)]
struct T {
    t: Option<i64>,
}

/// Quadro mais próximo de `t` (ou de agora).
async fn campo(State(a): St, Query(q): Query<T>) -> Response {
    let t = q.t.unwrap_or_else(agora);
    let c = a.consenso.read().await;
    match c.iter().min_by_key(|c| (c.tempo - t).abs()) {
        Some(c) => Json(serde_json::to_value(&**c).unwrap()).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "sem previsão ainda").into_response(),
    }
}

#[derive(Deserialize)]
struct Ponto {
    lat: f32,
    lon: f32,
}

async fn ponto(State(a): St, Query(p): Query<Ponto>) -> Response {
    resposta(openmeteo::ponto(&a.http, p.lat, p.lon).await)
}

async fn radar(State(a): St) -> Response {
    resposta(obs::radar(&a.http).await)
}

async fn satelite(State(a): St) -> Response {
    resposta(obs::satelite(&a.http).await)
}

async fn alertas(State(a): St) -> Response {
    resposta(obs::alertas(&a.http).await)
}

async fn raios_(State(a): St) -> Json<Vec<[f32; 3]>> {
    if agora() - a.pedido_raios.swap(agora(), Ordering::Relaxed) > 300 {
        let a = a.clone();
        tokio::spawn(async move { a.raios.atualizar(&a.http).await });
    }
    Json(a.raios.lista())
}

fn ler_json(p: PathBuf) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(p).ok()?).ok()
}

/// Paleta viva da petricor; sem ela, a paleta fixa.
async fn cores(State(a): St) -> Json<Value> {
    Json(
        ler_json(a.casa.join(".config/quickshell/petricor/cores.json")).unwrap_or_else(|| {
            json!({"breu":"#131110","terra":"#1B1817","barro":"#241F1D","limo":"#302A27","areia":"#EFE9E4","acento":"#74D0E4","acento_forte":"#74D0E4","musgo":"#97C489","ocre":"#DFA765","ferro":"#E08C7E"})
        }),
    )
}

async fn lugar() -> Json<Value> {
    Json(json!({"nome":"Quedas do Iguaçu","uf":"PR","lat":-25.4492,"lon":-52.9102}))
}

async fn tile(
    State(a): St,
    Path((camada, t, z, x, y)): Path<(String, String, u32, u32, String)>,
) -> Response {
    let Ok(y) = y.trim_end_matches(".png").parse() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    match obs::tile(&a.http, &a.cache, &camada, &t, z, x, y).await {
        Ok(png) => (
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "max-age=86400"),
            ],
            png,
        )
            .into_response(),
        Err(s) => s.into_response(),
    }
}
