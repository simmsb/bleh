use std::sync::Arc;

use matrix_sdk::{
    room::Room,
    ruma::events::{
        room::message::{MessageEventContent, MessageType, TextMessageEventContent},
        SyncMessageEvent,
    },
    Client,
};

use crate::framework::{
    commands::{Group, GroupMeta},
    context::BaseContext,
};

pub struct OnMessage<C> {
    prefix: Arc<String>,
    client: Client,
    commands: Arc<Group<C>>,
    commands_meta: Arc<GroupMeta>,
    build_context: Arc<dyn Fn(BaseContext) -> C + Send + Sync + 'static>,
}

impl<C: Send + 'static> OnMessage<C> {
    pub fn new(
        prefix: String,
        client: Client,
        commands: Group<C>,
        build_context: Arc<dyn Fn(BaseContext) -> C + Send + Sync + 'static>,
    ) -> Self {
        let commands_meta = Arc::new(commands.meta());
        Self {
            prefix: Arc::new(prefix),
            client,
            commands: Arc::new(commands),
            commands_meta,
            build_context,
        }
    }

    pub async fn register(self, client: Client) {
        let self_ = Arc::new(self);
        client
            .register_event_handler(move |message, room| {
                let self_ = self_.clone();
                async move {
                    self_.on_room_message(room, message).await;
                }
            })
            .await;
    }

    async fn on_room_message(&self, room: Room, message: SyncMessageEvent<MessageEventContent>) {
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

        let rest = match msg_body.strip_prefix(self.prefix.as_str()) {
            Some(rest) => rest,
            None => return,
        };

        let (cmd, rest) = match self.commands.find_command_parsing(rest) {
            Some(x) => x,
            None => return,
        };

        let base_ctx = BaseContext {
            client: self.client.clone(),
            author: message.sender.clone(),
            room,
            original_event: message.clone(),
            root: self.commands_meta.clone(),
        };

        let ctx = (self.build_context)(base_ctx);

        let _ = cmd.invoke(ctx, rest).await;
    }
}
