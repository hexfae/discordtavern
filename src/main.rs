mod error;
mod prelude;
mod utils;

#[tokio::main]
async fn main() -> prelude::Result<()> {
    start_logging()?;
    utils::discord::Data::start_bot().await
}

fn start_logging() -> prelude::Result<()> {
    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
        .from_env()?
        .add_directive("discordtavern=debug".parse()?)
        // silence annoying startup logging
        .add_directive("serenity::gateway::bridge=warn".parse()?)
        // silence annoying occasional logging
        .add_directive("serenity::gateway::shard=error".parse()?);

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
    Ok(())
}
