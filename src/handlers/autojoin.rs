use std::sync::Arc;

use matrix_sdk::{
    room::Room,
    ruma::events::room::member::StrippedRoomMemberEvent,
    Client,
};
use time::ext::NumericalStdDuration;

#[derive(Clone)]
pub struct OnJoin {
    client: Client,
}

impl OnJoin {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn register(self, client: Client) {
        let self_ = Arc::new(self);
        client
            .register_event_handler(move |room_member, room| {
                let self_ = self_.clone();
                async move {
                    self_.on_stripped_state_member(room_member, room).await;
                }
            })
            .await;
    }

    async fn on_stripped_state_member(
        &self,
        room_member: StrippedRoomMemberEvent,
        room: Room,
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
