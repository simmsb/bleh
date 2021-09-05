use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use matrix_sdk::{
    room::Joined,
    ruma::{
        api::client::r0::message::send_message_event,
        events::{room::message::MessageEventContent, AnyMessageEventContent, SyncMessageEvent},
        UserId,
    },
};

#[derive(Clone)]
pub struct Context {
    pub author: UserId,
    pub room: Joined,
    pub original_event: SyncMessageEvent<MessageEventContent>,
    pub root: Arc<Group<'static>>,
}

impl Context {
    pub async fn send(&self, msg: &str) -> matrix_sdk::Result<send_message_event::Response> {
        let m = AnyMessageEventContent::RoomMessage(MessageEventContent::text_plain(msg));

        self.room.send(m, None).await
    }

    pub async fn reply(&self, msg: &str) -> matrix_sdk::Result<send_message_event::Response> {
        let m = AnyMessageEventContent::RoomMessage(MessageEventContent::text_reply_plain(
            msg,
            &self
                .original_event
                .clone()
                .into_full_event(self.room.room_id().clone()),
        ));

        self.room.send(m, None).await
    }
}

pub trait Parameter
where
    Self: Sized,
{
    const INFO: &'static str;
    const VISIBLE: bool;

    fn parse<'a>(ctx: &Context, input: &'a str) -> nom::IResult<&'a str, Self>;
    fn meta() -> ParameterMeta {
        ParameterMeta {
            info: Self::INFO,
            visible: Self::VISIBLE,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParameterMeta {
    pub info: &'static str,
    pub visible: bool,
}

macro_rules! p_via_nom {
    ($T:ty, $fn:expr) => {
        impl Parameter for $T {
            const INFO: &'static str = stringify!($T);
            const VISIBLE: bool = true;

            fn parse<'a>(_ctx: &Context, input: &'a str) -> nom::IResult<&'a str, Self> {
                nom::sequence::delimited(
                    nom::character::complete::multispace0,
                    $fn,
                    nom::character::complete::multispace0,
                )(input)
            }
        }
    };
}

impl Parameter for Context {
    const INFO: &'static str = "Context";
    const VISIBLE: bool = false;

    fn parse<'a>(ctx: &Context, input: &'a str) -> nom::IResult<&'a str, Self> {
        nom::IResult::Ok((input, ctx.clone()))
    }
}

p_via_nom!(u64, nom::character::complete::u64);
p_via_nom!(i64, nom::character::complete::i64);
p_via_nom!(u32, nom::character::complete::u32);
p_via_nom!(i32, nom::character::complete::i32);
p_via_nom!(u16, nom::character::complete::u16);
p_via_nom!(i16, nom::character::complete::i16);
p_via_nom!(u8, nom::character::complete::u8);
p_via_nom!(i8, nom::character::complete::i8);
p_via_nom!(f32, nom::number::complete::float);

p_via_nom!(
    String,
    nom::combinator::map(
        nom::branch::alt((
            nom::sequence::delimited(
                nom::bytes::complete::tag("\""),
                nom::bytes::complete::take_until("\""),
                nom::bytes::complete::tag("\"")
            ),
            nom::bytes::complete::take_till1(|c: char| c.is_whitespace())
        )),
        String::from
    )
);

impl<T: Parameter> Parameter for Vec<T> {
    const INFO: &'static str = str_concat(str_concat("Vec<", T::INFO).as_str(), ">").as_str();

    const VISIBLE: bool = T::VISIBLE;

    fn parse<'a>(ctx: &Context, mut input: &'a str) -> nom::IResult<&'a str, Self> {
        let mut out = Vec::new();

        while let Ok((new_input, v)) = T::parse(ctx, input) {
            input = new_input;
            out.push(v);
        }

        Ok((input, out))
    }
}

pub struct Named<T, const NAME: &'static str>(pub T);

struct HelpMe {
    s: [u8; 255],
    l: usize,
}

impl HelpMe {
    const fn as_str(&self) -> &str {
        unsafe {
            let s = &*std::ptr::slice_from_raw_parts(&self.s as *const u8, self.l);
            std::str::from_utf8_unchecked(s)
        }
    }
}

const fn str_concat(l: &'static str, r: &'static str) -> HelpMe {
    let mut out = [0u8; 255];

    // The compiler crashes if I try to make the out length const generic

    assert!(l.len() + r.len() < out.len());

    let l = l.as_bytes();
    let r = r.as_bytes();

    let mut i = 0;
    while i < l.len() {
        out[i] = l[i];
        i += 1;
    }

    let mut j = 0;
    while j < r.len() {
        out[i + j] = r[j];
        j += 1;
    }

    HelpMe {
        s: out,
        l: l.len() + r.len(),
    }
}

impl<T: Parameter, const NAME: &'static str> Parameter for Named<T, NAME> {
    const INFO: &'static str = str_concat(str_concat(NAME, ": ").as_str(), T::INFO).as_str();
    const VISIBLE: bool = T::VISIBLE;

    fn parse<'a>(ctx: &Context, input: &'a str) -> nom::IResult<&'a str, Self> {
        let (input, v) = T::parse(ctx, input)?;
        nom::IResult::Ok((input, Named(v)))
    }
}

pub trait ReifyParameterMeta {
    fn reify_inner(out: &mut Vec<ParameterMeta>);
    fn reify() -> Vec<ParameterMeta> {
        let mut v = Vec::new();
        Self::reify_inner(&mut v);
        v
    }
}

impl<T: Parameter, U: ReifyParameterMeta> ReifyParameterMeta for frunk::HCons<T, U> {
    fn reify_inner(out: &mut Vec<ParameterMeta>) {
        out.push(T::meta());
        U::reify_inner(out);
    }
}

impl ReifyParameterMeta for frunk::HNil {
    fn reify_inner(_out: &mut Vec<ParameterMeta>) {}
}

#[async_trait::async_trait]
pub trait Command<P> {
    fn parse(ctx: Context, input: &str) -> nom::IResult<&str, P>;

    async fn invoke(self, params: P);

    fn into_erased<'c>(self) -> ErasedCommand<'c>
    where
        Self: Sized + Clone + Send + Sync + 'c,
        P: ReifyParameterMeta,
    {
        ErasedCommand {
            params: P::reify(),
            invoke: Arc::new(move |ctx, input| {
                let (input, params): (&str, P) = Self::parse(ctx, input)?;

                nom::IResult::Ok((input, self.clone().invoke(params)))
            }),
        }
    }
}

#[derive(Clone)]
#[allow(clippy::type_complexity)]
pub struct ErasedCommand<'c> {
    params: Vec<ParameterMeta>,
    invoke: Arc<
        dyn Fn(Context, &str) -> nom::IResult<&str, Pin<Box<dyn Future<Output = ()> + Send + 'c>>>
            + Send
            + Sync
            + 'c,
    >,
}

impl<'c> ErasedCommand<'c> {
    pub async fn invoke<'i>(
        &self,
        ctx: Context,
        input: &'i str,
    ) -> nom::IResult<(), (), nom::error::Error<&'i str>> {
        let (_, fut) = (self.invoke)(ctx, input)?;

        fut.await;

        nom::IResult::Ok(((), ()))
    }

    pub fn format_params(&self) -> String {
        self.params
            .iter()
            .filter(|m| m.visible)
            .map(|m| m.info)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[async_trait::async_trait]
impl<F, Fut> Command<frunk::HList![]> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    fn parse(_: Context, input: &str) -> nom::IResult<&str, frunk::HList![]> {
        nom::IResult::Ok((input, frunk::hlist!()))
    }

    async fn invoke(self, _: frunk::HList![]) {
        self().await;
    }
}

macro_rules! doit {
    ($dummy:ident $(,)?) => {};
    ($dummy:ident, $($Y:ident),*) => {
        doit!($($Y),*);

        #[async_trait::async_trait]
        #[allow(non_snake_case)]
        impl<F, Fut, $($Y),*> Command<frunk::HList![$($Y),*]> for F
            where F: FnOnce($($Y),*) -> Fut + Clone + Send + Sync + 'static,
                  Fut: std::future::Future<Output = ()> + Send,
                  $($Y: Parameter + Send + 'static),*
        {
            fn parse(ctx: Context, input: &str) -> nom::IResult<&str, frunk::HList![$($Y),*]> {
                $(
                    let (input, $Y) = $Y::parse(&ctx, input)?;
                )*

                nom::IResult::Ok((input, frunk::hlist![$($Y),*]))
            }

            async fn invoke(self, params: frunk::HList![$($Y),*]) {
                let frunk::hlist_pat!($($Y),*) = params;

                self($($Y),*).await;
            }
        }
    }
}

doit!(dummy, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);

#[derive(Default, Clone)]
pub struct Group<'c> {
    pub inner: HashMap<String, GroupOrCommand<'c>>,
    pub fallback: Option<ErasedCommand<'c>>,
}

fn next_word(s: &str) -> Option<(&str, &str)> {
    if s.trim().is_empty() {
        None
    } else {
        s.split_once(char::is_whitespace).or(Some((s, "")))
    }
}

impl<'c> Group<'c> {
    fn add_command(&mut self, name: &str, command: ErasedCommand<'c>) {
        match self.inner.get_mut(name) {
            Some(GroupOrCommand::Command(_)) => {
                panic!("Command {} already exists", name)
            }
            Some(GroupOrCommand::Group(Group {
                fallback: Some(_), ..
            })) => {
                panic!("Fallback command for group {} already exists", name)
            }
            Some(GroupOrCommand::Group(Group { fallback, .. })) => {
                *fallback = Some(command);
                return;
            }
            None => (),
        };

        self.inner
            .insert(name.to_owned(), GroupOrCommand::Command(command));
    }

    fn add_group(&mut self, name: &str, group: Group<'c>) {
        match self.inner.get_mut(name) {
            Some(GroupOrCommand::Group(_)) => {
                panic!("Group with name {} already exists", name)
            }
            Some(v @ &mut GroupOrCommand::Command(_)) => {
                if group.fallback.is_some() {
                    panic!("Can't add group {}, a command already exists and this group has it's fallback set", name);
                }

                let c = std::mem::replace(v, GroupOrCommand::Group(group));
                v.as_group_mut().unwrap().fallback = c.into_command().ok();
                return;
            }
            None => (),
        }

        self.inner
            .insert(name.to_owned(), GroupOrCommand::Group(group));
    }

    pub fn find_command_parsing<'a, 'b>(
        &'a self,
        input: &'b str,
    ) -> Option<(&'a ErasedCommand<'c>, &'b str)> {
        if let Some((x, xs)) = next_word(input) {
            match self.inner.get(x) {
                Some(GroupOrCommand::Command(c)) => Some((c, xs)),
                Some(GroupOrCommand::Group(g)) => g.find_command_parsing(xs),
                None => self.fallback.as_ref().map(|f| (f, input)),
            }
        } else {
            self.fallback.as_ref().map(|f| (f, input))
        }
    }

    /// Find a command given a path
    pub fn find_command(&self, path: &[&str]) -> Option<&ErasedCommand<'c>> {
        match *path {
            [] => self.fallback.as_ref(),
            [x, ref xs @ ..] => match self.inner.get(x) {
                Some(GroupOrCommand::Command(c)) => Some(c),
                Some(GroupOrCommand::Group(g)) => g.find_command(xs),
                None => self.fallback.as_ref(),
            },
        }
    }

    /// Find a command or group given a path, this doesn't peek into Group.fallback
    pub fn find_thing<'a>(&'a self, path: &[&str]) -> Option<GroupOrCommandRef<'a, 'c>> {
        match *path {
            [] => Some(GroupOrCommandRef::Group(self)),
            [x, ref xs @ ..] => match self.inner.get(x) {
                Some(GroupOrCommand::Command(c)) => Some(GroupOrCommandRef::Command(c)),
                Some(GroupOrCommand::Group(g)) => g.find_thing(xs),
                None => Some(GroupOrCommandRef::Group(self)),
            },
        }
    }
}

#[derive(enum_as_inner::EnumAsInner, Clone)]
pub enum GroupOrCommand<'c> {
    Command(ErasedCommand<'c>),
    Group(Group<'c>),
}

#[derive(enum_as_inner::EnumAsInner)]
pub enum GroupOrCommandRef<'a, 'c> {
    Command(&'a ErasedCommand<'c>),
    Group(&'a Group<'c>),
}

#[derive(Default, Clone)]
pub struct CommandBuilder<'c> {
    root: Group<'c>,
}

impl<'c> CommandBuilder<'c> {
    pub fn new() -> Self {
        CommandBuilder::default()
    }

    pub fn group<F>(&mut self, name: &str, f: F) -> &mut Self
    where
        F: FnOnce(&mut CommandBuilder<'c>),
    {
        let mut inner = CommandBuilder::default();
        f(&mut inner);
        self.root.add_group(name, inner.done());
        self
    }

    pub fn command<C, P>(&mut self, name: &str, c: C) -> &mut Self
    where
        C: Command<P> + Clone + Send + Sync + 'c,
        P: ReifyParameterMeta,
    {
        let ec = c.into_erased();
        self.root.add_command(name, ec);
        self
    }

    pub fn done(&mut self) -> Group<'c> {
        self.clone().root
    }
}
