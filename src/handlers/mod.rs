use matrix_sdk::Client;

use crate::dispatch::NEventHandler;

mod autojoin;
mod commands;

pub fn build_handlers(client: Client) -> NEventHandler {
    NEventHandler::new([
        autojoin::OnJoin::new(client.clone()).as_eh(),
        commands::OnMessage::new(client).as_eh(),
    ])
}
