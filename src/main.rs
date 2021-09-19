#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(const_fn_trait_bound)]
#![feature(const_raw_ptr_deref)]
#![feature(const_panic)]
#![feature(const_slice_from_raw_parts)]
#![feature(const_type_name)]

use std::ffi::OsString;

use color_eyre::eyre::Result;
use envconfig::Envconfig;
use matrix_sdk::{Client, config::{ClientConfig, SyncSettings}};
use path_abs::PathAbs;
use tokio::fs;
use tracing_subscriber::EnvFilter;
use sqlx::sqlite::SqlitePool;

mod commands;
pub mod framework;
pub mod rrules;
mod handlers;

#[derive(Envconfig)]
struct Config {
    #[envconfig(from = "MATRIX_CONFIG_PATH")]
    config_path: OsString,

    #[envconfig(from = "MATRIX_HOMESERVER")]
    homeserver: url::Url,

    #[envconfig(from = "MATRIX_USERNAME")]
    username: String,

    #[envconfig(from = "MATRIX_PASSWORD")]
    password: String,

    #[envconfig(from = "MATRIX_DEVICE_ID")]
    device_id: Option<String>,

    #[envconfig(from = "DATABASE_URL")]
    sqlite_url: String,
}

async fn login_and_sync(config: &Config, pool: SqlitePool) -> Result<()> {
    let dir = PathAbs::new(&config.config_path)?;

    fs::create_dir_all(&dir).await?;

    let client_config = ClientConfig::new().store_path(&dir);

    let client = Client::new_with_config(config.homeserver.clone(), client_config)?;

    let device_id = config.device_id.as_deref();

    if let Some(device_id) = device_id {
        tracing::info!(device_id, "Restoring device id");
    }

    client
        .login(
            &config.username,
            &config.password,
            device_id,
            Some("TestBot"),
        )
        .await?;

    tracing::info!(username = %config.username, "Logged in {}", config.username);

    client.sync_once(SyncSettings::default()).await?;

    if device_id.is_none() {
        println!(
            "Generated device id: '{}'",
            client.device_id().await.unwrap().as_str()
        );
    }

    rrules::setup(client.clone(), &pool).await;

    handlers::register_handlers(client.clone(), pool, commands::make_commands()).await;

    let sync_settings = SyncSettings::default().token(client.sync_token().await.unwrap());

    client.sync(sync_settings).await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("bleh=info".parse()?))
        .init();

    let config = Config::init_from_env()?;

    let pool = SqlitePool::connect(&config.sqlite_url).await?;

    login_and_sync(&config, pool).await?;

    Ok(())
}
