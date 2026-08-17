//! Dimensioni: da testo libero a millimetri.
//!
//! Ogni fornitore scrive le misure a modo suo — `120x60x2 cm`, `1200 mm`,
//! `47in`, `2,5 m`. L'unita' viene decisa una volta sola, in ingresso, e da li'
//! in poi nel catalogo esistono solo millimetri.

use super::numeric::parse_decimal;
use crate::model::{Dimensions, Outcome};

/// Fattori di conversione verso il millimetro. L'ordine conta: i suffissi piu'
/// lunghi vanno provati per primi, altrimenti `mm` verrebbe letto come `m`.
const UNITS: &[(&str, f64)] = &[
    ("mm", 1.0),
    ("cm", 10.0),
    ("in", 25.4),
    ("\"", 25.4),
    ("m", 1000.0),
];

pub fn parse(raw: &str) -> Outcome<Dimensions> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Outcome::Absent;
    }

    let lower = trimmed.to_lowercase();

    let Some((unit, factor)) = UNITS.iter().find(|(u, _)| lower.ends_with(u)) else {
        return Outcome::ambiguous(
            trimmed,
            "unita' di misura assente o non riconosciuta (attese: mm, cm, m, in)",
        );
    };

    let body = lower[..lower.len() - unit.len()].trim();
    if body.is_empty() {
        return Outcome::ambiguous(trimmed, "unita' di misura senza valore numerico");
    }

    // `x`, `×` e `*` sono tutti separatori di dimensione nella pratica reale.
    let parts: Vec<&str> = body
        .split(['x', '×', '*'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() || parts.len() > 3 {
        return Outcome::ambiguous(
            trimmed,
            "attese da una a tre dimensioni separate da x",
        );
    }

    let mut values = Vec::with_capacity(parts.len());
    for part in &parts {
        match parse_decimal(part) {
            Ok(v) if v > 0.0 => values.push(v * factor),
            Ok(_) => return Outcome::ambiguous(trimmed, "dimensione nulla o negativa"),
            Err(e) => return Outcome::ambiguous(trimmed, e.reason()),
        }
    }

    Outcome::Resolved(Dimensions {
        length_mm: values[0],
        width_mm: values.get(1).copied(),
        thickness_mm: values.get(2).copied(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(raw: &str) -> Dimensions {
        match parse(raw) {
            Outcome::Resolved(d) => d,
            other => panic!("atteso Resolved per {raw:?}, ottenuto {other:?}"),
        }
    }

    #[test]
    fn converte_i_centimetri_in_millimetri() {
        let d = resolved("120x60x2 cm");
        assert_eq!(d.length_mm, 1200.0);
        assert_eq!(d.width_mm, Some(600.0));
        assert_eq!(d.thickness_mm, Some(20.0));
    }

    #[test]
    fn accetta_una_dimensione_sola() {
        let d = resolved("1200 mm");
        assert_eq!(d.length_mm, 1200.0);
        assert_eq!(d.width_mm, None);
    }

    #[test]
    fn converte_i_pollici() {
        assert_eq!(resolved("47in").length_mm, 47.0 * 25.4);
        assert_eq!(resolved("10\"").length_mm, 254.0);
    }

    #[test]
    fn accetta_il_separatore_unicode() {
        let d = resolved("120 × 60 cm");
        assert_eq!(d.width_mm, Some(600.0));
    }

    #[test]
    fn legge_i_decimali_con_la_virgola() {
        assert_eq!(resolved("2,5 m").length_mm, 2500.0);
    }

    #[test]
    fn il_campo_vuoto_non_e_un_errore() {
        assert_eq!(parse("   "), Outcome::Absent);
    }

    #[test]
    fn segnala_l_unita_mancante() {
        let o = parse("120x60");
        assert!(o.is_ambiguous(), "senza unita' non si puo' normalizzare");
    }

    #[test]
    fn segnala_il_testo_non_dimensionale() {
        assert!(parse("taglia L").is_ambiguous());
    }

    #[test]
    fn segnala_troppe_dimensioni() {
        assert!(parse("1x2x3x4 cm").is_ambiguous());
    }
}
