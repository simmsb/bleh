use matrix_sdk::Client;
use sqlx::SqlitePool;

use crate::framework::Group;

mod autojoin;
mod messages;

pub async fn register_handlers(client: Client, pool: SqlitePool, commands: Group<'static>) {
    autojoin::OnJoin::new(client.clone())
        .register(client.clone())
        .await;
    messages::OnMessage::new("!".to_owned(), client.clone(), pool, commands)
        .register(client)
        .await;
}
