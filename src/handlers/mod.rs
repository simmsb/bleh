use std::sync::Arc;

use matrix_sdk::Client;

use crate::framework::{commands::Group, context::BaseContext};

mod autojoin;
mod messages;

pub async fn register_handlers<C: Send + 'static>(
    client: Client,
    commands: Group<C>,
    build_context: Arc<dyn Fn(BaseContext) -> C + Send + Sync + 'static>,
) {
    autojoin::OnJoin::new(client.clone())
        .register(client.clone())
        .await;
    messages::OnMessage::new("!".to_owned(), client.clone(), commands, build_context)
        .register(client)
        .await;
}
