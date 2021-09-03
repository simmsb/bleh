use std::ffi::OsString;

use color_eyre::eyre::Result;
use envconfig::Envconfig;
use matrix_sdk::{
    Client, ClientConfig, SyncSettings,
};
use path_abs::PathAbs;
use tokio::fs;
use tracing_subscriber::EnvFilter;

mod dispatch;
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
}

async fn login_and_sync(config: &Config) -> Result<()> {
    let dir = PathAbs::new(&config.config_path)?;

    fs::create_dir_all(&dir).await?;

    let client_config = ClientConfig::new().store_path(&dir);

    let client = Client::new_with_config(config.homeserver.clone(), client_config)?;

    client
        .login(&config.username, &config.password, None, Some("TestBot"))
        .await?;

    tracing::info!(username = %config.username, "Logged in {}", config.username);

    let eh = handlers::build_handlers(client.clone());

    client.set_event_handler(Box::new(eh)).await;

    client.sync(SyncSettings::default()).await;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("bleh=info".parse()?))
        .init();

    let config = Config::init_from_env()?;

    login_and_sync(&config).await?;

    Ok(())
}
