use matrix_sdk::Client;

mod autojoin;
mod messages;

pub async fn register_handlers(client: Client) {
    autojoin::OnJoin::new(client.clone())
        .register(client.clone())
        .await;
    messages::OnMessage::new("!".to_owned(), client.clone())
        .register(client)
        .await;
}
