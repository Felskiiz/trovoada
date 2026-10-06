//! Raios do GLM (GOES-19), últimos 30 min.
use netcdf_reader::NcFile;
use reqwest::Client;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex as TMutex, Semaphore};
use tokio::task::JoinSet;

const JANELA: i64 = 1800;
const BASE: &str = "https://noaa-goes19.s3.amazonaws.com";

#[derive(Default)]
pub struct Raios {
    /// início do arquivo (epoch s) -> flashes [lat, lon] na caixa. Vazio também conta como visto.
    arquivos: Mutex<BTreeMap<i64, Vec<[f32; 2]>>>,
    /// Serializa `atualizar`: o segundo chamador espera e já encontra tudo baixado.
    trava: TMutex<()>,
    baixados: AtomicUsize,
}

fn agora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

fn bissexto(a: i64) -> bool {
    a % 4 == 0 && (a % 100 != 0 || a % 400 == 0)
}

fn dias_no_ano(a: i64) -> i64 {
    if bissexto(a) { 366 } else { 365 }
}

/// (ano, dia do ano a partir de 1, hora) em UTC.
fn ano_doy_hora(t: i64) -> (i64, i64, i64) {
    let (mut dias, hora) = (t.div_euclid(86400), t.rem_euclid(86400) / 3600);
    let mut a = 1970;
    while dias >= dias_no_ano(a) {
        dias -= dias_no_ano(a);
        a += 1;
    }
    (a, dias + 1, hora)
}

/// Início do arquivo, do trecho `_sYYYYDDDHHMMSSt_` do nome (t = décimos, ignorado).
fn inicio(chave: &str) -> Option<i64> {
    let s = chave.split("_s").nth(1)?.get(..14)?;
    let n = |i: usize, j: usize| s.get(i..j)?.parse::<i64>().ok();
    let (a, d) = (n(0, 4)?, n(4, 7)?);
    let dias: i64 = (1970..a).map(dias_no_ano).sum::<i64>() + d - 1;
    Some(dias * 86400 + n(7, 9)? * 3600 + n(9, 11)? * 60 + n(11, 13)?)
}

async fn chaves(http: &Client, t: i64) -> Result<Vec<String>, String> {
    let (a, d, h) = ano_doy_hora(t);
    let url = format!("{BASE}/?list-type=2&prefix=GLM-L2-LCFA/{a}/{d:03}/{h:02}/");
    let xml = http
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let xml = xml.text().await.map_err(|e| e.to_string())?;
    // ponytail: sem paginação; uma hora tem ~180 chaves, abaixo do limite de 1000 do S3
    Ok(xml
        .split("<Key>")
        .skip(1)
        .filter_map(|p| Some(p.split("</Key>").next()?.to_string()))
        .collect())
}

fn na_caixa(bytes: &[u8]) -> Result<Vec<[f32; 2]>, String> {
    let f = NcFile::from_bytes(bytes).map_err(|e| e.to_string())?;
    let ler = |n: &str| f.read_variable_unpacked(n).map_err(|e| e.to_string());
    let (lat, lon) = (ler("flash_lat")?, ler("flash_lon")?);
    Ok(lat
        .iter()
        .zip(lon.iter())
        .filter(|(la, lo)| (-58.0..=14.0).contains(*la) && (-110.0..=0.0).contains(*lo))
        .map(|(la, lo)| [*la as f32, *lo as f32])
        .collect())
}

impl Raios {
    pub async fn atualizar(&self, http: &Client) -> Result<(), String> {
        let _g = self.trava.lock().await;
        let t = agora();
        let corte = t - JANELA;
        let mut novos = Vec::new();
        for h in [t - 3600, t] {
            for k in chaves(http, h).await? {
                let Some(i) = inicio(&k) else { continue };
                if i >= corte && !self.arquivos.lock().unwrap().contains_key(&i) {
                    novos.push((i, k));
                }
            }
        }
        let sem = Arc::new(Semaphore::new(8));
        let mut tarefas = JoinSet::new();
        for (i, k) in novos {
            let (http, sem) = (http.clone(), sem.clone());
            tarefas.spawn(async move {
                let _p = sem.acquire().await;
                let r = async {
                    let r = http
                        .get(format!("{BASE}/{k}"))
                        .send()
                        .await
                        .and_then(|r| r.error_for_status());
                    r.map_err(|e| e.to_string())?
                        .bytes()
                        .await
                        .map_err(|e| e.to_string())
                }
                .await;
                (i, k, r.map(|b| na_caixa(&b)))
            });
        }
        let mut erro = None;
        while let Some(r) = tarefas.join_next().await {
            let (i, k, r) = r.map_err(|e| e.to_string())?;
            self.baixados.fetch_add(1, Ordering::Relaxed);
            match r {
                Ok(Ok(v)) => drop(self.arquivos.lock().unwrap().insert(i, v)),
                // arquivo ilegível não melhora ao baixar de novo: marca como visto
                Ok(Err(e)) => {
                    self.arquivos.lock().unwrap().insert(i, Vec::new());
                    erro = Some(format!("{k}: {e}"));
                }
                // falha de rede: não marca, a próxima rodada tenta de novo
                Err(e) => erro = Some(format!("{k}: {e}")),
            }
        }
        self.arquivos.lock().unwrap().retain(|i, _| *i >= corte);
        erro.map_or(Ok(()), Err)
    }

    /// [lat, lon, idade em s].
    pub fn lista(&self) -> Vec<[f32; 3]> {
        let t = agora();
        let m = self.arquivos.lock().unwrap();
        m.range(t - JANELA..)
            .flat_map(|(i, v)| v.iter().map(move |f| [f[0], f[1], (t - i) as f32]))
            .collect()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_e_data() {
        let k = "GLM-L2-LCFA/2026/278/22/OR_GLM-L2-LCFA_G19_s20262782243200_e20262782243400_c1.nc";
        // 2026-10-05 22:43:20 UTC
        assert_eq!(inicio(k), Some(1791240200));
        assert_eq!(ano_doy_hora(1791240200), (2026, 278, 22));
        assert_eq!(ano_doy_hora(951782400), (2000, 60, 0)); // 29/fev/2000
    }

    #[tokio::test]
    #[ignore]
    async fn rede() {
        let (r, http) = (Raios::default(), Client::new());
        for n in 1..=2 {
            let t0 = std::time::Instant::now();
            let antes = r.baixados.load(Ordering::Relaxed);
            r.atualizar(&http).await.unwrap();
            let l = r.lista();
            println!(
                "chamada {n}: baixou {} arquivos, {} flashes na caixa, exemplos {:?}, {:?}",
                r.baixados.load(Ordering::Relaxed) - antes,
                l.len(),
                l.iter()
                    .step_by((l.len() / 3).max(1))
                    .take(3)
                    .collect::<Vec<_>>(),
                t0.elapsed()
            );
        }
    }
}
