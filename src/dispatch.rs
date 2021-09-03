use std::sync::Arc;

use matrix_sdk::{
    room::Room,
    ruma::events::{room::member::MemberEventContent, StrippedStateEvent},
    EventHandler,
};

pub struct NEventHandler {
    inner_events: Arc<Vec<Box<dyn EventHandler>>>,
}

impl NEventHandler {
    pub fn new(handlers: impl IntoIterator<Item = Box<dyn EventHandler>>) -> Self {
        Self {
            inner_events: Arc::new(handlers.into_iter().collect()),
        }
    }
}

#[matrix_sdk::async_trait]
impl EventHandler for NEventHandler {
    async fn on_stripped_state_member(
        &self,
        room: Room,
        room_member: &StrippedStateEvent<MemberEventContent>,
        member: Option<MemberEventContent>,
    ) {
        for eh in self.inner_events.as_slice() {
            eh.on_stripped_state_member(room.clone(), room_member, member.clone())
                .await;
        }
    }

    async fn on_room_message(
        &self,
        room: Room,
        message: &matrix_sdk::ruma::events::SyncMessageEvent<
            matrix_sdk::ruma::events::room::message::MessageEventContent,
        >,
    ) {
        for eh in self.inner_events.as_slice() {
            eh.on_room_message(room.clone(), message).await;
        }
    }
}
