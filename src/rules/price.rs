//! Prezzi: da testo libero a centesimi interi.
//!
//! Il valore viene tenuto in centesimi come intero, mai in virgola mobile: su un
//! catalogo di centinaia di migliaia di righe gli errori di arrotondamento si
//! accumulano, e un prezzo e' un dato contabile.

use super::numeric::parse_decimal;
use crate::model::{Currency, Money, Outcome};

/// Simboli e codici ISO riconosciuti. I codici a tre lettere vanno cercati prima
/// dei simboli per non lasciare residui alfabetici nella stringa numerica.
const CURRENCIES: &[(&str, Currency)] = &[
    ("eur", Currency::Eur),
    ("usd", Currency::Usd),
    ("gbp", Currency::Gbp),
    ("€", Currency::Eur),
    ("$", Currency::Usd),
    ("£", Currency::Gbp),
];

pub fn parse(raw: &str, default_currency: Option<Currency>) -> Outcome<Money> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Outcome::Absent;
    }

    let lower = trimmed.to_lowercase();

    let mut currency = None;
    let mut body = lower.clone();
    for (token, c) in CURRENCIES {
        if let Some(pos) = body.find(token) {
            currency = Some(*c);
            body.replace_range(pos..pos + token.len(), " ");
            break;
        }
    }

    let currency = match currency.or(default_currency) {
        Some(c) => c,
        None => {
            return Outcome::ambiguous(
                trimmed,
                "valuta non indicata e nessuna valuta predefinita impostata",
            )
        }
    };

    // Se restano lettere, la stringa contiene qualcosa che non e' un prezzo.
    if body.chars().any(|c| c.is_alphabetic()) {
        return Outcome::ambiguous(trimmed, "testo non riconducibile a un importo");
    }

    let value = match parse_decimal(&body) {
        Ok(v) => v,
        Err(e) => return Outcome::ambiguous(trimmed, e.reason()),
    };

    if value < 0.0 {
        return Outcome::ambiguous(trimmed, "importo negativo");
    }

    // `round` prima della conversione: 19.99 in binario e' 19.989999...
    let cents = (value * 100.0).round() as i64;

    Outcome::Resolved(Money { cents, currency })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved(raw: &str, default_currency: Option<Currency>) -> Money {
        match parse(raw, default_currency) {
            Outcome::Resolved(m) => m,
            other => panic!("atteso Resolved per {raw:?}, ottenuto {other:?}"),
        }
    }

    #[test]
    fn legge_il_formato_italiano_con_simbolo() {
        let m = resolved("1.234,56 €", None);
        assert_eq!(m.cents, 123456);
        assert_eq!(m.currency, Currency::Eur);
    }

    #[test]
    fn legge_il_formato_anglosassone_con_simbolo() {
        let m = resolved("$1,234.56", None);
        assert_eq!(m.cents, 123456);
        assert_eq!(m.currency, Currency::Usd);
    }

    #[test]
    fn legge_il_codice_iso() {
        assert_eq!(resolved("1234.56 EUR", None).currency, Currency::Eur);
    }

    #[test]
    fn arrotonda_correttamente_i_centesimi() {
        // 19.99 non e' rappresentabile esattamente in binario.
        assert_eq!(resolved("19,99 €", None).cents, 1999);
    }

    #[test]
    fn usa_la_valuta_predefinita_quando_manca() {
        let m = resolved("99", Some(Currency::Eur));
        assert_eq!(m.cents, 9900);
        assert_eq!(m.currency, Currency::Eur);
    }

    #[test]
    fn senza_valuta_ne_predefinita_segnala() {
        assert!(parse("99", None).is_ambiguous());
    }

    #[test]
    fn il_campo_vuoto_non_e_un_errore() {
        assert_eq!(parse("", Some(Currency::Eur)), Outcome::Absent);
    }

    #[test]
    fn respinge_il_separatore_ambiguo() {
        // 1,234 euro: 1234 o 1,23? Meglio una riga da rivedere che un prezzo
        // sbagliato di mille volte.
        assert!(parse("1,234 €", None).is_ambiguous());
    }

    #[test]
    fn respinge_il_testo_libero() {
        assert!(parse("su richiesta", Some(Currency::Eur)).is_ambiguous());
        assert!(parse("prezzo da concordare", Some(Currency::Eur)).is_ambiguous());
    }

    #[test]
    fn respinge_gli_importi_negativi() {
        assert!(parse("-10 €", None).is_ambiguous());
    }
}
