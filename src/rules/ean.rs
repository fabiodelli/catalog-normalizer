//! Codici EAN: validazione con cifra di controllo.
//!
//! Un EAN sbagliato e' peggio di un EAN assente: fa fallire l'aggancio a monte
//! (marketplace, magazzino) in un punto lontano da dove e' stato introdotto.
//! Qui si valida la cifra di controllo invece di limitarsi a contare i caratteri.

use crate::model::Outcome;

/// Calcola la cifra di controllo EAN-13 dalle prime dodici cifre.
///
/// Pesi alternati 1 e 3 da sinistra, somma, complemento a dieci.
fn check_digit(first_twelve: &[u32]) -> u32 {
    let sum: u32 = first_twelve
        .iter()
        .enumerate()
        .map(|(i, d)| if i % 2 == 0 { *d } else { d * 3 })
        .sum();
    (10 - (sum % 10)) % 10
}

pub fn parse(raw: &str) -> Outcome<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Outcome::Absent;
    }

    // Trattini e spazi sono decorazioni tipografiche, non parte del codice.
    let digits: Vec<u32> = trimmed
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .filter_map(|c| c.to_digit(10))
        .collect();

    let non_numeric = trimmed
        .chars()
        .any(|c| !c.is_ascii_digit() && !c.is_whitespace() && c != '-');

    if non_numeric {
        return Outcome::ambiguous(trimmed, "il codice contiene caratteri non numerici");
    }

    match digits.len() {
        13 => {
            let expected = check_digit(&digits[..12]);
            if expected == digits[12] {
                Outcome::Resolved(digits.iter().map(|d| d.to_string()).collect())
            } else {
                Outcome::ambiguous(
                    trimmed,
                    "cifra di controllo EAN-13 non valida: il codice e' corrotto o trascritto male",
                )
            }
        }
        12 => {
            // Dodici cifre sono un EAN-13 a cui manca il controllo: si completa.
            let mut full = digits.clone();
            full.push(check_digit(&digits));
            Outcome::Resolved(full.iter().map(|d| d.to_string()).collect())
        }
        8 => Outcome::ambiguous(
            trimmed,
            "EAN-8: la conversione a EAN-13 richiede il prefisso aziendale, non deducibile dal codice",
        ),
        n => Outcome::ambiguous(trimmed, format!("{n} cifre: attese 8, 12 o 13")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accetta_un_ean13_valido() {
        // 4006381333931 e' un EAN-13 con cifra di controllo corretta.
        assert_eq!(
            parse("4006381333931"),
            Outcome::Resolved("4006381333931".to_string())
        );
    }

    #[test]
    fn ignora_trattini_e_spazi() {
        assert_eq!(
            parse("400-6381 333931"),
            Outcome::Resolved("4006381333931".to_string())
        );
    }

    #[test]
    fn completa_le_dodici_cifre() {
        assert_eq!(
            parse("400638133393"),
            Outcome::Resolved("4006381333931".to_string())
        );
    }

    #[test]
    fn respinge_la_cifra_di_controllo_sbagliata() {
        assert!(parse("4006381333930").is_ambiguous());
    }

    #[test]
    fn il_campo_vuoto_non_e_un_errore() {
        assert_eq!(parse(""), Outcome::Absent);
    }

    #[test]
    fn segnala_ean8_come_non_convertibile() {
        assert!(parse("96385074").is_ambiguous());
    }

    #[test]
    fn segnala_i_codici_alfanumerici() {
        assert!(parse("ABC123").is_ambiguous());
    }
}
