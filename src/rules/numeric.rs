//! Lettura di numeri scritti da esseri umani.
//!
//! Il separatore decimale cambia da fonte a fonte, e la stessa stringa puo'
//! significare due cose diverse: `1,234` vale 1.234 in Italia e 1234 negli Stati
//! Uniti. Qui si sceglie di **non indovinare**: i casi realmente ambigui vengono
//! respinti, perche' un prezzo sbagliato di tre ordini di grandezza fa piu' danno
//! di una riga in piu' da rivedere a mano.

/// Motivo per cui una stringa numerica non e' stata accettata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NumericError {
    NoDigits,
    Ambiguous,
    Malformed,
}

impl NumericError {
    pub fn reason(&self) -> &'static str {
        match self {
            NumericError::NoDigits => "nessuna cifra riconoscibile",
            NumericError::Ambiguous => {
                "separatore ambiguo: le tre cifre finali possono essere decimali o migliaia"
            }
            NumericError::Malformed => "formato numerico non interpretabile",
        }
    }
}

/// Interpreta un numero decimale scritto in formato italiano o anglosassone.
///
/// Regole applicate, in ordine:
/// 1. Se compaiono sia `.` sia `,`, l'ultimo dei due e' il separatore decimale
///    e l'altro e' quello delle migliaia. Questo caso non e' mai ambiguo.
/// 2. Se ne compare uno solo seguito da esattamente 3 cifre, e' impossibile
///    distinguere `1,234` (millequattro) da `1,234` (uno virgola due tre quattro):
///    si respinge.
/// 3. Negli altri casi il separatore singolo e' decimale.
pub fn parse_decimal(input: &str) -> Result<f64, NumericError> {
    let cleaned: String = input
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',' || *c == '-')
        .collect();

    if !cleaned.chars().any(|c| c.is_ascii_digit()) {
        return Err(NumericError::NoDigits);
    }

    let last_dot = cleaned.rfind('.');
    let last_comma = cleaned.rfind(',');

    let normalized = match (last_dot, last_comma) {
        (Some(d), Some(c)) => {
            // Il separatore piu' a destra e' quello decimale.
            let (decimal_at, thousands) = if d > c { (d, ',') } else { (c, '.') };
            let mut s: String = cleaned
                .chars()
                .enumerate()
                .filter(|(i, ch)| !(*ch == thousands || (*i != decimal_at && (*ch == '.' || *ch == ','))))
                .map(|(_, ch)| ch)
                .collect();
            // Il carattere rimasto in posizione decimale puo' essere una virgola.
            s = s.replace(',', ".");
            s
        }
        (Some(pos), None) | (None, Some(pos)) => {
            let decimals = cleaned.len() - pos - 1;
            if decimals == 3 && cleaned[..pos].chars().any(|c| c.is_ascii_digit()) {
                return Err(NumericError::Ambiguous);
            }
            cleaned.replace(',', ".")
        }
        (None, None) => cleaned,
    };

    normalized.parse::<f64>().map_err(|_| NumericError::Malformed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legge_il_formato_italiano() {
        assert_eq!(parse_decimal("2,5").unwrap(), 2.5);
        assert_eq!(parse_decimal("1.234,56").unwrap(), 1234.56);
    }

    #[test]
    fn legge_il_formato_anglosassone() {
        assert_eq!(parse_decimal("2.5").unwrap(), 2.5);
        assert_eq!(parse_decimal("1,234.56").unwrap(), 1234.56);
    }

    #[test]
    fn legge_gli_interi() {
        assert_eq!(parse_decimal("1200").unwrap(), 1200.0);
        assert_eq!(parse_decimal("99").unwrap(), 99.0);
    }

    #[test]
    fn respinge_il_separatore_ambiguo() {
        // Puo' valere 1234 oppure 1.234: indovinare qui significa sbagliare
        // di un fattore mille su una parte del catalogo.
        assert_eq!(parse_decimal("1,234"), Err(NumericError::Ambiguous));
        assert_eq!(parse_decimal("1.234"), Err(NumericError::Ambiguous));
    }

    #[test]
    fn accetta_tre_decimali_solo_se_il_contesto_e_chiaro() {
        // Con due separatori la posizione del decimale e' determinata.
        assert_eq!(parse_decimal("1.234,567").unwrap(), 1234.567);
    }

    #[test]
    fn respinge_le_stringhe_senza_cifre() {
        assert_eq!(parse_decimal("taglia unica"), Err(NumericError::NoDigits));
        assert_eq!(parse_decimal(""), Err(NumericError::NoDigits));
    }

    #[test]
    fn ignora_i_caratteri_di_contorno() {
        assert_eq!(parse_decimal("~ 42 circa").unwrap(), 42.0);
    }
}
