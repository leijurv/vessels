pub mod id_channel;
pub use id_channel::IdChannel;

use crate::{kind::Future, Kind};

use failure::{Error, Fail};
use futures::{Sink, Stream};
use serde::{
    de::{DeserializeOwned, DeserializeSeed},
    Deserialize, Serialize,
};
use std::{
    fmt::{self, Display, Formatter},
    marker::Unpin,
};

#[derive(Serialize, Deserialize, Debug, PartialEq, Hash, Eq, Clone, Copy)]
#[repr(transparent)]
pub struct ForkHandle(pub(crate) u32);

impl Display for ForkHandle {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub trait Fork: Sync + Send + 'static {
    fn fork<K: Kind>(&self, kind: K) -> Future<Result<ForkHandle, K::DeconstructError>>;
    fn get_fork<K: Kind>(&self, fork_ref: ForkHandle) -> Future<Result<K, K::ConstructError>>;
}

#[derive(Debug)]
pub struct ChannelError(pub(crate) Error);

impl<T: Fail> From<T> for ChannelError {
    fn from(input: T) -> Self {
        ChannelError(input.into())
    }
}

impl Display for ChannelError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

pub trait Channel<
    I: Serialize + DeserializeOwned + Sync + Send + 'static,
    O: Serialize + DeserializeOwned + Sync + Send + 'static,
>: Stream<Item = I> + Sink<O, Error = ChannelError> + Fork + Send + Sync + Unpin
{
}

pub trait Shim<'a, T: Target<'a, K>, K: Kind>:
    Context<'a, Item = <T as Context<'a>>::Item>
{
    fn complete<
        C: Sync
            + Send
            + Stream<Item = <T as Context<'a>>::Item>
            + Sink<<T as Context<'a>>::Item>
            + 'static,
    >(
        self,
        input: C,
    ) -> Future<Result<K, K::ConstructError>>;
}

pub trait Target<'a, K: Kind>: Context<'a> + Sized {
    type Shim: Shim<'a, Self, K>;

    fn new_with(kind: K) -> Future<Self>
    where
        K::DeconstructFuture: Send;

    fn new_shim() -> Self::Shim;
}

pub trait Waiter {
    fn wait_for(&self, data: String) -> Future<()>;
}

pub trait Context<'de> {
    type Item: Serialize + 'static;
    type Target: Waiter + DeserializeSeed<'de, Value = Self::Item> + Clone + Sync + Send + 'static;

    fn context(&self) -> Self::Target;
}

pub trait OnTo: Kind {
    fn on_to<'a, T: Target<'a, Self>>(self) -> Future<T>
    where
        Self: Send + 'static,
        Self::DeconstructFuture: Sync + Send;
}

impl<K: Kind> OnTo for K {
    fn on_to<'a, T: Target<'a, Self>>(self) -> Future<T>
    where
        Self: Send + 'static,
        Self::DeconstructFuture: Sync + Send,
    {
        T::new_with(self)
    }
}
