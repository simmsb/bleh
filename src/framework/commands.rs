use std::{collections::HashMap, error::Error, future::Future, pin::Pin, str::FromStr, sync::Arc};

pub trait Parameter<C>
where
    Self: Sized,
{
    const INFO: &'static str;
    const VISIBLE: bool;

    fn parse<'a>(ctx: &C, input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>>;
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
        impl<C> Parameter<C> for $T {
            const INFO: &'static str = stringify!($T);
            const VISIBLE: bool = true;

            fn parse<'a>(_ctx: &C, input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>> {
                nom::sequence::delimited(
                    nom::character::complete::multispace0,
                    $fn,
                    nom::character::complete::multispace0,
                )(input)
                .map_err(|e: nom::Err<nom::error::Error<_>>| e.into())
            }
        }
    };
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

#[derive(Clone)]
pub struct Remainder(pub String);

impl<C> Parameter<C> for Remainder {
    const INFO: &'static str = "String*";

    const VISIBLE: bool = true;

    fn parse<'a>(_ctx: &C, input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>> {
        Ok(("", Self(input.to_string())))
    }
}

pub struct ViaFromStr<T>(pub T);

impl<T: FromStr, C> Parameter<C> for ViaFromStr<T>
where
    <T as FromStr>::Err: Error + 'static,
{
    const INFO: &'static str = std::any::type_name::<T>();

    const VISIBLE: bool = true;

    fn parse<'a>(ctx: &C, input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>> {
        let (input, chunk) = <String as Parameter<C>>::parse(ctx, input)?;
        let res = chunk.parse()?;
        Ok((input, ViaFromStr(res)))
    }
}

impl<T: Parameter<C>, C> Parameter<C> for Vec<T> {
    const INFO: &'static str = str_concat(str_concat("Vec<", T::INFO).as_str(), ">").as_str();

    const VISIBLE: bool = T::VISIBLE;

    fn parse<'a>(ctx: &C, mut input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>> {
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

impl<T: Parameter<C>, C, const NAME: &'static str> Parameter<C> for Named<T, NAME> {
    const INFO: &'static str = str_concat(str_concat(NAME, ": ").as_str(), T::INFO).as_str();
    const VISIBLE: bool = T::VISIBLE;

    fn parse<'a>(ctx: &C, input: &'a str) -> Result<(&'a str, Self), Box<dyn Error + 'a>> {
        let (input, v) = T::parse(ctx, input)?;
        Ok((input, Named(v)))
    }
}

pub trait ReifyParameterMeta<C> {
    fn reify_inner(out: &mut Vec<ParameterMeta>);
    fn reify() -> Vec<ParameterMeta> {
        let mut v = Vec::new();
        Self::reify_inner(&mut v);
        v
    }
}

impl<T: Parameter<C>, C, U> ReifyParameterMeta<C> for frunk::HCons<T, U>
where
    U: ReifyParameterMeta<C>,
{
    fn reify_inner(out: &mut Vec<ParameterMeta>) {
        out.push(T::meta());
        U::reify_inner(out);
    }
}

impl<C> ReifyParameterMeta<C> for frunk::HNil {
    fn reify_inner(_out: &mut Vec<ParameterMeta>) {}
}

#[derive(derivative::Derivative)]
#[derivative(Clone(bound = ""))]
#[allow(clippy::type_complexity)]
pub struct ErasedCommand<C> {
    pub meta: CommandMeta,
    invoke: Arc<
        dyn for<'a> Fn(
                C,
                &'a str,
            ) -> Result<
                (&'a str, Pin<Box<dyn Future<Output = ()> + Send + 'static>>),
                Box<dyn Error + 'a>,
            > + Send
            + Sync
            + 'static,
    >,
}

impl<C> ErasedCommand<C> {
    pub async fn invoke<'i>(&self, ctx: C, input: &'i str) -> Result<(), Box<dyn Error + 'i>> {
        let (_, fut) = (self.invoke)(ctx, input)?;

        fut.await;

        Ok(())
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.meta.description = Some(description.to_owned());
        self
    }
}

#[derive(Clone)]
pub struct CommandMeta {
    pub description: Option<String>,
    pub params: Vec<ParameterMeta>,
}

impl CommandMeta {
    pub fn visible_params(&self) -> impl Iterator<Item = &str> {
        self.params.iter().filter(|m| m.visible).map(|m| m.info)
    }

    pub fn format_params(&self) -> String {
        self.visible_params().collect::<Vec<_>>().join(", ")
    }
}

#[async_trait::async_trait]
pub trait Command<P, C> {
    fn parse<'a>(ctx: &'_ C, input: &'a str) -> Result<(&'a str, P), Box<dyn Error + 'a>>;

    async fn invoke(self, ctx: C, params: P);

    fn into_erased(self, description: Option<String>) -> ErasedCommand<C>
    where
        Self: Sized + Clone + Send + Sync + 'static,
        P: ReifyParameterMeta<C>,
    {
        ErasedCommand {
            meta: CommandMeta {
                description,
                params: P::reify(),
            },
            invoke: Arc::new(move |ctx, input| {
                let (input, params): (&str, P) = Self::parse(&ctx, input)?;

                Ok((input, self.clone().invoke(ctx, params)))
            }),
        }
    }
}

#[async_trait::async_trait]
impl<F, C, Fut> Command<frunk::HList![], C> for F
where
    F: FnOnce(C) -> Fut + Clone + Send + Sync + 'static,
    C: Send + 'static,
    Fut: std::future::Future<Output = ()> + Send,
{
    fn parse<'a>(
        _: &'_ C,
        input: &'a str,
    ) -> Result<(&'a str, frunk::HList![]), Box<dyn Error + 'a>> {
        Ok((input, frunk::hlist!()))
    }

    async fn invoke(self, ctx: C, _: frunk::HList![]) {
        self(ctx).await;
    }
}

macro_rules! doit {
    ($dummy:ident $(,)?) => {};
    ($dummy:ident, $($Y:ident),*) => {
        doit!($($Y),*);

        #[async_trait::async_trait]
        #[allow(non_snake_case)]
        impl<F, C, Fut, $($Y),*> Command<frunk::HList![$($Y),*], C> for F
            where F: FnOnce(C, $($Y),*) -> Fut + Clone + Send + Sync + 'static,
                  C: Send + 'static,
                  Fut: std::future::Future<Output = ()> + Send,
                  $($Y: Parameter<C> + Send + 'static),*
        {
            fn parse<'a>(ctx: &'_ C, input: &'a str) -> Result<(&'a str, frunk::HList![$($Y),*]), Box<dyn Error + 'a>> {
                $(
                    let (input, $Y) = $Y::parse(ctx, input)?;
                )*

                Ok((input, frunk::hlist![$($Y),*]))
            }

            async fn invoke(self, ctx: C, params: frunk::HList![$($Y),*]) {
                let frunk::hlist_pat!($($Y),*) = params;

                self(ctx, $($Y),*).await;
            }
        }
    }
}

doit!(dummy, T0, T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);

#[derive(derivative::Derivative)]
#[derivative(Clone(bound = ""), Default(bound = ""))]
pub struct Group<C> {
    pub description: Option<String>,
    pub inner: HashMap<String, GroupOrCommand<C>>,
    pub fallback: Option<ErasedCommand<C>>,
}

impl<C> Group<C> {
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_owned());
        self
    }
}

#[derive(Clone)]
pub struct GroupMeta {
    pub description: Option<String>,
    pub inner: HashMap<String, GroupOrCommandMeta>,
    pub fallback: Option<CommandMeta>,
}

impl GroupMeta {
    /// Find a command or group given a path, this doesn't peek into Group.fallback
    pub fn find_thing<'a>(&'a self, path: &[&str]) -> Option<GroupOrCommandMetaRef<'a>> {
        match *path {
            [] => Some(GroupOrCommandMetaRef::Group(self)),
            [x, ref xs @ ..] => match self.inner.get(x) {
                Some(GroupOrCommandMeta::Command(c)) => Some(GroupOrCommandMetaRef::Command(c)),
                Some(GroupOrCommandMeta::Group(g)) => g.find_thing(xs),
                None => Some(GroupOrCommandMetaRef::Group(self)),
            },
        }
    }
}

fn next_word(s: &str) -> Option<(&str, &str)> {
    if s.trim().is_empty() {
        None
    } else {
        s.split_once(char::is_whitespace).or(Some((s, "")))
    }
}

impl<C> Group<C> {
    fn add_command(&mut self, name: &str, command: ErasedCommand<C>) {
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

    fn add_group(&mut self, name: &str, group: Group<C>) {
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
    ) -> Option<(&'a ErasedCommand<C>, &'b str)> {
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
    pub fn find_command(&self, path: &[&str]) -> Option<&ErasedCommand<C>> {
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
    pub fn find_thing<'a>(&'a self, path: &[&str]) -> Option<GroupOrCommandRef<'a, C>> {
        match *path {
            [] => Some(GroupOrCommandRef::Group(self)),
            [x, ref xs @ ..] => match self.inner.get(x) {
                Some(GroupOrCommand::Command(c)) => Some(GroupOrCommandRef::Command(c)),
                Some(GroupOrCommand::Group(g)) => g.find_thing(xs),
                None => Some(GroupOrCommandRef::Group(self)),
            },
        }
    }

    pub fn meta(&self) -> GroupMeta {
        GroupMeta {
            description: self.description.clone(),
            inner: self
                .inner
                .iter()
                .map(|(k, v)| (k.clone(), v.meta()))
                .collect(),
            fallback: self.fallback.as_ref().map(|c| c.meta.clone()),
        }
    }
}

#[derive(derivative::Derivative)]
#[derivative(Clone(bound = ""))]
#[derive(enum_as_inner::EnumAsInner)]
pub enum GroupOrCommand<C> {
    Command(ErasedCommand<C>),
    Group(Group<C>),
}

impl<C> GroupOrCommand<C> {
    fn meta(&self) -> GroupOrCommandMeta {
        match self {
            GroupOrCommand::Command(c) => GroupOrCommandMeta::Command(c.meta.clone()),
            GroupOrCommand::Group(g) => GroupOrCommandMeta::Group(g.meta()),
        }
    }
}

// #[derive(derivative::Derivative)]
// #[derivative(Clone(bound=""))]
#[derive(enum_as_inner::EnumAsInner)]
pub enum GroupOrCommandRef<'a, C> {
    Command(&'a ErasedCommand<C>),
    Group(&'a Group<C>),
}

#[derive(Clone, enum_as_inner::EnumAsInner)]
pub enum GroupOrCommandMeta {
    Command(CommandMeta),
    Group(GroupMeta),
}

#[derive(Clone, enum_as_inner::EnumAsInner)]
pub enum GroupOrCommandMetaRef<'a> {
    Command(&'a CommandMeta),
    Group(&'a GroupMeta),
}

#[derive(derivative::Derivative)]
#[derivative(Clone(bound = ""), Default(bound = ""))]
pub struct GroupBuilder<C> {
    root: Group<C>,
}

impl<C> GroupBuilder<C> {
    pub fn new() -> Self {
        GroupBuilder::default()
    }

    pub fn group(&mut self, name: &str, grp: Group<C>) -> &mut Self {
        self.root.add_group(name, grp);
        self
    }

    pub fn command(&mut self, name: &str, cmd: ErasedCommand<C>) -> &mut Self {
        self.root.add_command(name, cmd);
        self
    }

    pub fn done(&mut self) -> Group<C> {
        self.clone().root
    }
}

pub fn cmd<Cmd, P, C>(cmd: Cmd) -> ErasedCommand<C>
where
    Cmd: Command<P, C> + Clone + Send + Sync + 'static,
    P: ReifyParameterMeta<C>,
{
    cmd.into_erased(None)
}
