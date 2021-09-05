use std::sync::Arc;

use matrix_sdk::{
    room::Room,
    ruma::events::{
        room::message::{MessageEventContent, MessageType, TextMessageEventContent},
        SyncMessageEvent,
    },
    Client, EventHandler,
};

use crate::commands::{CommandBuilder, Context, Group, Named};

pub struct OnMessage {
    prefix: String,
    client: Client,
    commands: Arc<Group<'static>>,
}

impl OnMessage {
    pub fn new(prefix: String, client: Client) -> Self {
        Self {
            prefix,
            client,
            commands: Arc::new(make_commands()),
        }
    }

    pub fn into_eh(self) -> Box<dyn EventHandler> {
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

        let rest = match msg_body.strip_prefix(&self.prefix) {
            Some(rest) => rest,
            None => return,
        };

        let (cmd, rest) = match self.commands.find_command_parsing(rest) {
            Some(x) => x,
            None => return,
        };

        let ctx = Context {
            author: message.sender.clone(),
            room,
            original_event: message.clone(),
            root: self.commands.clone(),
        };

        let _ = cmd.invoke(ctx, rest).await;
    }
}

fn make_commands() -> Group<'static> {
    CommandBuilder::new()
        .command("hi", |c: Context| async move {
            let _ = c.send("Hi").await;
        })
        .group("grp", |g| {
            g.command("a", |c: Context| async move {
                let _ = c.reply("A").await;
            })
            .command("b", |c: Context, Named(v): Named<String, "v">| async move {
                let _ = c.reply(&format!("B: {}", v)).await;
            });
        })
        .command(
            "help",
            |c: Context, Named(path): Named<Vec<String>, "path">| async move {
                let path = path.iter().map(|s| s.as_str()).collect::<Vec<_>>();
                let thing = match c.root.find_thing(&path) {
                    Some(thing) => thing,
                    None => {
                        let _ = c.reply(&format!("Couldn't find {}", path.join(" "))).await;
                        return;
                    }
                };

                match thing {
                    crate::commands::GroupOrCommandRef::Command(cmd) => {
                        let _ = c
                            .send(&format!(
                                "Command: {}\nparams: {}",
                                path.join(" "),
                                cmd.format_params()
                            ))
                            .await;
                    }
                    crate::commands::GroupOrCommandRef::Group(grp) => {
                        let _ = c
                            .send(&format!(
                                "Group: {}\nSubcommands:\n{}\nFallback command: {}",
                                path.join(" "),
                                grp.inner
                                    .keys()
                                    .map(|cmd| format!("- {}", cmd))
                                    .collect::<Vec<_>>()
                                    .join("\n"),
                                grp.fallback
                                    .as_ref()
                                    .map(|cmd| cmd.format_params())
                                    .unwrap_or("None".to_owned())
                            ))
                            .await;
                    }
                }
            },
        )
        .done()
}
