//! Voto ponderado entre as fontes e campos derivados.
use crate::modelo::{Campos, Consenso, HORIZONTE, N, NX, NY, PASSADO, Previsao, QUADRO, lat};

const LIMIAR_CHUVA: f32 = 0.2;
const CHUVA_RISCO: f32 = 1.0;
const CAPE_RISCO: [f32; 3] = [800.0, 1500.0, 2500.0];
const RAJADA_RISCO: f32 = 60.0;
// ponytail: calibrável; vento a 850 hPa seria melhor que o de 10 m para a advecção.
const LIMIAR_FRENTE: f32 = 1.0;
const KM_GRAU: f32 = 111.0;

/// Instantes válidos (múltiplos de QUADRO) de agora-PASSADO até agora+HORIZONTE cobertos por alguma fonte.
pub fn tempos(prev: &[&Previsao], agora: i64) -> Vec<i64> {
    let fim = agora + HORIZONTE;
    ((agora - PASSADO).div_euclid(QUADRO)..)
        .map(|k| k * QUADRO)
        .take_while(|&t| t <= fim)
        .filter(|t| prev.iter().any(|p| p.passos.contains_key(t)))
        .collect()
}

/// Mediana dos valores finitos; NaN se não houver nenhum.
fn mediana(mut v: Vec<f32>) -> f32 {
    v.retain(|x| x.is_finite());
    v.sort_by(f32::total_cmp);
    match v.len() {
        0 => f32::NAN,
        n if n % 2 == 1 => v[n / 2],
        n => (v[n / 2 - 1] + v[n / 2]) / 2.0,
    }
}

/// Média 3x3 que ignora NaN.
fn suavizar(c: &[f32]) -> Vec<f32> {
    (0..N)
        .map(|i| {
            let (y, x) = (i / NX, i % NX);
            let (mut s, mut n) = (0.0, 0);
            for yy in y.saturating_sub(1)..=(y + 1).min(NY - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(NX - 1) {
                    let a = c[yy * NX + xx];
                    if a.is_finite() {
                        s += a;
                        n += 1;
                    }
                }
            }
            if n == 0 { f32::NAN } else { s / n as f32 }
        })
        .collect()
}

pub fn calcular(prev: &[&Previsao], t: i64) -> Consenso {
    let passos: Vec<(f32, &Campos)> = prev
        .iter()
        .filter_map(|p| {
            let peso = if p.fonte == "ecmwf" { 1.25 } else { 1.0 };
            p.passos.get(&t).map(|c| (peso, c))
        })
        .collect();
    let med = |f: fn(&Campos) -> &Vec<f32>, i: usize| {
        mediana(passos.iter().map(|(_, c)| f(c)[i]).collect())
    };

    let mut c = Consenso {
        tempo: t,
        chuva: vec![0.0; N],
        votos_chuva: vec![0; N],
        fontes: vec![0; N],
        cape: vec![f32::NAN; N],
        rajada: vec![f32::NAN; N],
        u: vec![f32::NAN; N],
        v: vec![f32::NAN; N],
        t2m: vec![f32::NAN; N],
        t850: vec![f32::NAN; N],
        anom850: vec![f32::NAN; N],
        pmsl: vec![f32::NAN; N],
        frente: vec![0; N],
        risco: vec![0; N],
    };
    for i in 0..N {
        let (mut total, mut pro, mut valores) = (0.0, 0.0, Vec::new());
        for (peso, cp) in &passos {
            let ch = cp.chuva[i];
            if !ch.is_finite() {
                continue;
            }
            c.fontes[i] += 1;
            total += peso;
            if ch >= LIMIAR_CHUVA {
                c.votos_chuva[i] += 1;
                pro += peso;
                valores.push(ch);
            }
        }
        if pro > total / 2.0 {
            c.chuva[i] = mediana(valores);
        }
        c.cape[i] = med(|c| &c.cape, i);
        c.rajada[i] = med(|c| &c.rajada, i);
        c.u[i] = med(|c| &c.u, i);
        c.v[i] = med(|c| &c.v, i);
        c.t2m[i] = med(|c| &c.t2m, i);
        c.t850[i] = med(|c| &c.t850, i);
        c.pmsl[i] = med(|c| &c.pmsl, i);
    }

    // ponytail: a média zonal da própria janela aproxima a climatologia; trocar por normal real se houver.
    for y in 0..NY {
        let linha = &c.t850[y * NX..(y + 1) * NX];
        let fin: Vec<f32> = linha.iter().copied().filter(|x| x.is_finite()).collect();
        let media = fin.iter().sum::<f32>() / fin.len() as f32;
        for x in 0..NX {
            c.anom850[y * NX + x] = c.t850[y * NX + x] - media;
        }
    }

    // Gradiente em °C/100 km por diferença central; y cresce para o sul. Bordas ficam 0.
    let ts = suavizar(&c.t850);
    let mut g = vec![(0.0f32, 0.0f32); N];
    for y in 1..NY - 1 {
        let dy = 0.5 * KM_GRAU;
        let dx = dy * lat(y).to_radians().cos();
        for x in 1..NX - 1 {
            let i = y * NX + x;
            g[i] = (
                (ts[i + 1] - ts[i - 1]) / (2.0 * dx) * 100.0,
                (ts[i - NX] - ts[i + NX]) / (2.0 * dy) * 100.0,
            );
        }
    }
    let m = |i: usize| g[i].0.hypot(g[i].1);
    for y in 2..NY - 2 {
        for x in 2..NX - 2 {
            let i = y * NX + x;
            let (gx, gy) = g[i];
            // ponytail: t2m < t850 marca relevo acima de 850 hPa (Andes), onde o t850 é extrapolado; troca por pressão de superfície se o falso positivo incomodar.
            if m(i) <= LIMIAR_FRENTE || c.t2m[i] < c.t850[i] {
                continue;
            }
            // Só o máximo do gradiente ao longo da própria direção: a zona vira linha.
            let (sx, sy) = ((gx / m(i)).round() as isize, (-gy / m(i)).round() as isize);
            let d = sy * NX as isize + sx;
            if m(i) < m((i as isize + d) as usize) || m(i) < m((i as isize - d) as usize) {
                continue;
            }
            let adv = -(c.u[i] * gx + c.v[i] * gy);
            c.frente[i] = (adv > 0.0) as i8 - (adv < 0.0) as i8;
        }
    }

    for i in 0..N {
        if c.chuva[i] >= CHUVA_RISCO {
            let n = CAPE_RISCO.iter().filter(|&&l| c.cape[i] >= l).count()
                + (c.rajada[i] >= RAJADA_RISCO) as usize;
            c.risco[i] = n.min(3) as u8;
        }
    }
    c
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::collections::BTreeMap;

    const T: i64 = 10 * QUADRO;
    const I: usize = 40 * NX + 40;

    fn prev(fonte: &'static str, f: impl Fn(&mut Campos)) -> Previsao {
        let mut c = Campos::vazio();
        f(&mut c);
        Previsao {
            fonte,
            rodada: 0,
            passos: BTreeMap::from([(T, c)]),
        }
    }

    /// Cinco fontes com `chuva` na célula I, na ordem ecmwf, gfs, icon, ukmo, cma.
    fn cinco(chuva: [f32; 5]) -> Vec<Previsao> {
        ["ecmwf", "gfs", "icon", "ukmo", "cma"]
            .into_iter()
            .zip(chuva)
            .map(|(f, ch)| prev(f, |c| c.chuva[I] = ch))
            .collect()
    }

    fn calc(p: &[Previsao]) -> Consenso {
        calcular(&p.iter().collect::<Vec<_>>(), T)
    }

    #[test]
    fn tres_de_cinco_votam_chuva() {
        let c = calc(&cinco([1.0, 2.0, 5.0, 0.0, 0.0]));
        assert_eq!((c.chuva[I], c.votos_chuva[I], c.fontes[I]), (2.0, 3, 5));
    }

    #[test]
    fn dois_de_cinco_nao_chove() {
        let c = calc(&cinco([1.0, 2.0, 0.0, 0.0, 0.0]));
        assert_eq!((c.chuva[I], c.votos_chuva[I]), (0.0, 2));
    }

    #[test]
    fn peso_do_ecmwf_desempata() {
        // ecmwf + gfs votam, icon e ukmo não, cma sem dado: 2,25 > 4,25/2
        let c = calc(&cinco([1.0, 1.0, 0.0, 0.0, f32::NAN]));
        assert_eq!(c.chuva[I], 1.0);
        let c = calc(&cinco([0.0, 1.0, 1.0, 0.0, 0.0]));
        assert_eq!(c.chuva[I], 0.0);
    }

    #[test]
    fn nan_tira_a_fonte() {
        let c = calc(&cinco([1.0, 1.0, 1.0, 1.0, f32::NAN]));
        assert_eq!(c.fontes[I], 4);
    }

    #[test]
    fn frente_fria_no_gradiente() {
        let p = prev("gfs", |c| {
            for i in 0..N {
                c.t850[i] = 2.0 * (i % NX) as f32;
                c.u[i] = 5.0;
                c.v[i] = 0.0;
            }
        });
        let c = calcular(&[&p], T);
        assert_eq!(c.frente[I], -1);
        assert_eq!(c.frente[40 * NX], 0);
        // Relevo acima de 850 hPa: t2m mais fria que o t850 extrapolado não marca frente.
        let p = prev("gfs", |c| {
            for i in 0..N {
                c.t850[i] = 2.0 * (i % NX) as f32;
                c.t2m[i] = c.t850[i] - 1.0;
                c.u[i] = 5.0;
            }
        });
        assert_eq!(calcular(&[&p], T).frente[I], 0);
    }

    #[test]
    fn risco_por_cape_e_rajada() {
        let p = prev("gfs", |c| {
            c.chuva[I] = 3.0;
            c.cape[I] = 1600.0;
            c.rajada[I] = 70.0;
        });
        assert_eq!(calcular(&[&p], T).risco[I], 3);
    }

    #[test]
    fn tempos_filtra_janela() {
        let p = Previsao {
            fonte: "gfs",
            rodada: 0,
            passos: [0, T, 100 * QUADRO].map(|t| (t, Campos::vazio())).into(),
        };
        let agora = T + 100;
        assert_eq!(tempos(&[&p], agora), vec![0, T]);
        assert_eq!(tempos(&[&p], agora - 20 * QUADRO), vec![0, T]);
        // Passado entra até PASSADO atrás; o futuro corta em HORIZONTE.
        assert_eq!(tempos(&[&p], PASSADO + QUADRO), vec![T]);
        assert_eq!(tempos(&[&p], T + PASSADO + QUADRO - 1), vec![T]);
        assert!(tempos(&[&p], T + PASSADO + QUADRO).is_empty());
    }
}
