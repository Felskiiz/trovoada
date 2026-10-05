//! ICON, UKMO e CMA pelo Open-Meteo, mais o detalhe de um ponto.
use crate::modelo::{Campos, LAT0, LON0, NX, NY, PASSO, Previsao, QUADRO};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const URL: &str = "https://api.open-meteo.com/v1/forecast";
/// (fonte, modelo na API)
const MODELOS: [(&str, &str); 3] = [
    ("icon", "icon_global"),
    ("ukmo", "ukmo_global_deterministic_10km"),
    ("cma", "cma_grapes_global"),
];
const HORARIAS: &str = "precipitation,cape,wind_gusts_10m,wind_speed_10m,wind_direction_10m,temperature_2m,temperature_850hPa,pressure_msl";
const LOTE: usize = 100;
const ENTRE_LOTES: Duration = Duration::from_secs(65);
/// Espera depois de cada 429 (3 tentativas).
const ESPERA_429: [u64; 3] = [120, 300, 900];
const VALIDADE: i64 = 8 * 3600;
/// Malha de 1°: lat 5 - i, lon -73 + j.
const M: usize = 39;
/// ~150 km em graus.
const FOLGA: f64 = 1.35;

#[derive(Serialize, Deserialize)]
struct Resp {
    hourly: HashMap<String, Vec<Option<f64>>>,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    /// Unix s da busca.
    t: i64,
    /// (lat, lon) inteiros, na ordem das respostas.
    locais: Vec<(i32, i32)>,
    resp: Vec<Resp>,
}

fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// Ray casting; vale para anéis de polígono com buracos (par/ímpar).
fn dentro(aneis: &[Vec<[f64; 2]>], lat: f64, lon: f64) -> bool {
    let mut c = false;
    for r in aneis {
        for k in 0..r.len() {
            let (a, b) = (r[k], r[(k + 1) % r.len()]);
            if (a[1] > lat) != (b[1] > lat)
                && lon < (b[0] - a[0]) * (lat - a[1]) / (b[1] - a[1]) + a[0]
            {
                c = !c;
            }
        }
    }
    c
}

/// Pontos da malha de 1° em terra a até ~150 km do Brasil.
fn locais() -> Result<Vec<(i32, i32)>, String> {
    let txt = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/web/dados/ufs.json"))
        .map_err(|e| format!("ufs.json: {e}"))?;
    let j: Value = serde_json::from_str(&txt).map_err(|e| format!("ufs.json: {e}"))?;
    let mut pols: Vec<Vec<Vec<[f64; 2]>>> = vec![];
    for f in j["features"].as_array().ok_or("ufs.json sem features")? {
        let g = &f["geometry"];
        let mut p = |v: &Value| {
            pols.push(serde_json::from_value(v.clone()).unwrap_or_default());
        };
        match g["type"].as_str() {
            Some("Polygon") => p(&g["coordinates"]),
            _ => g["coordinates"]
                .as_array()
                .into_iter()
                .flatten()
                .for_each(&mut p),
        }
    }
    let mut v = vec![];
    for i in 0..M as i32 {
        for j in 0..M as i32 {
            let (la, lo) = (5 - i, -73 + j);
            let achou = [-FOLGA, 0.0, FOLGA].iter().any(|dla| {
                [-FOLGA, 0.0, FOLGA].iter().any(|dlo| {
                    pols.iter()
                        .any(|p| dentro(p, la as f64 + dla, lo as f64 + dlo))
                })
            });
            if achou {
                v.push((la, lo));
            }
        }
    }
    println!("openmeteo: {} pontos na malha", v.len());
    Ok(v)
}

/// GET com as variáveis horárias; 429 espera e repete, 414 devolve Err("414").
async fn pedir(http: &Client, q: &[(&str, String)]) -> Result<Vec<Resp>, String> {
    for esp in ESPERA_429.iter().map(|s| Some(*s)).chain([None]) {
        let r = http
            .get(URL)
            .query(q)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        match r.status() {
            StatusCode::TOO_MANY_REQUESTS => match esp {
                Some(s) => {
                    eprintln!("openmeteo: 429, espera {s}s");
                    tokio::time::sleep(Duration::from_secs(s)).await;
                }
                None => return Err("429 persistente".into()),
            },
            StatusCode::URI_TOO_LONG => return Err("414".into()),
            s if !s.is_success() => {
                return Err(format!("HTTP {s}: {}", r.text().await.unwrap_or_default()));
            }
            _ => {
                let b = r.bytes().await.map_err(|e| e.to_string())?;
                // Um local só vem como objeto, não como lista.
                return serde_json::from_slice::<Vec<Resp>>(&b)
                    .or_else(|_| serde_json::from_slice::<Resp>(&b).map(|x| vec![x]))
                    .map_err(|e| format!("resposta: {e}"));
            }
        }
    }
    unreachable!()
}

/// Busca todos os locais em lotes, com pausa entre eles; 414 parte o lote ao meio.
async fn buscar(http: &Client, locais: &[(i32, i32)]) -> Result<Vec<Resp>, String> {
    let modelos = MODELOS.map(|m| m.1).join(",");
    let mut fila: std::collections::VecDeque<&[(i32, i32)]> = locais.chunks(LOTE).collect();
    let mut out = vec![];
    let mut primeiro = true;
    while let Some(l) = fila.pop_front() {
        if !primeiro {
            tokio::time::sleep(ENTRE_LOTES).await;
        }
        primeiro = false;
        let lista = |f: fn(&(i32, i32)) -> i32| {
            l.iter()
                .map(|p| f(p).to_string())
                .collect::<Vec<_>>()
                .join(",")
        };
        let q = [
            ("latitude", lista(|p| p.0)),
            ("longitude", lista(|p| p.1)),
            ("models", modelos.clone()),
            ("hourly", HORARIAS.into()),
            ("forecast_days", "4".into()),
            ("timeformat", "unixtime".into()),
            ("timezone", "GMT".into()),
        ];
        match pedir(http, &q).await {
            Ok(r) if r.len() == l.len() => out.extend(r),
            Ok(r) => return Err(format!("lote de {} voltou com {}", l.len(), r.len())),
            Err(e) if e == "414" && l.len() > 1 => {
                let (a, b) = l.split_at(l.len() / 2);
                fila.push_front(b);
                fila.push_front(a);
                primeiro = true;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(out)
}

/// Interpolação bilinear da malha de 1° (M*M, NaN onde falta) para as células de 0,5°.
/// Vizinho de peso zero não conta; célula sem os vizinhos que pesam fica NaN.
fn bilinear(nos: &[f32]) -> Vec<f32> {
    let mut out = vec![f32::NAN; NX * NY];
    for y in 0..NY {
        for x in 0..NX {
            let fi = (5.0 - (LAT0 - y as f32 * PASSO)) as f64;
            let fj = ((LON0 + x as f32 * PASSO) + 73.0) as f64;
            if fi < 0.0 || fj < 0.0 || fi > (M - 1) as f64 || fj > (M - 1) as f64 {
                continue;
            }
            let (i0, j0) = (fi.floor() as usize, fj.floor() as usize);
            let (di, dj) = (fi - i0 as f64, fj - j0 as f64);
            let mut s = 0.0;
            let mut ok = true;
            for (i, wi) in [(i0, 1.0 - di), (i0 + 1, di)] {
                for (j, wj) in [(j0, 1.0 - dj), (j0 + 1, dj)] {
                    let w = wi * wj;
                    if w > 0.0 {
                        let n = nos[i * M + j];
                        ok &= n.is_finite();
                        s += w * n as f64;
                    }
                }
            }
            if ok {
                out[y * NX + x] = s as f32;
            }
        }
    }
    out
}

/// u, v (m/s) de vento em km/h e direção meteorológica (de onde vem).
fn uv(kmh: f64, dir: f64) -> (f32, f32) {
    let (s, d) = (kmh / 3.6, dir.to_radians());
    ((-s * d.sin()) as f32, (-s * d.cos()) as f32)
}

/// Série horária de uma variável de um modelo; NaN onde falta.
fn serie(r: &Resp, var: &str, mod_: &str) -> Vec<f64> {
    r.hourly
        .get(&format!("{var}_{mod_}"))
        .map(|v| v.iter().map(|x| x.unwrap_or(f64::NAN)).collect())
        .unwrap_or_default()
}

fn monta(c: &Cache) -> Vec<Previsao> {
    let rodada = c.t - c.t % 3600;
    let tempo: Vec<i64> = c
        .resp
        .first()
        .and_then(|r| r.hourly.get("time"))
        .map(|v| v.iter().map(|x| x.unwrap_or(0.0) as i64).collect())
        .unwrap_or_default();
    // Posição de cada local na malha de nós.
    let pos: Vec<usize> = c
        .locais
        .iter()
        .map(|&(la, lo)| (5 - la) as usize * M + (lo + 73) as usize)
        .collect();
    MODELOS
        .iter()
        .map(|&(fonte, m)| {
            let s: Vec<[Vec<f64>; 8]> = c
                .resp
                .iter()
                .map(|r| {
                    [
                        "precipitation",
                        "cape",
                        "wind_gusts_10m",
                        "wind_speed_10m",
                        "wind_direction_10m",
                        "temperature_2m",
                        "temperature_850hPa",
                        "pressure_msl",
                    ]
                    .map(|v| serie(r, v, m))
                })
                .collect();
            let mut passos = BTreeMap::new();
            for (k, &t) in tempo.iter().enumerate() {
                if t % QUADRO != 0 || k < 2 {
                    continue;
                }
                let mut nos = vec![vec![f32::NAN; M * M]; 8];
                for (p, &n) in s.iter().zip(&pos) {
                    let g = |f: usize, k: usize| p[f].get(k).copied().unwrap_or(f64::NAN);
                    let chuva = (g(0, k) + g(0, k - 1) + g(0, k - 2)) / 3.0;
                    let raj = g(2, k).max(g(2, k - 1)).max(g(2, k - 2));
                    // f64::max ignora NaN; exige as 3 horas.
                    let raj = if (0..3).any(|d| g(2, k - d).is_nan()) {
                        f64::NAN
                    } else {
                        raj
                    };
                    let (u, v) = uv(g(3, k), g(4, k));
                    for (f, x) in [
                        chuva,
                        g(1, k),
                        raj,
                        u as f64,
                        v as f64,
                        g(5, k),
                        g(6, k),
                        g(7, k),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        nos[f][n] = x as f32;
                    }
                }
                let mut b: Vec<Vec<f32>> = nos.iter().map(|n| bilinear(n)).collect();
                let mut d = b.drain(..);
                passos.insert(
                    t,
                    Campos {
                        chuva: d.next().unwrap(),
                        cape: d.next().unwrap(),
                        rajada: d.next().unwrap(),
                        u: d.next().unwrap(),
                        v: d.next().unwrap(),
                        t2m: d.next().unwrap(),
                        t850: d.next().unwrap(),
                        pmsl: d.next().unwrap(),
                    },
                );
            }
            Previsao {
                fonte,
                rodada,
                passos,
            }
        })
        .collect()
}

/// Uma Previsao por modelo (fonte "icon", "ukmo", "cma").
pub async fn grades(http: &Client, cache: &Path) -> Result<Vec<Previsao>, String> {
    let arq = cache.join("openmeteo.json");
    let velho = tokio::fs::read(&arq)
        .await
        .ok()
        .and_then(|b| serde_json::from_slice::<Cache>(&b).ok())
        .filter(|c| agora() - c.t < VALIDADE);
    let c = match velho {
        Some(c) => c,
        None => {
            let locais = locais()?;
            let resp = buscar(http, &locais).await?;
            let c = Cache {
                t: agora(),
                locais,
                resp,
            };
            let tmp = cache.join("openmeteo.json.tmp");
            tokio::fs::write(&tmp, serde_json::to_vec(&c).map_err(|e| e.to_string())?)
                .await
                .map_err(|e| e.to_string())?;
            tokio::fs::rename(&tmp, &arq)
                .await
                .map_err(|e| e.to_string())?;
            c
        }
    };
    Ok(monta(&c))
}

const PONTO_MODELOS: [(&str, &str); 5] = [
    ("ecmwf", "ecmwf_ifs025"),
    ("gfs", "gfs_global"),
    ("icon", "icon_global"),
    ("ukmo", "ukmo_global_deterministic_10km"),
    ("cma", "cma_grapes_global"),
];
/// (saída, variável da API)
const PONTO_VARS: [(&str, &str); 7] = [
    ("chuva", "precipitation"),
    ("t2m", "temperature_2m"),
    ("vento", "wind_speed_10m"),
    ("dir", "wind_direction_10m"),
    ("rajada", "wind_gusts_10m"),
    ("cape", "cape"),
    ("codigo", "weather_code"),
];

type CachePonto = Mutex<HashMap<(i32, i32), (Instant, Value)>>;

pub async fn ponto(http: &Client, lat: f32, lon: f32) -> Result<Value, String> {
    static C: OnceLock<CachePonto> = OnceLock::new();
    let c = C.get_or_init(Default::default);
    let chave = ((lat * 10.0).round() as i32, (lon * 10.0).round() as i32);
    if let Some((t, v)) = c.lock().unwrap().get(&chave)
        && t.elapsed() < Duration::from_secs(1800)
    {
        return Ok(v.clone());
    }
    let vars = PONTO_VARS.map(|v| v.1).join(",");
    let q = [
        ("latitude", lat.to_string()),
        ("longitude", lon.to_string()),
        ("models", PONTO_MODELOS.map(|m| m.1).join(",")),
        ("hourly", vars),
        ("forecast_days", "3".into()),
        ("timeformat", "unixtime".into()),
        ("timezone", "GMT".into()),
    ];
    let r = pedir(http, &q)
        .await?
        .into_iter()
        .next()
        .ok_or("sem dados")?;
    let horas: Vec<Value> = r
        .hourly
        .get("time")
        .into_iter()
        .flatten()
        .map(|x| json!(x.map(|x| x as i64)))
        .collect();
    let mut modelos = serde_json::Map::new();
    for (nome, m) in PONTO_MODELOS {
        let mut o = serde_json::Map::new();
        for (saida, v) in PONTO_VARS {
            let a = r.hourly.get(&format!("{v}_{m}"));
            o.insert(
                saida.into(),
                a.map_or_else(|| json!(vec![Value::Null; horas.len()]), |a| json!(a)),
            );
        }
        modelos.insert(nome.into(), o.into());
    }
    let v = json!({"lat": lat, "lon": lon, "horas": horas, "modelos": modelos});
    let mut m = c.lock().unwrap();
    m.retain(|_, (t, _)| t.elapsed() < Duration::from_secs(1800));
    m.insert(chave, (Instant::now(), v.clone()));
    Ok(v)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{celula, lat, lon};

    #[test]
    fn bilinear_e_vento() {
        // Plano 2*lat + 3*lon nos nós de 1°: a bilinear reproduz o plano.
        let mut nos = vec![f32::NAN; M * M];
        for i in 0..M {
            for j in 0..M {
                nos[i * M + j] = 2.0 * (5 - i as i32) as f32 + 3.0 * (-73 + j as i32) as f32;
            }
        }
        let g = bilinear(&nos);
        let c = celula(-25.34, -52.37).unwrap();
        let (y, x) = (c / NX, c % NX);
        let esp = 2.0 * lat(y) + 3.0 * lon(x);
        assert!((g[c] - esp).abs() < 1e-3, "{} vs {esp}", g[c]);
        // Fora da malha (lat 6) e sem vizinho: NaN.
        assert!(g[0].is_nan());
        nos[(5 + 25) * M + (73 - 52)] = f32::NAN;
        assert!(bilinear(&nos)[c].is_nan());
        // Vento de norte (0°) sopra para sul: v < 0; de leste (90°) sopra para oeste: u < 0.
        let (u, v) = uv(36.0, 0.0);
        assert!(u.abs() < 1e-5 && (v + 10.0).abs() < 1e-5);
        let (u, v) = uv(36.0, 90.0);
        assert!((u + 10.0).abs() < 1e-5 && v.abs() < 1e-5);
    }

    fn laranjeiras(p: &[Previsao]) {
        let c = celula(-25.34, -52.37).unwrap();
        for p in p {
            let ok = p
                .passos
                .values()
                .next()
                .map_or(0, |c| c.t2m.iter().filter(|x| x.is_finite()).count());
            let (t, k) = p.passos.iter().nth(2).unwrap();
            println!(
                "{}: {} passos, {} células válidas, Laranjeiras t2m={} chuva={} (t={t})",
                p.fonte,
                p.passos.len(),
                ok,
                k.t2m[c],
                k.chuva[c]
            );
        }
    }

    #[tokio::test]
    #[ignore]
    async fn rede_lote_pequeno() {
        let http = Client::new();
        let locais = vec![(-26, -53), (-26, -52), (-25, -53), (-25, -52), (-24, -52)];
        let resp = buscar(&http, &locais).await.unwrap();
        let c = Cache {
            t: agora(),
            locais,
            resp,
        };
        laranjeiras(&monta(&c));
        let v = ponto(&http, -25.34, -52.37).await.unwrap();
        for (m, o) in v["modelos"].as_object().unwrap() {
            println!(
                "{m}: {}",
                serde_json::to_string(o)
                    .unwrap()
                    .chars()
                    .take(330)
                    .collect::<String>()
            );
        }
        assert_eq!(v["modelos"].as_object().unwrap().len(), 5);
    }

    /// Gasta a cota cheia: rodar uma vez só. Grava ~/.cache/trovoada/openmeteo.json.
    #[tokio::test]
    #[ignore]
    async fn rede_completa() {
        let dir = std::path::PathBuf::from(std::env::var("HOME").unwrap()).join(".cache/trovoada");
        std::fs::create_dir_all(&dir).unwrap();
        laranjeiras(&grades(&Client::new(), &dir).await.unwrap());
    }
}
