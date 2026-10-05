//! Voto ponderado entre as fontes e campos derivados.
use crate::modelo::{Consenso, Previsao};

/// Instantes válidos (múltiplos de QUADRO) de agora até HORIZONTE cobertos por alguma fonte.
pub fn tempos(_prev: &[&Previsao], _agora: i64) -> Vec<i64> {
    Vec::new()
}

pub fn calcular(_prev: &[&Previsao], _t: i64) -> Consenso {
    unimplemented!()
}
