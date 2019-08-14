use crate::{protocol::Protocol, targets};
use failure::Error;
use futures::Future;

/// A webassembly-based containerized implementation of a protocol.
pub trait Module<T: Protocol + ?Sized + 'static>: Send {
    /// Constructs a new instance of the containerized protocol.
    fn instantiate(&self) -> Box<dyn Future<Item = Box<T>, Error = Error> + Send>;
}

impl<T: Protocol + ?Sized + 'static> dyn Module<T> {
    /// Compiles the protocol from wasm bytes. This is likely costly.
    pub fn compile(
        data: Vec<u8>,
    ) -> impl Future<Item = Box<dyn Module<T> + 'static>, Error = Error> {
        #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
        return targets::native::vessels::WasmerModule::compile(data);
        #[cfg(any(target_arch = "wasm32", target_arch = "asmjs"))]
        return targets::web::vessels::WASMModule::compile(data);
    }
}
