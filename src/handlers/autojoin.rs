use matrix_sdk::{
    room::Room,
    ruma::events::{room::member::MemberEventContent, StrippedStateEvent},
    Client, EventHandler,
};
use time::ext::NumericalStdDuration;

pub struct OnJoin {
    client: Client,
}

impl OnJoin {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn as_eh(self) -> Box<dyn EventHandler> {
        Box::new(self)
    }
}

#[matrix_sdk::async_trait]
impl EventHandler for OnJoin {
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
