//! GFS (NOMADS) e ECMWF IFS (open data) em GRIB2, recortados para a grade.
use crate::modelo::{Campos, LAT0, LON0, N, NX, NY, PASSO, Previsao};
use grib::LatLons;
use reqwest::Client;
use serde_json::Value;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Semaphore;

static GFS: Mutex<Option<Previsao>> = Mutex::new(None);
static ECMWF: Mutex<Option<Previsao>> = Mutex::new(None);

const PASSOS: std::ops::RangeInclusive<u32> = 0..=24; // x3 h = f000 a f072

/// Mensagem GRIB já recortada para a grade 0,5°.
struct Msg {
    cat: u8,
    num: u8,
    /// Tipo e valor da superfície (1 = chão, 100 = Pa, 101 = nível do mar, 103 = m).
    sup: u8,
    nivel: f64,
    /// Janela de acúmulo em horas desde a rodada: [ini, ini+dur]; dur 0 se não for acúmulo.
    ini: u32,
    dur: u32,
    campo: Vec<f32>,
}

/// Decodifica todas as mensagens. A ordem de varredura vem do próprio GRIB (latlons), então
/// amostrar "um sim, um não" vira pegar só os pontos que caem em múltiplos de 0,5°.
fn decodifica(bytes: Vec<u8>) -> Result<Vec<Msg>, String> {
    let g = grib::from_bytes(bytes).map_err(|e| e.to_string())?;
    let mut msgs = vec![];
    for (_, m) in g.iter() {
        let pd = m.prod_def();
        let (Some(cat), Some(num), Some((s1, _))) = (
            pd.parameter_category(),
            pd.parameter_number(),
            pd.fixed_surfaces(),
        ) else {
            continue;
        };
        if m.indicator().discipline != 0 {
            continue;
        }
        // Template 4.8: unidade (índice 43) e duração (44..48) da janela estatística.
        let b: Vec<u8> = pd.iter().copied().collect();
        let (ini, dur) = match (pd.prod_tmpl_num(), pd.forecast_time()) {
            (8, Some(ft)) if b[43] == 1 => {
                (ft.value, u32::from_be_bytes([b[44], b[45], b[46], b[47]]))
            }
            _ => (0, 0),
        };
        let ll = m.latlons().map_err(|e| e.to_string())?;
        let dec = grib::Grib2SubmessageDecoder::from(m).map_err(|e| e.to_string())?;
        let vals = dec.dispatch().map_err(|e| e.to_string())?;
        let mut campo = vec![f32::NAN; N];
        for ((la, lo), v) in ll.zip(vals) {
            let lo = if lo > 180.0 { lo - 360.0 } else { lo };
            let (y, x) = (
                (LAT0 as f64 - la) / PASSO as f64,
                (lo - LON0 as f64) / PASSO as f64,
            );
            let (yr, xr) = (y.round(), x.round());
            if (y - yr).abs() < 1e-3 && (x - xr).abs() < 1e-3 && yr >= 0.0 && xr >= 0.0 {
                let (yr, xr) = (yr as usize, xr as usize);
                if yr < NY && xr < NX {
                    campo[yr * NX + xr] = v;
                }
            }
        }
        msgs.push(Msg {
            cat,
            num,
            sup: s1.surface_type,
            nivel: s1.value(),
            ini,
            dur,
            campo,
        });
    }
    Ok(msgs)
}

async fn decodifica_async(bytes: Vec<u8>) -> Result<Vec<Msg>, String> {
    tokio::task::spawn_blocking(move || decodifica(bytes))
        .await
        .map_err(|e| e.to_string())?
}

/// GET com 2 tentativas; `faixa` = (offset, tamanho) vira Range.
async fn baixa(http: &Client, url: &str, faixa: Option<(u64, u64)>) -> Result<Vec<u8>, String> {
    let mut erro = String::new();
    for _ in 0..2 {
        let mut rq = http.get(url).timeout(Duration::from_secs(90));
        if let Some((o, l)) = faixa {
            rq = rq.header("Range", format!("bytes={}-{}", o, o + l - 1));
        }
        let r = async { rq.send().await?.error_for_status()?.bytes().await }.await;
        match r {
            Ok(b) => return Ok(b.to_vec()),
            Err(e) => erro = e.to_string(),
        }
    }
    Err(format!("{url}: {erro}"))
}

/// Roda `f` para cada passo, no máximo 4 ao mesmo tempo.
async fn paralelo<T, Fut>(
    passos: impl Iterator<Item = u32>,
    f: impl Fn(u32) -> Fut,
) -> Result<Vec<(u32, T)>, String>
where
    T: Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send + 'static,
{
    let sem = Arc::new(Semaphore::new(4));
    let hs: Vec<_> = passos
        .map(|p| {
            let (sem, fut) = (sem.clone(), f(p));
            tokio::spawn(async move {
                let _g = sem.acquire().await.unwrap();
                fut.await.map(|t| (p, t))
            })
        })
        .collect();
    let mut out = vec![];
    for h in hs {
        out.push(h.await.map_err(|e| e.to_string())??);
    }
    Ok(out)
}

fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// AAAAMMDD de um instante unix (algoritmo civil_from_days).
fn ymd(t: i64) -> String {
    let z = t.div_euclid(86400) + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}{m:02}{d:02}", yoe + era * 400 + (m <= 2) as i64)
}

/// Rodadas de hoje e ontem nos `ciclos` (horas, do mais novo ao mais antigo) que já começaram.
fn rodadas(agora: i64, ciclos: &[i64]) -> Vec<i64> {
    let dia = agora - agora.rem_euclid(86400);
    [0, 1]
        .iter()
        .flat_map(|d| ciclos.iter().map(move |c| dia - d * 86400 + c * 3600))
        .filter(|r| *r <= agora)
        .collect()
}

fn em_cache(c: &Mutex<Option<Previsao>>, r: i64) -> Option<Previsao> {
    c.lock()
        .unwrap()
        .as_ref()
        .filter(|p| p.rodada == r)
        .cloned()
}

fn url_gfs(r: i64, f: u32) -> String {
    let (d, h) = (ymd(r), r.rem_euclid(86400) / 3600);
    format!(
        "https://nomads.ncep.noaa.gov/cgi-bin/filter_gfs_0p25.pl?dir=%2Fgfs.{d}%2F{h:02}%2Fatmos&file=gfs.t{h:02}z.pgrb2.0p25.f{f:03}&var_APCP=on&var_CAPE=on&var_TMP=on&var_PRMSL=on&var_GUST=on&var_UGRD=on&var_VGRD=on&lev_surface=on&lev_850_mb=on&lev_mean_sea_level=on&lev_10_m_above_ground=on&lev_2_m_above_ground=on&subregion=&toplat=6&leftlon=286&rightlon=326&bottomlat=-34"
    )
}

/// mm/h da janela [f-3, f] a partir dos acúmulos (mm) indexados por (início, fim) em horas.
/// O GFS acumula em baldes de 0-3, 0-6, 6-9, 6-12...: no fim de balde longo subtrai o acúmulo de f-3.
fn chuva_gfs(acum: &HashMap<(u32, u32), Vec<f32>>, f: u32) -> Option<Vec<f32>> {
    if f < 3 {
        return None;
    }
    if let Some(a) = acum.get(&(f - 3, f)) {
        return Some(a.iter().map(|x| x.max(0.0) / 3.0).collect());
    }
    acum.iter()
        .filter(|((i, fim), _)| *fim == f && *i < f - 3)
        .find_map(|((i, _), a)| acum.get(&(*i, f - 3)).map(|b| dif(a, b)))
}

/// (a - b) / 3, sem negativos de arredondamento.
fn dif(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter()
        .zip(b)
        .map(|(a, b)| ((a - b) / 3.0).max(0.0))
        .collect()
}

/// Aplica `f` ao campo da mensagem (cat, num, sup, nivel), se existir.
fn campo(
    ms: &[Msg],
    cat: u8,
    num: u8,
    sup: u8,
    nivel: f64,
    f: impl Fn(f32) -> f32,
) -> Option<Vec<f32>> {
    ms.iter()
        .find(|m| (m.cat, m.num, m.sup) == (cat, num, sup) && (m.nivel - nivel).abs() < 1e-6)
        .map(|m| m.campo.iter().map(|x| f(*x)).collect())
}

pub async fn gfs(http: &Client) -> Result<Previsao, String> {
    let mut erro = "sem rodada".to_string();
    for r in rodadas(agora(), &[18, 12, 6, 0]) {
        if let Some(p) = em_cache(&GFS, r) {
            return Ok(p);
        }
        // Existe se f072 existe (o filtro responde 200 com HTML quando falta).
        match baixa(http, &url_gfs(r, 72), None).await {
            Ok(b) if b.starts_with(b"GRIB") => {}
            Ok(_) => {
                erro = format!("rodada {} sem f072", ymd(r));
                continue;
            }
            Err(e) => {
                erro = e;
                continue;
            }
        }
        // ponytail: f072 é baixado duas vezes (teste de existência e passo), 450 KB/h.
        let h = http.clone();
        let msgs = paralelo(PASSOS.map(|p| p * 3), |f| {
            let h = h.clone();
            async move {
                let b = baixa(&h, &url_gfs(r, f), None).await?;
                decodifica_async(b).await
            }
        })
        .await?;
        let mut acum = HashMap::new();
        for (f, ms) in &msgs {
            for m in ms.iter().filter(|m| (m.cat, m.num) == (1, 8) && m.dur > 0) {
                acum.insert((m.ini, *f), m.campo.clone());
            }
        }
        let mut p = Previsao {
            fonte: "gfs",
            rodada: r,
            passos: Default::default(),
        };
        for (f, ms) in msgs {
            let mut c = Campos::vazio();
            let pega = |dst: &mut Vec<f32>, v: Option<Vec<f32>>| {
                if let Some(v) = v {
                    *dst = v;
                }
            };
            pega(&mut c.pmsl, campo(&ms, 3, 1, 101, 0.0, |x| x / 100.0));
            pega(&mut c.rajada, campo(&ms, 2, 22, 1, 0.0, |x| x * 3.6));
            pega(&mut c.t850, campo(&ms, 0, 0, 100, 85000.0, |x| x - 273.15));
            pega(&mut c.t2m, campo(&ms, 0, 0, 103, 2.0, |x| x - 273.15));
            pega(&mut c.u, campo(&ms, 2, 2, 103, 10.0, |x| x));
            pega(&mut c.v, campo(&ms, 2, 3, 103, 10.0, |x| x));
            pega(&mut c.cape, campo(&ms, 7, 6, 1, 0.0, |x| x));
            pega(&mut c.chuva, chuva_gfs(&acum, f));
            p.passos.insert(r + f as i64 * 3600, c);
        }
        *GFS.lock().unwrap() = Some(p.clone());
        return Ok(p);
    }
    Err(erro)
}

fn url_ecmwf(r: i64, passo: u32) -> String {
    let (d, h) = (ymd(r), r.rem_euclid(86400) / 3600);
    format!(
        "https://data.ecmwf.int/forecasts/{d}/{h:02}z/ifs/0p25/oper/{d}{h:02}0000-{passo}h-oper-fc.grib2"
    )
}

fn indice(url: &str) -> String {
    url.replace(".grib2", ".index")
}

/// Campos buscados no IFS: (param, levelist).
const IFS: [(&str, Option<&str>); 8] = [
    ("tp", None),
    ("mucape", None),
    ("2t", None),
    ("msl", None),
    ("10u", None),
    ("10v", None),
    ("10fg", None),
    ("t", Some("850")),
];

/// Um passo do IFS: campos e o tp acumulado (m), que fica de fora de Campos até virar mm/h.
async fn passo_ecmwf(
    http: Client,
    r: i64,
    passo: u32,
) -> Result<(Campos, Option<Vec<f32>>), String> {
    let url = url_ecmwf(r, passo);
    let idx =
        String::from_utf8(baixa(&http, &indice(&url), None).await?).map_err(|e| e.to_string())?;
    let linhas: Vec<Value> = idx
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let mut c = Campos::vazio();
    let mut tp = None;
    for (param, nivel) in IFS {
        let Some(l) = linhas
            .iter()
            .find(|l| l["param"] == param && l["levelist"].as_str() == nivel)
        else {
            continue; // 10fg e tp não existem no passo 0
        };
        let (o, n) = (
            l["_offset"].as_u64().ok_or("sem _offset")?,
            l["_length"].as_u64().ok_or("sem _length")?,
        );
        let b = baixa(&http, &url, Some((o, n))).await?;
        let Some(m) = decodifica_async(b).await?.into_iter().next() else {
            return Err(format!("{url}: {param} vazio"));
        };
        let v = |f: fn(f32) -> f32| m.campo.iter().map(|x| f(*x)).collect::<Vec<_>>();
        match param {
            "tp" => tp = Some(m.campo.clone()),
            "mucape" => c.cape = m.campo.clone(),
            "2t" => c.t2m = v(|x| x - 273.15),
            "msl" => c.pmsl = v(|x| x / 100.0),
            "10u" => c.u = m.campo.clone(),
            "10v" => c.v = m.campo.clone(),
            "10fg" => c.rajada = v(|x| x * 3.6),
            _ => c.t850 = v(|x| x - 273.15),
        }
    }
    Ok((c, tp))
}

pub async fn ecmwf(http: &Client) -> Result<Previsao, String> {
    let mut erro = "sem rodada".to_string();
    for r in rodadas(agora(), &[12, 0]) {
        if let Some(p) = em_cache(&ECMWF, r) {
            return Ok(p);
        }
        // Existe quando o índice do passo 72 responde 200.
        if let Err(e) = baixa(http, &indice(&url_ecmwf(r, 72)), None).await {
            erro = e;
            continue;
        }
        let h = http.clone();
        let mut passos = paralelo(PASSOS.map(|p| p * 3), |s| passo_ecmwf(h.clone(), r, s)).await?;
        passos.sort_by_key(|(s, _)| *s);
        let mut p = Previsao {
            fonte: "ecmwf",
            rodada: r,
            passos: Default::default(),
        };
        let mut ant: Option<Vec<f32>> = None;
        for (s, (mut c, tp)) in passos {
            if let Some(tp) = tp {
                if s >= 3 {
                    let zero = vec![0.0; N];
                    c.chuva = dif_tp(&tp, ant.as_deref().unwrap_or(&zero));
                }
                ant = Some(tp);
            }
            p.passos.insert(r + s as i64 * 3600, c);
        }
        *ECMWF.lock().unwrap() = Some(p.clone());
        return Ok(p);
    }
    Err(erro)
}

/// tp acumulado em m (desde a rodada) para mm/h na janela de 3 h.
fn dif_tp(tp: &[f32], ant: &[f32]) -> Vec<f32> {
    tp.iter()
        .zip(ant)
        .map(|(a, b)| ((a - b) * 1000.0 / 3.0).max(0.0))
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{QUADRO, celula};

    #[test]
    fn desacumula_gfs() {
        let mut a = HashMap::new();
        a.insert((0, 3), vec![3.0, 0.0]);
        a.insert((0, 6), vec![9.0, 0.0]);
        a.insert((6, 9), vec![6.0, f32::NAN]);
        a.insert((6, 12), vec![18.0, 0.0]);
        assert_eq!(chuva_gfs(&a, 0), None);
        assert_eq!(chuva_gfs(&a, 3).unwrap(), vec![1.0, 0.0]); // balde curto
        assert_eq!(chuva_gfs(&a, 6).unwrap(), vec![2.0, 0.0]); // (0-6) - (0-3)
        assert_eq!(chuva_gfs(&a, 9).unwrap()[0], 2.0);
        assert_eq!(chuva_gfs(&a, 12).unwrap(), vec![4.0, 0.0]); // (6-12) - (6-9)
        assert_eq!(chuva_gfs(&a, 15), None);
    }

    #[test]
    fn desacumula_ecmwf() {
        // tp em m: 0,003 em 3 h; 0,009 em 6 h => 1 mm/h e 2 mm/h; queda vira 0.
        assert_eq!(dif_tp(&[0.003], &[0.0]), vec![1.0]);
        let v = dif_tp(&[0.009, 0.001], &[0.003, 0.002]);
        assert!((v[0] - 2.0).abs() < 1e-4 && v[1] == 0.0);
    }

    #[test]
    fn datas() {
        assert_eq!(ymd(1_759_622_400), "20251005");
        assert_eq!(
            rodadas(1_759_622_400 + 13 * 3600, &[12, 0]),
            vec![
                1_759_622_400 + 12 * 3600,
                1_759_622_400,
                1_759_622_400 - 12 * 3600,
                1_759_622_400 - 86400
            ]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn rede() {
        let http = Client::new();
        for (nome, p) in [
            ("gfs", gfs(&http).await.unwrap()),
            ("ecmwf", ecmwf(&http).await.unwrap()),
        ] {
            let (t, c) = p.passos.iter().nth(8).unwrap();
            let i = |la, lo| celula(la, lo).unwrap();
            let (l, m, a) = (i(-25.34, -52.37), i(-3.1, -60.0), i(-30.0, -40.0));
            println!(
                "{nome} rodada={} passos={} t={t}\n  Laranjeiras: chuva={} cape={} rajada={} u={} v={} t2m={} t850={} pmsl={}\n  Manaus pmsl={} t2m={}\n  Atlantico pmsl={}",
                p.rodada,
                p.passos.len(),
                c.chuva[l],
                c.cape[l],
                c.rajada[l],
                c.u[l],
                c.v[l],
                c.t2m[l],
                c.t850[l],
                c.pmsl[l],
                c.pmsl[m],
                c.t2m[m],
                c.pmsl[a]
            );
            assert!(p.passos.len() == 25 && p.passos.keys().all(|t| t % QUADRO == 0));
            for x in [c.pmsl[l], c.pmsl[m], c.pmsl[a]] {
                assert!((1000.0..1030.0).contains(&x), "pmsl {x}");
            }
            assert!(c.t2m[m] > c.t2m[l]);
        }
    }
}
