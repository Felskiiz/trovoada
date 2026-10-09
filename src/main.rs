mod consenso;
mod grib;
mod modelo;
mod obs;
mod openmeteo;
mod raios;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use modelo::{Consenso, Previsao, Status};
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
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map_or(casa.join(".cache"), PathBuf::from)
        .join("trovoada");
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
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--exportar") {
        return exportar(
            &app,
            PathBuf::from(args.next().expect("--exportar <pasta>")),
        )
        .await;
    }
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
        .route("/api/tempos.json", get(tempos))
        .route("/api/campo/{t}", get(campo))
        .route("/api/satelite.json", get(satelite))
        .route("/api/alertas.json", get(alertas))
        .route("/api/raios.json", get(raios_))
        .route("/api/cores.json", get(cores))
        .route("/api/lugar.json", get(lugar))
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
        rodada(&app, qual, nomes).await;
        tokio::time::sleep(Duration::from_secs(intervalo)).await;
    }
}

async fn rodada(app: &App, qual: &'static str, nomes: &'static [&'static str]) {
    match buscar(app, qual).await {
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
            recalcular(app).await;
        }
        Err(e) => {
            eprintln!("{qual}: {e}");
            let mut st = app.status.write().await;
            for n in nomes {
                st.entry(n).or_default().erro = Some(e.clone());
            }
        }
    }
}

/// Uma rodada de cada fonte, gravada como arquivos em `dir/api` com os mesmos nomes
/// das rotas, para hospedagem estática (GitHub Pages).
async fn exportar(app: &App, dir: PathBuf) {
    tokio::join!(
        rodada(app, "gfs", &["gfs"]),
        rodada(app, "ecmwf", &["ecmwf"]),
        rodada(app, "openmeteo", &["icon", "ukmo", "cma"]),
    );
    if let Err(e) = app.raios.atualizar(&app.http).await {
        eprintln!("raios: {e}");
    }
    let api = dir.join("api");
    std::fs::create_dir_all(api.join("campo")).unwrap();
    let grava = |nome: String, v: Vec<u8>| std::fs::write(api.join(nome), v).unwrap();
    let js = |v: &Value| serde_json::to_vec(v).unwrap();
    grava("tempos.json".into(), js(&tempos_v(app).await));
    for c in app.consenso.read().await.iter() {
        grava(
            format!("campo/{}.json", c.tempo),
            serde_json::to_vec(&**c).unwrap(),
        );
    }
    // Fonte fora do ar: sem o arquivo, o site mostra a camada vazia.
    for (nome, r) in [
        ("satelite.json", obs::satelite(&app.http).await),
        ("alertas.json", obs::alertas(&app.http).await),
    ] {
        match r {
            Ok(v) => grava(nome.into(), js(&v)),
            Err(e) => eprintln!("{nome}: {e}"),
        }
    }
    grava("raios.json".into(), js(&json!(app.raios.lista())));
    grava("cores.json".into(), js(&cores_v(app)));
    grava("lugar.json".into(), js(&lugar_v()));
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
    Json(tempos_v(&a).await)
}

async fn tempos_v(a: &App) -> Value {
    let c = a.consenso.read().await;
    let st = a.status.read().await;
    json!({
        "agora": agora(),
        "tempos": c.iter().map(|c| c.tempo).collect::<Vec<_>>(),
        "fontes": *st,
        "grade": {"nx": modelo::NX, "ny": modelo::NY, "lat0": modelo::LAT0, "lon0": modelo::LON0, "passo": modelo::PASSO},
    })
}

/// Quadro mais próximo de `t` (`/api/campo/{t}.json`).
async fn campo(State(a): St, Path(t): Path<String>) -> Response {
    let Ok(t) = t.trim_end_matches(".json").parse::<i64>() else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let c = a.consenso.read().await;
    match c.iter().min_by_key(|c| (c.tempo - t).abs()) {
        Some(c) => Json(serde_json::to_value(&**c).unwrap()).into_response(),
        None => (StatusCode::SERVICE_UNAVAILABLE, "sem previsão ainda").into_response(),
    }
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
fn cores_v(a: &App) -> Value {
    ler_json(a.casa.join(".config/quickshell/petricor/cores.json")).unwrap_or_else(|| {
        json!({"breu":"#131110","terra":"#1B1817","barro":"#241F1D","limo":"#302A27","areia":"#EFE9E4","acento":"#74D0E4","acento_forte":"#74D0E4","musgo":"#97C489","ocre":"#DFA765","ferro":"#E08C7E"})
    })
}

async fn cores(State(a): St) -> Json<Value> {
    Json(cores_v(&a))
}

fn lugar_v() -> Value {
    json!({"nome":"Quedas do Iguaçu","uf":"PR","lat":-25.4492,"lon":-52.9102})
}

async fn lugar() -> Json<Value> {
    Json(lugar_v())
}
