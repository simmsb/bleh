use std::sync::Arc;

use matrix_sdk::{
    room::Joined,
    ruma::{
        api::client::r0::message::send_message_event,
        events::{room::message::RoomMessageEventContent, AnyMessageEventContent, SyncMessageEvent},
        UserId,
    },
    Client,
};

use crate::framework::commands::GroupMeta;

use crate as bleh;

#[ambassador::delegatable_trait]
pub trait Context: Sized {
    fn client(&self) -> &matrix_sdk::Client;
    fn author(&self) -> &matrix_sdk::ruma::UserId;
    fn room(&self) -> &matrix_sdk::room::Joined;
    fn original_event(
        &self,
    ) -> &matrix_sdk::ruma::events::SyncMessageEvent<
        matrix_sdk::ruma::events::room::message::RoomMessageEventContent,
    >;
    fn root(&self) -> &bleh::framework::commands::GroupMeta;
}

#[async_trait::async_trait]
pub trait ContextActions: Context {
    async fn send(&self, msg: &str) -> matrix_sdk::Result<send_message_event::Response> {
        let m = RoomMessageEventContent::text_plain(msg);

        self.room().send(m, None).await
    }

    async fn reply(&self, msg: &str) -> matrix_sdk::Result<send_message_event::Response> {
        let m = RoomMessageEventContent::text_reply_plain(
            msg,
            &self
                .original_event()
                .clone()
                .into_full_event(self.room().room_id().to_owned()),
        );

        self.room().send(m, None).await
    }

    async fn send_html(
        &self,
        plain: &str,
        html: &str,
    ) -> matrix_sdk::Result<send_message_event::Response> {
        let m = AnyMessageEventContent::RoomMessage(RoomMessageEventContent::text_html(plain, html));

        self.room().send(m, None).await
    }

    async fn reply_html(
        &self,
        plain: &str,
        html: &str,
    ) -> matrix_sdk::Result<send_message_event::Response> {
        let m = AnyMessageEventContent::RoomMessage(RoomMessageEventContent::text_reply_html(
            plain,
            html,
            &self
                .original_event()
                .clone()
                .into_full_event(self.room().room_id().to_owned()),
        ));

        self.room().send(m, None).await
    }
}

impl<T: Context> ContextActions for T {}

#[derive(Clone)]
pub struct BaseContext {
    pub client: Client,
    pub author: Box<UserId>,
    pub room: Joined,
    pub original_event: SyncMessageEvent<RoomMessageEventContent>,
    pub root: Arc<GroupMeta>,
}

impl Context for BaseContext {
    fn client(&self) -> &Client {
        &self.client
    }

    fn author(&self) -> &UserId {
        &self.author
    }

    fn room(&self) -> &Joined {
        &self.room
    }

    fn original_event(&self) -> &SyncMessageEvent<RoomMessageEventContent> {
        &self.original_event
    }

    fn root(&self) -> &GroupMeta {
        &self.root
    }
}
