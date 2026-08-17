use serde::{Deserialize, Serialize};
use std::fmt;

/// Esito dell'applicazione di una regola a un singolo campo.
///
/// La distinzione fra `Absent` e `Ambiguous` e' il cuore del progetto: un campo
/// vuoto all'origine non e' un errore e non va segnalato, mentre un campo pieno
/// che le regole non sanno leggere e' esattamente cio' che merita attenzione —
/// vada al modello o finisca nel log degli scarti.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome<T> {
    Resolved(T),
    Absent,
    Ambiguous { raw: String, reason: String },
}

impl<T> Outcome<T> {
    /// Costruisce un esito ambiguo, con la motivazione che finira' nel log.
    pub fn ambiguous(raw: &str, reason: impl Into<String>) -> Self {
        Outcome::Ambiguous {
            raw: raw.trim().to_string(),
            reason: reason.into(),
        }
    }

    pub fn resolved(&self) -> Option<&T> {
        match self {
            Outcome::Resolved(v) => Some(v),
            _ => None,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Outcome::Ambiguous { .. })
    }
}

/// Riga come arriva dalla fonte: tutto stringa, nessuna garanzia.
#[derive(Debug, Clone, Deserialize)]
pub struct RawProduct {
    pub sku: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub brand: String,
    #[serde(default)]
    pub color: String,
    #[serde(default)]
    pub size: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub ean: String,
}

/// Riga dopo la normalizzazione. Ogni campo opzionale e' un campo che le regole
/// non hanno potuto riempire: l'assenza e' esplicita, non un valore di comodo.
#[derive(Debug, Clone, Serialize)]
pub struct NormalizedProduct {
    pub sku: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions_mm: Option<Dimensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ean13: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Color {
    Nero,
    Bianco,
    Grigio,
    Beige,
    Marrone,
    Rosso,
    Blu,
    Verde,
    Giallo,
    Rosa,
    Arancione,
    Viola,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Color::Nero => "nero",
            Color::Bianco => "bianco",
            Color::Grigio => "grigio",
            Color::Beige => "beige",
            Color::Marrone => "marrone",
            Color::Rosso => "rosso",
            Color::Blu => "blu",
            Color::Verde => "verde",
            Color::Giallo => "giallo",
            Color::Rosa => "rosa",
            Color::Arancione => "arancione",
            Color::Viola => "viola",
        };
        f.write_str(s)
    }
}

impl Color {
    /// Riconosce il colore da un'etichetta gia' normalizzata (minuscola, senza
    /// spazi ai bordi). Usata sia dalle regole sia per validare la risposta del
    /// modello, cosi' che il modello non possa introdurre colori inventati.
    pub fn from_canonical(s: &str) -> Option<Color> {
        Some(match s {
            "nero" => Color::Nero,
            "bianco" => Color::Bianco,
            "grigio" => Color::Grigio,
            "beige" => Color::Beige,
            "marrone" => Color::Marrone,
            "rosso" => Color::Rosso,
            "blu" => Color::Blu,
            "verde" => Color::Verde,
            "giallo" => Color::Giallo,
            "rosa" => Color::Rosa,
            "arancione" => Color::Arancione,
            "viola" => Color::Viola,
            _ => return None,
        })
    }

    pub fn all() -> &'static [&'static str] {
        &[
            "nero",
            "bianco",
            "grigio",
            "beige",
            "marrone",
            "rosso",
            "blu",
            "verde",
            "giallo",
            "rosa",
            "arancione",
            "viola",
        ]
    }
}

/// Dimensioni sempre in millimetri: l'unita' viene decisa una volta, in ingresso.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Dimensions {
    pub length_mm: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width_mm: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness_mm: Option<f64>,
}

/// Prezzo in centesimi interi: mai in virgola mobile, per non accumulare errori
/// su un catalogo di centinaia di migliaia di righe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Money {
    pub cents: i64,
    pub currency: Currency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Currency {
    Eur,
    Usd,
    Gbp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outcome_distingue_assente_da_ambiguo() {
        let assente: Outcome<u8> = Outcome::Absent;
        let ambiguo: Outcome<u8> = Outcome::ambiguous("boh", "non interpretabile");
        assert!(!assente.is_ambiguous());
        assert!(ambiguo.is_ambiguous());
    }

    #[test]
    fn ambiguous_ripulisce_gli_spazi_del_valore_grezzo() {
        let o: Outcome<u8> = Outcome::ambiguous("  42 unita'  ", "motivo");
        match o {
            Outcome::Ambiguous { raw, .. } => assert_eq!(raw, "42 unita'"),
            _ => panic!("atteso Ambiguous"),
        }
    }

    #[test]
    fn i_colori_canonici_coprono_tutta_la_enum() {
        for name in Color::all() {
            assert!(Color::from_canonical(name).is_some(), "{name} non riconosciuto");
        }
        assert!(Color::from_canonical("turchese").is_none());
    }
}
