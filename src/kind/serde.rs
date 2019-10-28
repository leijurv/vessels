use serde::{de::DeserializeOwned, Serialize};

use futures::Future;

use crate::{channel::Channel, Kind};

use super::{using, AsKind};

use std::ops::Deref;

pub struct Serde<T: Serialize + DeserializeOwned + Send + 'static>(pub T);

impl<T: Serialize + DeserializeOwned + Send + 'static> Serde<T> {
    pub fn new(item: T) -> Self {
        Serde(item)
    }
}

impl<T: Serialize + DeserializeOwned + Send + 'static> Deref for Serde<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Serialize + DeserializeOwned + Send + 'static> From<T> for Serde<T> {
    fn from(item: T) -> Self {
        Serde(item)
    }
}

impl<T: Serialize + DeserializeOwned + Send + 'static> AsKind<using::Serde> for T {
    type Kind = Serde<T>;
    type ConstructFuture = Box<
        dyn Future<Item = Self, Error = <<Serde<T> as Kind>::ConstructFuture as Future>::Error>
            + Send,
    >;

    fn into_kind(self) -> Serde<T> {
        Serde(self)
    }
    fn from_kind(
        future: <Serde<T> as Kind>::ConstructFuture,
    ) -> Box<
        dyn Future<Item = Self, Error = <<Serde<T> as Kind>::ConstructFuture as Future>::Error>
            + Send,
    > {
        Box::new(future.map(|item| item.0))
    }
}

impl<T: Serialize + DeserializeOwned + Send + 'static> Kind for Serde<T> {
    type ConstructItem = T;
    type ConstructFuture = Box<dyn Future<Item = Serde<T>, Error = ()> + Send + 'static>;
    type DeconstructItem = ();
    type DeconstructFuture = Box<dyn Future<Item = (), Error = ()> + Send + 'static>;

    fn deconstruct<C: Channel<Self::DeconstructItem, Self::ConstructItem>>(
        self,
        channel: C,
    ) -> Self::DeconstructFuture {
        Box::new(
            channel
                .send(self.0)
                .and_then(|_| Ok(()))
                .map_err(|_| panic!()),
        )
    }
    fn construct<C: Channel<Self::ConstructItem, Self::DeconstructItem>>(
        channel: C,
    ) -> Self::ConstructFuture {
        Box::new(
            channel
                .into_future()
                .map_err(|e| panic!(e))
                .map(|v| Serde(v.0.unwrap())),
        )
    }
}
