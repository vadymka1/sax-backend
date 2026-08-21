use spa_sax_backend::bootstrap::build_rocket;
use spa_sax_backend::config::AppConfig;
use tracing_subscriber::EnvFilter;

#[rocket::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = AppConfig::from_env()?;
    tracing::info!("Starting application on {}:{}", config.host, config.port);

    let rocket = build_rocket(config).await?;
    rocket.launch().await?;

    Ok(())
}
