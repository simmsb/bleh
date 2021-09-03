use std::ffi::OsString;

use color_eyre::eyre::Result;
use envconfig::Envconfig;
use matrix_sdk::{
    room::Room,
    ruma::events::{room::member::MemberEventContent, StrippedStateEvent},
    Client, ClientConfig, EventHandler, SyncSettings,
};
use path_abs::PathAbs;
use time::ext::NumericalStdDuration;
use tokio::fs;
use tracing_subscriber::EnvFilter;

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

struct Bot {
    client: Client,
}

impl Bot {
    fn new(client: Client) -> Self {
        Self { client }
    }
}

#[matrix_sdk::async_trait]
impl EventHandler for Bot {
    async fn on_stripped_state_member(
        &self,
        room: Room,
        room_member: &StrippedStateEvent<MemberEventContent>,
        _: Option<MemberEventContent>,
    ) {
        if room_member.state_key != self.client.user_id().await.unwrap() {
            return;
        }

        if let Room::Invited(room) = room {
            tracing::info!(room = %room.room_id(), "Joining room");

            let mut delay = 2.std_seconds();

            while let Err(err) = room.accept_invitation().await {
                tracing::warn!(?err, room = %room.room_id(), ?delay, "Failed to join room, retrying");

                tokio::time::sleep(delay).await;
                delay *= 2;

                if delay > 3600.std_seconds() {
                    tracing::error!(?err, room = %room.room_id(), "Couldn't join room");
                    return;
                }
            }

            tracing::info!(room = %room.room_id(), "Joined room");
        }
    }
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

    client
        .set_event_handler(Box::new(Bot::new(client.clone())))
        .await;

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
