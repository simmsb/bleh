use matrix_sdk::Client;
use sqlx::SqlitePool;

mod autojoin;
mod messages;

pub async fn register_handlers(client: Client, pool: SqlitePool) {
    autojoin::OnJoin::new(client.clone())
        .register(client.clone())
        .await;
    messages::OnMessage::new("!".to_owned(), client.clone(), pool)
        .register(client)
        .await;
}
