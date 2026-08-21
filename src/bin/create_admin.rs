use spa_sax_backend::config::AppConfig;
use spa_sax_backend::domain::users::Role;
use spa_sax_backend::infrastructure::auth::PasswordService;
use sqlx::postgres::PgPoolOptions;
use sqlx::Error as SqlxError;
use std::env;
use std::io::{self, Write};
use std::process;
use uuid::Uuid;
use validator::ValidateEmail;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    let config = match AppConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error loading environment configuration: {}", e);
            process::exit(1);
        }
    };

    let args: Vec<String> = env::args().collect();
    let mut email = String::new();
    let mut display_name = String::new();

    let mut i = 1;
    while i < args.len() {
        if args[i] == "--email" && i + 1 < args.len() {
            email = args[i + 1].clone();
            i += 1;
        } else if args[i] == "--display-name" && i + 1 < args.len() {
            display_name = args[i + 1].clone();
            i += 1;
        }
        i += 1;
    }

    if email.trim().is_empty() {
        print!("Enter Super Admin Email: ");
        if let Err(e) = io::stdout().flush() {
            eprintln!("IO Error: {}", e);
            process::exit(1);
        }
        if let Err(e) = io::stdin().read_line(&mut email) {
            eprintln!("Failed to read email input: {}", e);
            process::exit(1);
        }
    }

    let email = email.trim().to_lowercase();
    if !ValidateEmail::validate_email(&email) {
        eprintln!("Invalid email format: {}", email);
        process::exit(1);
    }

    if display_name.trim().is_empty() {
        print!("Enter Super Admin Display Name: ");
        if let Err(e) = io::stdout().flush() {
            eprintln!("IO Error: {}", e);
            process::exit(1);
        }
        if let Err(e) = io::stdin().read_line(&mut display_name) {
            eprintln!("Failed to read display name input: {}", e);
            process::exit(1);
        }
    }
    let display_name = display_name.trim().to_string();

    let password = env::var("BOOTSTRAP_ADMIN_PASSWORD").unwrap_or_default();
    let password = if password.is_empty() {
        match rpassword::prompt_password("Enter Super Admin Password: ") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Failed to read password securely: {}", e);
                process::exit(1);
            }
        }
    } else {
        password
    };

    let password = password.trim().to_string();
    if password.len() < config.password_min_length {
        eprintln!(
            "Password must be at least {} characters long.",
            config.password_min_length
        );
        process::exit(1);
    }

    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            process::exit(1);
        }
    };

    let pass_hash = match PasswordService::hash_password(&password) {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Password hashing failed: {}", e);
            process::exit(1);
        }
    };

    let user_id = Uuid::new_v4();

    let result = sqlx::query(
        r#"
        INSERT INTO users (id, email, password_hash, display_name, role, is_active)
        VALUES ($1, $2, $3, $4, $5, TRUE)
        "#,
    )
    .bind(user_id)
    .bind(&email)
    .bind(pass_hash)
    .bind(&display_name)
    .bind(Role::SuperAdmin.as_str())
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            println!("Successfully created Super Admin user: {}", email);
        }
        Err(SqlxError::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            eprintln!(
                "User creation failed: Email address '{}' already exists.",
                email
            );
            process::exit(1);
        }
        Err(e) => {
            eprintln!("Database error during admin creation: {}", e);
            process::exit(1);
        }
    }
}
