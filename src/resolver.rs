//! Il residuo: cio' che le regole non hanno saputo risolvere.
//!
//! Qui sta il principio del progetto. Le regole decidono tutto quello che possono
//! decidere; il modello vede **solo** cio' che resta, in un'unica chiamata per
//! lotto, e la sua risposta non viene creduta sulla parola: ogni valore proposto
//! viene rifatto passare dalla stessa regola che aveva fallito. Se la regola non
//! lo accetta, il campo resta uno scarto.
//!
//! Il modello propone, le regole validano. Non c'e' modo per un'allucinazione di
//! entrare nel catalogo.

use crate::error::{Error, Result};
use crate::model::{Color, Currency, Outcome};
use crate::rules::{self, Ambiguity};
use serde::{Deserialize, Serialize};

/// Valore proposto per un campo rimasto ambiguo.
#[derive(Debug, Clone, Deserialize)]
pub struct Resolution {
    pub sku: String,
    pub field: String,
    pub value: String,
}

/// Token consumati e costo corrispondente.
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub requests: u32,
}

impl Usage {
    /// Costo in dollari, dai prezzi per milione di token del modello scelto.
    pub fn cost_usd(&self, prices: Prices) -> f64 {
        (self.input_tokens as f64 / 1_000_000.0) * prices.input_per_mtok
            + (self.output_tokens as f64 / 1_000_000.0) * prices.output_per_mtok
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Prices {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
}

/// Listino al 2026-08. Un modello sconosciuto non blocca l'esecuzione: si
/// contano comunque i token e il costo resta a zero, dichiarato come tale.
pub fn prices_for(model: &str) -> Prices {
    match model {
        "claude-opus-5" => Prices { input_per_mtok: 5.0, output_per_mtok: 25.0 },
        "claude-sonnet-5" => Prices { input_per_mtok: 3.0, output_per_mtok: 15.0 },
        "claude-haiku-4-5" => Prices { input_per_mtok: 1.0, output_per_mtok: 5.0 },
        _ => Prices { input_per_mtok: 0.0, output_per_mtok: 0.0 },
    }
}

/// Chi prova a risolvere il residuo.
///
/// Il tratto esiste perche' ci sono due implementazioni reali con lo stesso
/// contratto: nessuna chiamata, e una chiamata vera a un modello. I test girano
/// sulla prima e non toccano la rete.
pub trait Resolver {
    fn resolve(&mut self, batch: &[Ambiguity]) -> Result<Vec<Resolution>>;
    fn usage(&self) -> Usage;
}

/// Non risolve niente: ogni ambiguita' resta uno scarto. E' il comportamento
/// predefinito, quello che gira senza chiave API e dentro i test.
pub struct NullResolver;

impl Resolver for NullResolver {
    fn resolve(&mut self, _batch: &[Ambiguity]) -> Result<Vec<Resolution>> {
        Ok(Vec::new())
    }
    fn usage(&self) -> Usage {
        Usage::default()
    }
}

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";

pub struct AnthropicResolver {
    api_key: String,
    model: String,
    usage: Usage,
}

impl AnthropicResolver {
    /// Legge la chiave da ANTHROPIC_API_KEY.
    pub fn from_env(model: &str) -> Result<Self> {
        let api_key =
            std::env::var("ANTHROPIC_API_KEY").map_err(|_| Error::MissingEnv("ANTHROPIC_API_KEY"))?;
        Ok(Self { api_key, model: model.to_string(), usage: Usage::default() })
    }

    fn prompt(batch: &[Ambiguity]) -> String {
        let mut s = String::from(
            "Normalizza questi valori di catalogo che le regole automatiche non hanno saputo \
             interpretare. Per ogni voce restituisci il valore normalizzato, oppure ometti la \
             voce se il valore e' genuinamente indecidibile.\n\n\
             Formati attesi per campo:\n\
             - color: uno fra ",
        );
        s.push_str(&Color::all().join(", "));
        s.push_str(
            "\n\
             - size: dimensioni in millimetri come \"LxWxT\" o \"L\", solo numeri\n\
             - price: importo in centesimi interi, seguito da spazio e valuta (EUR, USD, GBP)\n\
             - brand: nome commerciale senza forma giuridica\n\
             - ean: 13 cifre\n\n\
             Voci:\n",
        );
        for a in batch {
            s.push_str(&format!(
                "- sku={} campo={} valore={:?} (motivo: {})\n",
                a.sku, a.field, a.raw, a.reason
            ));
        }
        s
    }

    fn schema() -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "resolutions": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "sku":   { "type": "string" },
                            "field": { "type": "string" },
                            "value": { "type": "string" }
                        },
                        "required": ["sku", "field", "value"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["resolutions"],
            "additionalProperties": false
        })
    }
}

impl Resolver for AnthropicResolver {
    fn resolve(&mut self, batch: &[Ambiguity]) -> Result<Vec<Resolution>> {
        if batch.is_empty() {
            return Ok(Vec::new());
        }

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            // Output strutturato: la risposta e' JSON valido per costruzione, senza
            // doverla estrarre da testo libero ne' gestire preamboli.
            "output_config": {
                "format": { "type": "json_schema", "schema": Self::schema() },
                // Mappatura meccanica, non un problema di ragionamento: lo sforzo
                // basso taglia i token senza togliere nulla alla qualita'.
                "effort": "low"
            },
            "messages": [{ "role": "user", "content": Self::prompt(batch) }]
        });

        let response = ureq::post(API_URL)
            .set("x-api-key", &self.api_key)
            .set("anthropic-version", API_VERSION)
            .set("content-type", "application/json")
            .send_json(body)
            .map_err(|e| Error::Model(e.to_string()))?;

        let json: serde_json::Value =
            response.into_json().map_err(|e| Error::Model(e.to_string()))?;

        self.usage.requests += 1;
        if let Some(u) = json.get("usage") {
            self.usage.input_tokens += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
            self.usage.output_tokens += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
        }

        let text = json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
            })
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .ok_or_else(|| Error::Model("nessun blocco di testo nella risposta".into()))?;

        #[derive(Deserialize)]
        struct Envelope {
            resolutions: Vec<Resolution>,
        }

        let parsed: Envelope =
            serde_json::from_str(text).map_err(|e| Error::Model(format!("JSON non valido: {e}")))?;

        Ok(parsed.resolutions)
    }

    fn usage(&self) -> Usage {
        self.usage
    }
}

/// Ri-applica la regola del campo al valore proposto dal modello.
///
/// E' la sola porta d'ingresso per un valore che non venga dalle regole, ed e'
/// sorvegliata dalle regole stesse: un colore fuori tavolozza, una misura senza
/// unita' o un EAN con la cifra di controllo sbagliata vengono rifiutati qui,
/// esattamente come se li avesse scritti il fornitore.
pub fn validate(field: &str, value: &str, default_currency: Option<Currency>) -> bool {
    match field {
        "color" => matches!(rules::color::parse(value), Outcome::Resolved(_)),
        "brand" => matches!(rules::brand::parse(value), Outcome::Resolved(_)),
        "ean" => matches!(rules::ean::parse(value), Outcome::Resolved(_)),
        "size" => {
            // Il modello risponde in millimetri: si ricompone l'unita' e si rilegge.
            matches!(rules::dimensions::parse(&format!("{value} mm")), Outcome::Resolved(_))
        }
        "price" => {
            // Formato atteso "<centesimi> <VALUTA>": si riconverte in unita' intere.
            let mut parts = value.split_whitespace();
            let Some(cents) = parts.next().and_then(|c| c.parse::<i64>().ok()) else {
                return false;
            };
            let currency = parts.next().unwrap_or("");
            let recomposed = format!("{}.{:02} {}", cents / 100, (cents % 100).abs(), currency);
            matches!(rules::price::parse(&recomposed, default_currency), Outcome::Resolved(_))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn il_null_resolver_non_risolve_niente() {
        let mut r = NullResolver;
        let batch = vec![Ambiguity {
            sku: "A".into(),
            field: "color".into(),
            raw: "turchese".into(),
            reason: "fuori tavolozza".into(),
        }];
        assert!(r.resolve(&batch).unwrap().is_empty());
        assert_eq!(r.usage().requests, 0);
    }

    #[test]
    fn la_validazione_accetta_i_valori_legittimi() {
        assert!(validate("color", "nero", None));
        assert!(validate("brand", "Acme", None));
        assert!(validate("ean", "4006381333931", None));
        assert!(validate("size", "1200x600", None));
        assert!(validate("price", "123456 EUR", None));
    }

    #[test]
    fn la_validazione_respinge_le_invenzioni_del_modello() {
        // Un colore che non esiste nel catalogo non entra, chiunque lo proponga.
        assert!(!validate("color", "turchese", None));
        // Un EAN con la cifra di controllo sbagliata resta invalido.
        assert!(!validate("ean", "4006381333930", None));
        // Una misura non numerica non diventa una dimensione.
        assert!(!validate("size", "grande", None));
        // Un campo che non conosciamo non e' risolvibile per definizione.
        assert!(!validate("categoria", "qualsiasi", None));
    }

    #[test]
    fn il_costo_segue_i_token_e_il_listino() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, requests: 1 };
        let c = u.cost_usd(prices_for("claude-opus-5"));
        assert!((c - 30.0).abs() < 1e-9, "atteso 5 + 25, ottenuto {c}");
    }

    #[test]
    fn un_modello_fuori_listino_non_inventa_un_prezzo() {
        let u = Usage { input_tokens: 1_000_000, output_tokens: 1_000_000, requests: 1 };
        assert_eq!(u.cost_usd(prices_for("modello-ignoto")), 0.0);
    }
}
