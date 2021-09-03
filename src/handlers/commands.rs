use matrix_sdk::{
    room::Room,
    ruma::events::{
        room::message::{MessageEventContent, MessageType, TextMessageEventContent},
        AnyMessageEventContent, SyncMessageEvent,
    },
    Client, EventHandler,
};

pub struct OnMessage {
    client: Client,
}

impl OnMessage {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn as_eh(self) -> Box<dyn EventHandler> {
        Box::new(self)
    }
}

#[matrix_sdk::async_trait]
impl EventHandler for OnMessage {
    async fn on_room_message(&self, room: Room, message: &SyncMessageEvent<MessageEventContent>) {
        let room = match room {
            Room::Joined(room) => room,
            _ => return,
        };

        if message.sender == self.client.user_id().await.unwrap() {
            return;
        }

        let msg_body = match &message.content.msgtype {
            MessageType::Text(TextMessageEventContent { body: msg_body, .. }) => msg_body.as_str(),
            _ => return,
        };

        if msg_body.contains("!test") {
            tracing::info!(sender = %message.sender, "Received command");

            let reply = AnyMessageEventContent::RoomMessage(MessageEventContent::text_reply_plain(
                "hello",
                &message.clone().into_full_event(room.room_id().clone()),
            ));

            room.send(reply, None).await.unwrap();
        }
    }
}
