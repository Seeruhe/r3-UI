use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;

pub async fn init_db(database_url: &str) -> anyhow::Result<SqlitePool> {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await?;

    // Run migrations
    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    // Create users table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            password TEXT NOT NULL,
            secret TEXT DEFAULT '',
            tfa_enabled INTEGER DEFAULT 0,
            tg_id INTEGER DEFAULT 0,
            created_at INTEGER DEFAULT 0,
            last_login INTEGER DEFAULT 0
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create inbounds table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS inbounds (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_id INTEGER NOT NULL,
            up INTEGER DEFAULT 0,
            down INTEGER DEFAULT 0,
            total INTEGER DEFAULT 0,
            all_time INTEGER DEFAULT 0,
            remark TEXT,
            enable INTEGER DEFAULT 1,
            expiry_time INTEGER DEFAULT 0,
            traffic_reset TEXT DEFAULT 'never',
            last_traffic_reset_time INTEGER DEFAULT 0,
            listen TEXT,
            port INTEGER NOT NULL,
            protocol TEXT NOT NULL,
            settings TEXT,
            stream_settings TEXT,
            tag TEXT UNIQUE NOT NULL,
            sniffing TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create settings table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS settings (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            key TEXT NOT NULL UNIQUE,
            value TEXT
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create client_traffic table
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS client_traffic (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            inbound_id INTEGER NOT NULL,
            email TEXT NOT NULL,
            up INTEGER DEFAULT 0,
            down INTEGER DEFAULT 0,
            total INTEGER DEFAULT 0,
            expiry_time INTEGER DEFAULT 0,
            enable INTEGER DEFAULT 1,
            FOREIGN KEY (inbound_id) REFERENCES inbounds(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Create default admin user if not exists
    let admin_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = 'admin'")
        .fetch_one(pool)
        .await?;

    if admin_exists.0 == 0 {
        // Default password: admin (hashed with argon2)
        let hashed_password = hash_password("admin")?;
        let now = chrono::Utc::now().timestamp();
        sqlx::query("INSERT INTO users (username, password, secret, tfa_enabled, tg_id, created_at, last_login) VALUES (?, ?, '', 0, 0, ?, ?)")
            .bind("admin")
            .bind(&hashed_password)
            .bind(now)
            .bind(now)
            .execute(pool)
            .await?;
        tracing::info!("Created default admin user (password: admin)");
    }

    // Add missing columns to existing users table (migration for existing databases)
    let columns = [
        ("secret", "TEXT DEFAULT ''"),
        ("tfa_enabled", "INTEGER DEFAULT 0"),
        ("tg_id", "INTEGER DEFAULT 0"),
        ("created_at", "INTEGER DEFAULT 0"),
        ("last_login", "INTEGER DEFAULT 0"),
    ];

    for (col_name, col_type) in columns {
        let result = sqlx::query(&format!(
            "ALTER TABLE users ADD COLUMN {} {}",
            col_name, col_type
        ))
        .execute(pool)
        .await;

        // Ignore error if column already exists
        if let Err(e) = result {
            tracing::debug!("Column {} might already exist: {}", col_name, e);
        }
    }

    tracing::info!("Database migrations completed");
    Ok(())
}

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};

fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))
}
