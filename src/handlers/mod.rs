use matrix_sdk::Client;

use crate::dispatch::NEventHandler;

mod autojoin;

pub fn build_handlers(client: Client) -> NEventHandler {
    NEventHandler::new([autojoin::OnJoin::new(client).as_eh()])
}
