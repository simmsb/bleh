use matrix_sdk::Client;

use crate::dispatch::NEventHandler;

mod autojoin;
mod messages;

pub fn build_handlers(client: Client) -> NEventHandler {
    NEventHandler::new([
        autojoin::OnJoin::new(client.clone()).into_eh(),
        messages::OnMessage::new("!".to_owned(), client).into_eh(),
    ])
}
