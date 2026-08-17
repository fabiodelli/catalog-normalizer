//! Da CSV disordinato a catalogo normalizzato piu' log degli scarti.

use crate::error::Result;
use crate::model::{Currency, NormalizedProduct, Outcome, RawProduct};
use crate::resolver::{self, Resolution, Resolver, Usage};
use crate::rules::{self, Ambiguity};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

/// Cosa e' successo all'intero lotto.
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub rows: usize,
    /// Campi ambigui dopo le sole regole deterministiche.
    pub ambiguous_after_rules: usize,
    /// Di quelli, quanti il modello ha risolto e le regole hanno accettato.
    pub resolved_by_model: usize,
    /// Proposte del modello rifiutate dalla validazione.
    pub rejected_by_validation: usize,
    /// Campi rimasti irrisolti: finiscono nel log degli scarti.
    pub discarded: usize,
    pub usage: Usage,
    pub cost_usd: f64,
}

pub struct Config<'a> {
    pub input: &'a Path,
    pub output: &'a Path,
    pub rejects: &'a Path,
    pub default_currency: Option<Currency>,
    pub batch_size: usize,
    pub model: &'a str,
}

pub fn run(cfg: &Config, resolver: &mut dyn Resolver) -> Result<Report> {
    let mut report = Report::default();

    // 1. Regole deterministiche su ogni riga.
    let mut reader = csv::Reader::from_path(cfg.input)?;
    let mut products: Vec<NormalizedProduct> = Vec::new();
    let mut ambiguities: Vec<Ambiguity> = Vec::new();

    for row in reader.deserialize::<RawProduct>() {
        let raw = row?;
        let out = rules::normalize(&raw, cfg.default_currency);
        report.rows += 1;
        ambiguities.extend(out.ambiguities);
        products.push(out.product);
    }
    report.ambiguous_after_rules = ambiguities.len();

    // 2. Solo il residuo va al modello, a lotti.
    let mut accepted: Vec<Resolution> = Vec::new();
    for chunk in ambiguities.chunks(cfg.batch_size.max(1)) {
        for proposal in resolver.resolve(chunk)? {
            // 3. Il modello propone, le regole validano.
            if resolver::validate(&proposal.field, &proposal.value, cfg.default_currency) {
                accepted.push(proposal);
            } else {
                report.rejected_by_validation += 1;
            }
        }
    }
    report.resolved_by_model = accepted.len();

    // 4. Applica i valori accettati e stabilisce cosa resta scarto.
    let resolved_keys = apply(&mut products, &accepted, cfg.default_currency);
    let discarded: Vec<&Ambiguity> = ambiguities
        .iter()
        .filter(|a| !resolved_keys.contains_key(&(a.sku.clone(), a.field.clone())))
        .collect();
    report.discarded = discarded.len();

    // 5. Uscite.
    let json = serde_json::to_string_pretty(&products)?;
    std::fs::write(cfg.output, json)?;

    let mut rejects = std::fs::File::create(cfg.rejects)?;
    for a in &discarded {
        writeln!(rejects, "{}", serde_json::to_string(a)?)?;
    }

    report.usage = resolver.usage();
    report.cost_usd = report.usage.cost_usd(resolver::prices_for(cfg.model));

    Ok(report)
}

/// Scrive i valori accettati nei prodotti, ri-passando dalle regole per ottenere
/// il tipo giusto. Restituisce le chiavi effettivamente riempite.
fn apply(
    products: &mut [NormalizedProduct],
    accepted: &[Resolution],
    default_currency: Option<Currency>,
) -> HashMap<(String, String), ()> {
    // L'indice possiede le chiavi: tenere &str presi da `products` impedirebbe
    // di mutare `products` subito dopo.
    let mut index: HashMap<String, usize> = HashMap::new();
    for (i, p) in products.iter().enumerate() {
        index.insert(p.sku.clone(), i);
    }

    let mut filled = HashMap::new();
    for r in accepted {
        let Some(&i) = index.get(&r.sku) else { continue };
        let p = &mut products[i];
        let ok = match r.field.as_str() {
            "color" => match rules::color::parse(&r.value) {
                Outcome::Resolved(c) => {
                    p.color = Some(c);
                    true
                }
                _ => false,
            },
            "brand" => match rules::brand::parse(&r.value) {
                Outcome::Resolved(b) => {
                    p.brand = Some(b);
                    true
                }
                _ => false,
            },
            "ean" => match rules::ean::parse(&r.value) {
                Outcome::Resolved(e) => {
                    p.ean13 = Some(e);
                    true
                }
                _ => false,
            },
            "size" => match rules::dimensions::parse(&format!("{} mm", r.value)) {
                Outcome::Resolved(d) => {
                    p.dimensions_mm = Some(d);
                    true
                }
                _ => false,
            },
            "price" => {
                let mut parts = r.value.split_whitespace();
                match parts.next().and_then(|c| c.parse::<i64>().ok()) {
                    Some(cents) => {
                        let currency = parts.next().unwrap_or("");
                        let recomposed =
                            format!("{}.{:02} {}", cents / 100, (cents % 100).abs(), currency);
                        match rules::price::parse(&recomposed, default_currency) {
                            Outcome::Resolved(m) => {
                                p.price = Some(m);
                                true
                            }
                            _ => false,
                        }
                    }
                    None => false,
                }
            }
            _ => false,
        };
        if ok {
            filled.insert((r.sku.clone(), r.field.clone()), ());
        }
    }
    filled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Color;
    use crate::resolver::NullResolver;

    fn scratch(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("catnorm-{name}-{}", std::process::id()));
        p
    }

    fn write_csv(path: &Path, body: &str) {
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn una_riga_sporca_produce_catalogo_e_scarti() {
        let input = scratch("in.csv");
        let out = scratch("out.json");
        let rej = scratch("rej.jsonl");

        write_csv(
            &input,
            "sku,name,brand,color,size,price,ean\n\
             SKU-1,Piano,ACME S.r.l.,nero opaco,120x60 cm,\"1.234,56 €\",4006381333931\n\
             SKU-2,Mensola,,turchese,taglia L,su richiesta,\n",
        );

        let cfg = Config {
            input: &input,
            output: &out,
            rejects: &rej,
            default_currency: Some(Currency::Eur),
            batch_size: 20,
            model: "claude-opus-5",
        };

        let report = run(&cfg, &mut NullResolver).unwrap();

        assert_eq!(report.rows, 2);
        // SKU-2: color, size e price sono tutti e tre ambigui.
        assert_eq!(report.ambiguous_after_rules, 3);
        assert_eq!(report.resolved_by_model, 0, "NullResolver non risolve");
        assert_eq!(report.discarded, 3);
        assert_eq!(report.cost_usd, 0.0);

        let catalog: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let first = &catalog[0];
        assert_eq!(first["brand"], "Acme");
        assert_eq!(first["color"], "nero");
        assert_eq!(first["price"]["cents"], 123456);

        // Ogni scarto porta con se' la motivazione: e' il punto del log.
        let rejects = std::fs::read_to_string(&rej).unwrap();
        assert_eq!(rejects.lines().count(), 3);
        for line in rejects.lines() {
            let a: serde_json::Value = serde_json::from_str(line).unwrap();
            assert_eq!(a["sku"], "SKU-2");
            assert!(!a["reason"].as_str().unwrap().is_empty());
        }

        for p in [input, out, rej] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn le_proposte_non_valide_vengono_contate_e_non_applicate() {
        // Un resolver che propone un colore inesistente: la validazione lo ferma.
        struct Bugiardo;
        impl Resolver for Bugiardo {
            fn resolve(&mut self, batch: &[Ambiguity]) -> Result<Vec<Resolution>> {
                Ok(batch
                    .iter()
                    .filter(|a| a.field == "color")
                    .map(|a| Resolution {
                        sku: a.sku.clone(),
                        field: "color".into(),
                        value: "turchese".into(),
                    })
                    .collect())
            }
            fn usage(&self) -> Usage {
                Usage::default()
            }
        }

        let input = scratch("in2.csv");
        let out = scratch("out2.json");
        let rej = scratch("rej2.jsonl");

        write_csv(&input, "sku,name,color\nSKU-9,Vaso,turchese\n");

        let cfg = Config {
            input: &input,
            output: &out,
            rejects: &rej,
            default_currency: None,
            batch_size: 20,
            model: "claude-opus-5",
        };

        let report = run(&cfg, &mut Bugiardo).unwrap();
        assert_eq!(report.rejected_by_validation, 1);
        assert_eq!(report.resolved_by_model, 0);
        assert_eq!(report.discarded, 1);

        let catalog: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert!(catalog[0].get("color").is_none(), "il colore inventato non entra");

        for p in [input, out, rej] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn una_proposta_valida_viene_applicata() {
        struct Corretto;
        impl Resolver for Corretto {
            fn resolve(&mut self, batch: &[Ambiguity]) -> Result<Vec<Resolution>> {
                Ok(batch
                    .iter()
                    .filter(|a| a.field == "color")
                    .map(|a| Resolution {
                        sku: a.sku.clone(),
                        field: "color".into(),
                        // "acquamarina" non e' in tavolozza; "blu" si'.
                        value: "blu".into(),
                    })
                    .collect())
            }
            fn usage(&self) -> Usage {
                Usage { input_tokens: 100, output_tokens: 20, requests: 1 }
            }
        }

        let input = scratch("in3.csv");
        let out = scratch("out3.json");
        let rej = scratch("rej3.jsonl");

        write_csv(&input, "sku,name,color\nSKU-7,Tazza,acquamarina\n");

        let cfg = Config {
            input: &input,
            output: &out,
            rejects: &rej,
            default_currency: None,
            batch_size: 20,
            model: "claude-opus-5",
        };

        let report = run(&cfg, &mut Corretto).unwrap();
        assert_eq!(report.resolved_by_model, 1);
        assert_eq!(report.rejected_by_validation, 0);
        assert_eq!(report.discarded, 0);
        assert!(report.cost_usd > 0.0, "i token contabilizzati producono un costo");

        let catalog: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(catalog[0]["color"], format!("{}", Color::Blu));

        for p in [input, out, rej] {
            let _ = std::fs::remove_file(p);
        }
    }
}
