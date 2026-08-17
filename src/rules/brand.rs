//! Marche: da ragione sociale a nome commerciale.
//!
//! La stessa marca arriva come `ACME S.r.l.`, `acme srl`, `Acme  Group`. Senza
//! normalizzazione diventano tre marche distinte nei filtri del negozio, che e'
//! uno dei modi piu' rapidi di rendere inutile una navigazione a faccette.

use crate::model::Outcome;

/// Forme giuridiche e suffissi societari da rimuovere dal nome commerciale.
const LEGAL_SUFFIXES: &[&str] = &[
    "srl", "s r l", "spa", "s p a", "snc", "s n c", "sas", "s a s", "gmbh", "ltd", "inc",
    "llc", "bv", "nv", "sa", "ag", "group", "italia",
];

/// Marche con una grafia ufficiale che non si ottiene per regola.
const CANONICAL: &[(&str, &str)] = &[
    ("3m", "3M"),
    ("bosch", "Bosch"),
    ("ikea", "IKEA"),
    ("hp", "HP"),
    ("lg", "LG"),
];

pub fn parse(raw: &str) -> Outcome<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Outcome::Absent;
    }

    // I punti separano le sigle (S.r.l.), quindi diventano spazi prima del taglio.
    let flattened: String = trimmed
        .chars()
        .map(|c| if c == '.' || c == ',' || c == '-' { ' ' } else { c })
        .collect();

    let words: Vec<String> = flattened
        .split_whitespace()
        .map(|w| w.to_lowercase())
        .collect();

    // Il suffisso si toglie solo in coda: "Group" a inizio nome fa parte del marchio.
    let mut kept = words.clone();
    while let Some(last) = kept.last() {
        if LEGAL_SUFFIXES.contains(&last.as_str()) && kept.len() > 1 {
            kept.pop();
        } else {
            break;
        }
    }

    // "s r l" si riduce a tre parole separate: si ripulisce anche quel caso.
    while kept.len() > 1 {
        let tail = kept[kept.len().saturating_sub(3)..].join(" ");
        if LEGAL_SUFFIXES.contains(&tail.as_str()) {
            let n = kept.len() - 3;
            kept.truncate(n);
        } else {
            break;
        }
    }

    if kept.is_empty() {
        return Outcome::ambiguous(trimmed, "rimane solo la forma giuridica, nessun nome commerciale");
    }

    let joined = kept.join(" ");

    if let Some((_, canonical)) = CANONICAL.iter().find(|(k, _)| *k == joined) {
        return Outcome::Resolved((*canonical).to_string());
    }

    // Iniziale maiuscola per parola: convenzione unica per tutto il catalogo.
    let titled = kept
        .iter()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    Outcome::Resolved(titled)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(raw: &str) -> String {
        match parse(raw) {
            Outcome::Resolved(s) => s,
            other => panic!("atteso Resolved per {raw:?}, ottenuto {other:?}"),
        }
    }

    #[test]
    fn unifica_le_grafie_della_stessa_marca() {
        assert_eq!(resolved("ACME S.r.l."), "Acme");
        assert_eq!(resolved("acme srl"), "Acme");
        assert_eq!(resolved("Acme  Group"), "Acme");
    }

    #[test]
    fn rispetta_le_grafie_ufficiali() {
        assert_eq!(resolved("3m italia srl"), "3M");
        assert_eq!(resolved("IKEA"), "IKEA");
    }

    #[test]
    fn non_taglia_le_parole_iniziali() {
        // "Group" in testa fa parte del marchio, non e' un suffisso societario.
        assert_eq!(resolved("Group Lotus"), "Group Lotus");
    }

    #[test]
    fn conserva_i_nomi_composti() {
        assert_eq!(resolved("van der berg"), "Van Der Berg");
    }

    #[test]
    fn il_campo_vuoto_non_e_un_errore() {
        assert_eq!(parse("  "), Outcome::Absent);
    }
}
