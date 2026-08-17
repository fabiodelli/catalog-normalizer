use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("lettura del CSV fallita: {0}")]
    Csv(#[from] csv::Error),

    #[error("errore di I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("serializzazione JSON fallita: {0}")]
    Json(#[from] serde_json::Error),

    #[error("la variabile d'ambiente {0} non e' impostata")]
    MissingEnv(&'static str),

    #[error("il modello ha risposto in modo inutilizzabile: {0}")]
    Model(String),
}

pub type Result<T> = std::result::Result<T, Error>;
