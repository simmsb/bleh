use std::sync::Arc;

use matrix_sdk::{
    room::Room,
    ruma::events::{
        room::message::{MessageEventContent, MessageType, TextMessageEventContent},
        SyncMessageEvent,
    },
    Client,
};
use rrule::RRule;
use sqlx::SqlitePool;

use crate::commands::{CommandBuilder, Context, Group, Named, Remainder};

#[derive(Clone)]
pub struct OnMessage {
    prefix: Arc<String>,
    client: Client,
    commands: Arc<Group<'static>>,
    pool: SqlitePool,
}

impl OnMessage {
    pub fn new(prefix: String, client: Client, pool: SqlitePool) -> Self {
        Self {
            prefix: Arc::new(prefix),
            client,
            commands: Arc::new(make_commands()),
            pool,
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

        let ctx = Context {
            client: self.client.clone(),
            author: message.sender.clone(),
            room,
            original_event: message.clone(),
            root: self.commands.clone(),
            pool: self.pool.clone(),
        };

        let _ = cmd.invoke(ctx, rest).await;
    }
}

#[derive(askama::Template)]
#[template(path = "command_help.html")]
struct HtmlCommandHelpTemplate<'a> {
    name: &'a str,
    params: &'a [&'a str],
}

impl<'a> HtmlCommandHelpTemplate<'a> {
    fn new(name: &'a str, params: &'a [&'a str]) -> Self {
        Self { name, params }
    }
}

#[derive(askama::Template)]
#[template(path = "command_help.txt")]
struct PlainCommandHelpTemplate<'a> {
    name: &'a str,
    params: &'a [&'a str],
}

impl<'a> PlainCommandHelpTemplate<'a> {
    fn new(name: &'a str, params: &'a [&'a str]) -> Self {
        Self { name, params }
    }
}

#[derive(askama::Template)]
#[template(path = "group_help.html")]
struct HtmlGroupHelpTemplate<'a> {
    name: &'a str,
    subcommands: &'a [&'a str],
    fallback: Option<&'a [&'a str]>,
}

impl<'a> HtmlGroupHelpTemplate<'a> {
    fn new(name: &'a str, subcommands: &'a [&'a str], fallback: Option<&'a [&'a str]>) -> Self {
        Self {
            name,
            subcommands,
            fallback,
        }
    }
}

#[derive(askama::Template)]
#[template(path = "group_help.txt")]
struct PlainGroupHelpTemplate<'a> {
    name: &'a str,
    subcommands: &'a [&'a str],
    fallback: Option<&'a [&'a str]>,
}

impl<'a> PlainGroupHelpTemplate<'a> {
    fn new(name: &'a str, subcommands: &'a [&'a str], fallback: Option<&'a [&'a str]>) -> Self {
        Self {
            name,
            subcommands,
            fallback,
        }
    }
}

fn make_commands() -> Group<'static> {
    use askama::Template;

    CommandBuilder::new()
        .command("hi", |c: Context| async move {
            let _ = c.send("Hi").await;
        })
        .command("fart", |c: Context| async move {
            let _ = c
                .send_html(
                    "*farts*",
                    "<h1><span data-mx-color=\"#7a5901\">*farts*</span></h1>",
                )
                .await;
        })
        .command(
            "recur",
            |c: Context,
             p: SqlitePool,
             Named(rule): Named<String, "rule">,
             Named(Remainder(message)): Named<Remainder, "message">| async move {
                let _parsed_rule: RRule = match rule.parse() {
                    Ok(rule) => rule,
                    Err(e) => {
                        let _ = c.reply(&format!("Couldn't parse rule: {:?}", e)).await;
                        return;
                    }
                };

                let room_id = c.room.room_id().as_ref();
                let author_id = c.author.as_ref();
                let id = sqlx::query!(
                    r#"INSERT INTO rrules ( rule, message, channel, userid )
                       VALUES ( ?1, ?2, ?3, ?4 )"#,
                    rule,
                    message,
                    room_id,
                    author_id,
                )
                .execute(&p)
                .await
                .unwrap()
                .last_insert_rowid();

                let r = crate::rrules::RRule {
                    id,
                    rule,
                    message,
                    channel: room_id.to_owned(),
                    userid: author_id.to_owned(),
                };

                let _ = c.reply("Sure thing dude").await;

                tokio::spawn(async move {
                    r.perform(c.client).await;
                });
            },
        )
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
                        let name = path.join(" ");
                        let params = cmd.visible_params().collect::<Vec<_>>();
                        let plain = PlainCommandHelpTemplate::new(&name, &params)
                            .render()
                            .unwrap();
                        let html = HtmlCommandHelpTemplate::new(&name, &params)
                            .render()
                            .unwrap();
                        let _ = c.send_html(&plain, &html).await;
                    }
                    crate::commands::GroupOrCommandRef::Group(grp) => {
                        let name = path.join(" ");
                        let subcommands = grp.inner.keys().map(|k| k.as_str()).collect::<Vec<_>>();
                        let fallback = grp
                            .fallback
                            .as_ref()
                            .map(|cmd| cmd.visible_params().collect::<Vec<_>>());
                        let plain =
                            PlainGroupHelpTemplate::new(&name, &subcommands, fallback.as_deref())
                                .render()
                                .unwrap();
                        let html =
                            HtmlGroupHelpTemplate::new(&name, &subcommands, fallback.as_deref())
                                .render()
                                .unwrap();
                        let _ = c.send_html(&plain, &html).await;
                    }
                }
            },
        )
        .done()
}
