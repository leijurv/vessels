use super::WrappedError;
use crate::{
    channel::{Channel, ForkHandle},
    kind,
    kind::{ConstructResult, DeconstructResult, Future},
    Kind,
};

use anyhow::Error;
use core::fmt::{self, Debug, Display, Formatter};
use futures::{SinkExt, StreamExt};
use std::error::Error as StdError;
use void::Void;

#[derive(Kind)]
struct ErrorShim {
    source: Option<Box<ErrorShim>>,
    debug: String,
    display: String,
}

impl Display for ErrorShim {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display)
    }
}

impl Debug for ErrorShim {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.debug)
    }
}

impl StdError for ErrorShim {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source
            .as_ref()
            .map(|item| item.as_ref() as &dyn StdError)
    }
}

impl<T: StdError + ?Sized> From<&T> for ErrorShim {
    fn from(input: &T) -> Self {
        ErrorShim {
            source: input.source().map(|e| Box::new(ErrorShim::from(e))),
            debug: format!("{:?}", input),
            display: format!("{}", input),
        }
    }
}

#[kind]
impl Kind for Error {
    type ConstructItem = ForkHandle;
    type ConstructError = WrappedError<Void>;
    type ConstructFuture = Future<ConstructResult<Self>>;
    type DeconstructItem = ();
    type DeconstructError = WrappedError<Void>;
    type DeconstructFuture = Future<DeconstructResult<Self>>;
    fn deconstruct<C: Channel<Self::DeconstructItem, Self::ConstructItem>>(
        self,
        mut channel: C,
    ) -> Self::DeconstructFuture {
        Box::pin(async move {
            Ok(channel
                .send(channel.fork(ErrorShim::from(&*self)).await?)
                .await
                .map_err(WrappedError::Send)?)
        })
    }
    fn construct<C: Channel<Self::ConstructItem, Self::DeconstructItem>>(
        mut channel: C,
    ) -> Self::ConstructFuture {
        Box::pin(async move {
            let handle = channel.next().await.ok_or(WrappedError::Insufficient {
                got: 0,
                expected: 1,
            })?;
            Ok(channel.get_fork::<ErrorShim>(handle).await?.into())
        })
    }
}
