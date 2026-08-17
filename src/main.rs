use catalog_normalizer::model::Currency;
use catalog_normalizer::pipeline::{self, Config};
use catalog_normalizer::resolver::{AnthropicResolver, NullResolver, Resolver};
use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// Normalizza righe prodotto disordinate in un catalogo strutturato.
///
/// Le regole deterministiche risolvono quanto possono. Solo il residuo ambiguo
/// puo' passare a un modello (con --llm), e ogni sua risposta viene rifatta
/// passare dalle stesse regole prima di entrare nel catalogo.
#[derive(Parser)]
#[command(name = "catalog-normalizer", version, about, long_about = None)]
struct Cli {
    /// CSV di ingresso (colonne: sku, name, brand, color, size, price, ean)
    #[arg(short, long)]
    input: PathBuf,

    /// Catalogo normalizzato in uscita (JSON)
    #[arg(short, long, default_value = "catalog.json")]
    output: PathBuf,

    /// Log degli scarti, una riga JSON per campo non risolto
    #[arg(short, long, default_value = "rejects.jsonl")]
    rejects: PathBuf,

    /// Valuta da assumere quando il prezzo non ne indica una
    #[arg(long, value_parser = parse_currency)]
    default_currency: Option<Currency>,

    /// Manda il residuo ambiguo a un modello. Richiede ANTHROPIC_API_KEY.
    #[arg(long)]
    llm: bool,

    /// Modello da usare con --llm
    #[arg(long, default_value = "claude-opus-5")]
    model: String,

    /// Quanti campi ambigui per chiamata
    #[arg(long, default_value_t = 50)]
    batch_size: usize,
}

fn parse_currency(s: &str) -> Result<Currency, String> {
    match s.to_uppercase().as_str() {
        "EUR" => Ok(Currency::Eur),
        "USD" => Ok(Currency::Usd),
        "GBP" => Ok(Currency::Gbp),
        other => Err(format!("valuta non supportata: {other} (attese: EUR, USD, GBP)")),
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let mut resolver: Box<dyn Resolver> = if cli.llm {
        match AnthropicResolver::from_env(&cli.model) {
            Ok(r) => Box::new(r),
            Err(e) => {
                eprintln!("errore: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        Box::new(NullResolver)
    };

    let cfg = Config {
        input: &cli.input,
        output: &cli.output,
        rejects: &cli.rejects,
        default_currency: cli.default_currency,
        batch_size: cli.batch_size,
        model: &cli.model,
    };

    match pipeline::run(&cfg, resolver.as_mut()) {
        Ok(report) => {
            println!("righe lette              {}", report.rows);
            println!("ambigui dopo le regole    {}", report.ambiguous_after_rules);
            if cli.llm {
                println!("risolti dal modello       {}", report.resolved_by_model);
                println!("proposte rifiutate        {}", report.rejected_by_validation);
                println!(
                    "token                     {} in / {} out in {} chiamate",
                    report.usage.input_tokens, report.usage.output_tokens, report.usage.requests
                );
                println!("costo                     ${:.4}", report.cost_usd);
            }
            println!("scartati                  {}", report.discarded);
            println!();
            println!("catalogo -> {}", cli.output.display());
            println!("scarti   -> {}", cli.rejects.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("errore: {e}");
            ExitCode::FAILURE
        }
    }
}
