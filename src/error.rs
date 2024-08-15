#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Discord(#[from] poise::serenity_prelude::Error),
    #[error(transparent)]
    OpenAI(#[from] async_openai::error::OpenAIError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    SpannedRon(#[from] ron::error::SpannedError),
    #[error(transparent)]
    Ron(#[from] ron::Error),
    #[error(transparent)]
    TracingFromEnv(#[from] tracing_subscriber::filter::FromEnvError),
    #[error(transparent)]
    TracingParse(#[from] tracing_subscriber::filter::ParseError),
}
