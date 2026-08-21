use spa_sax_backend::config::AppConfig;
use sqlx::postgres::PgPoolOptions;
use std::process;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let config = match AppConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    println!("Connecting to database: {}", config.database_url);

    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .connect(&config.database_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            process::exit(1);
        }
    };

    println!("Running migrations...");
    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => {
            println!("Migrations completed successfully.");
        }
        Err(e) => {
            eprintln!("Migration failed: {}", e);
            process::exit(1);
        }
    }

    Ok(())
}
