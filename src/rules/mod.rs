//! Le regole deterministiche: tutto cio' che si puo' decidere senza un modello.
//!
//! Questo strato deve risolvere il piu' possibile, perche' ogni campo risolto
//! qui e' un campo che non costa una chiamata di inferenza. Cio' che resta esce
//! come [`Ambiguity`] con la motivazione, e sara' il chiamante a decidere se
//! passarlo a un modello o scartarlo.

pub mod brand;
pub mod color;
pub mod dimensions;
pub mod ean;
pub mod numeric;
pub mod price;

use crate::model::{Currency, NormalizedProduct, Outcome, RawProduct};
use serde::Serialize;

/// Un campo pieno che le regole non hanno saputo interpretare.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ambiguity {
    pub sku: String,
    pub field: String,
    pub raw: String,
    pub reason: String,
}

pub struct RuleOutput {
    pub product: NormalizedProduct,
    pub ambiguities: Vec<Ambiguity>,
}

/// Applica tutte le regole a una riga, raccogliendo separatamente i valori
/// risolti e i campi rimasti ambigui.
pub fn normalize(raw: &RawProduct, default_currency: Option<Currency>) -> RuleOutput {
    let mut ambiguities = Vec::new();

    // Ogni campo segue lo stesso schema: risolto -> valore, assente -> None,
    // ambiguo -> None piu' una riga nel registro delle ambiguita'.
    macro_rules! take {
        ($field:literal, $outcome:expr) => {
            match $outcome {
                Outcome::Resolved(v) => Some(v),
                Outcome::Absent => None,
                Outcome::Ambiguous { raw: r, reason } => {
                    ambiguities.push(Ambiguity {
                        sku: raw.sku.clone(),
                        field: $field.to_string(),
                        raw: r,
                        reason,
                    });
                    None
                }
            }
        };
    }

    let brand = take!("brand", brand::parse(&raw.brand));
    let color = take!("color", color::parse(&raw.color));
    let dimensions_mm = take!("size", dimensions::parse(&raw.size));
    let price = take!("price", price::parse(&raw.price, default_currency));
    let ean13 = take!("ean", ean::parse(&raw.ean));

    RuleOutput {
        product: NormalizedProduct {
            sku: raw.sku.trim().to_string(),
            name: raw.name.trim().to_string(),
            brand,
            color,
            dimensions_mm,
            price,
            ean13,
        },
        ambiguities,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;

    fn raw(sku: &str) -> RawProduct {
        RawProduct {
            sku: sku.to_string(),
            name: "Piano in marmo".to_string(),
            brand: String::new(),
            color: String::new(),
            size: String::new(),
            price: String::new(),
            ean: String::new(),
        }
    }

    #[test]
    fn una_riga_pulita_non_produce_ambiguita() {
        let mut r = raw("SKU-1");
        r.brand = "ACME S.r.l.".into();
        r.color = "nero opaco".into();
        r.size = "120x60x2 cm".into();
        r.price = "1.234,56 €".into();
        r.ean = "4006381333931".into();

        let out = normalize(&r, None);
        assert!(out.ambiguities.is_empty(), "{:?}", out.ambiguities);
        assert_eq!(out.product.brand.as_deref(), Some("Acme"));
        assert_eq!(out.product.color, Some(Color::Nero));
        assert_eq!(out.product.dimensions_mm.unwrap().length_mm, 1200.0);
        assert_eq!(out.product.price.unwrap().cents, 123456);
    }

    #[test]
    fn i_campi_vuoti_non_generano_ambiguita() {
        let out = normalize(&raw("SKU-2"), None);
        assert!(out.ambiguities.is_empty());
        assert_eq!(out.product.brand, None);
        assert_eq!(out.product.color, None);
    }

    #[test]
    fn ogni_campo_ambiguo_e_registrato_col_suo_nome() {
        let mut r = raw("SKU-3");
        r.color = "turchese".into();
        r.size = "taglia L".into();

        let out = normalize(&r, None);
        let campi: Vec<&str> = out.ambiguities.iter().map(|a| a.field.as_str()).collect();
        assert!(campi.contains(&"color"));
        assert!(campi.contains(&"size"));
        assert_eq!(out.ambiguities.len(), 2);
        assert!(out.ambiguities.iter().all(|a| a.sku == "SKU-3"));
        assert!(out.ambiguities.iter().all(|a| !a.reason.is_empty()));
    }

    #[test]
    fn un_campo_ambiguo_non_blocca_gli_altri() {
        let mut r = raw("SKU-4");
        r.color = "turchese".into();
        r.price = "10,00 €".into();

        let out = normalize(&r, None);
        assert_eq!(out.ambiguities.len(), 1);
        assert_eq!(out.product.price.unwrap().cents, 1000);
    }
}
