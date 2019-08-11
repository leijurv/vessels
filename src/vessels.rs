use crate::{protocol::Protocol, targets};
use failure::Error;
use futures::Future;

pub trait Module<T: Protocol + ?Sized + 'static> {
    fn instantiate(&self) -> Box<dyn Future<Item = Box<T>, Error = Error>>;
}

impl<T: Protocol + ?Sized + 'static> dyn Module<T> {
    pub fn compile(data: &'_ [u8]) -> impl Future<Item = Box<dyn Module<T>>, Error = Error> {
        targets::native::vessels::WasmerModule::compile(data)
    }
}
