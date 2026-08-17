//! Colori: da etichetta commerciale a valore di catalogo.
//!
//! I fornitori scrivono il colore in lingue diverse e con qualificatori di
//! finitura (`nero opaco`, `matt black`). Qui si riduce alla tinta di base, che
//! e' quella su cui il cliente filtra; la finitura, se serve, e' un altro campo.

use crate::model::{Color, Outcome};

/// Sinonimi accettati, in italiano, inglese e tedesco.
const ALIASES: &[(&str, Color)] = &[
    ("nero", Color::Nero),
    ("black", Color::Nero),
    ("schwarz", Color::Nero),
    ("bianco", Color::Bianco),
    ("white", Color::Bianco),
    ("weiss", Color::Bianco),
    ("grigio", Color::Grigio),
    ("grey", Color::Grigio),
    ("gray", Color::Grigio),
    ("antracite", Color::Grigio),
    ("beige", Color::Beige),
    ("sabbia", Color::Beige),
    ("crema", Color::Beige),
    ("marrone", Color::Marrone),
    ("brown", Color::Marrone),
    ("testa di moro", Color::Marrone),
    ("rosso", Color::Rosso),
    ("red", Color::Rosso),
    ("bordeaux", Color::Rosso),
    ("blu", Color::Blu),
    ("blue", Color::Blu),
    ("azzurro", Color::Blu),
    ("navy", Color::Blu),
    ("verde", Color::Verde),
    ("green", Color::Verde),
    ("giallo", Color::Giallo),
    ("yellow", Color::Giallo),
    ("rosa", Color::Rosa),
    ("pink", Color::Rosa),
    ("arancione", Color::Arancione),
    ("orange", Color::Arancione),
    ("viola", Color::Viola),
    ("purple", Color::Viola),
];

/// Qualificatori di finitura da ignorare prima del confronto.
const FINISHES: &[&str] = &[
    "opaco", "lucido", "satinato", "matt", "matte", "glossy", "metallizzato", "perlato",
];

pub fn parse(raw: &str) -> Outcome<Color> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Outcome::Absent;
    }

    let lower = trimmed.to_lowercase();

    // Corrispondenza esatta, che copre anche gli alias composti da piu' parole.
    if let Some((_, c)) = ALIASES.iter().find(|(a, _)| *a == lower) {
        return Outcome::Resolved(*c);
    }

    // Altrimenti si tolgono le finiture e si cercano i termini rimasti.
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| !w.is_empty() && !FINISHES.contains(w))
        .collect();

    let found: Vec<Color> = words
        .iter()
        .filter_map(|w| ALIASES.iter().find(|(a, _)| a == w).map(|(_, c)| *c))
        .collect();

    match found.len() {
        0 => Outcome::ambiguous(trimmed, "colore non presente nella tavolozza di catalogo"),
        1 => Outcome::Resolved(found[0]),
        _ if found.windows(2).all(|w| w[0] == w[1]) => Outcome::Resolved(found[0]),
        _ => Outcome::ambiguous(
            trimmed,
            "piu' colori nella stessa etichetta: serve una decisione, non una scelta automatica",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(raw: &str) -> Color {
        match parse(raw) {
            Outcome::Resolved(c) => c,
            other => panic!("atteso Resolved per {raw:?}, ottenuto {other:?}"),
        }
    }

    #[test]
    fn riconosce_le_tre_lingue() {
        assert_eq!(resolved("nero"), Color::Nero);
        assert_eq!(resolved("Black"), Color::Nero);
        assert_eq!(resolved("SCHWARZ"), Color::Nero);
    }

    #[test]
    fn ignora_la_finitura() {
        assert_eq!(resolved("nero opaco"), Color::Nero);
        assert_eq!(resolved("matt black"), Color::Nero);
    }

    #[test]
    fn riconosce_gli_alias_commerciali() {
        assert_eq!(resolved("antracite"), Color::Grigio);
        assert_eq!(resolved("testa di moro"), Color::Marrone);
        assert_eq!(resolved("bordeaux"), Color::Rosso);
    }

    #[test]
    fn il_campo_vuoto_non_e_un_errore() {
        assert_eq!(parse(""), Outcome::Absent);
    }

    #[test]
    fn segnala_il_colore_fuori_tavolozza() {
        assert!(parse("turchese").is_ambiguous());
    }

    #[test]
    fn segnala_le_etichette_bicolore() {
        // "nero / bianco" e' una decisione di catalogo, non un parsing.
        assert!(parse("nero / bianco").is_ambiguous());
    }

    #[test]
    fn accetta_la_ripetizione_dello_stesso_colore() {
        assert_eq!(resolved("nero nero opaco"), Color::Nero);
    }
}
