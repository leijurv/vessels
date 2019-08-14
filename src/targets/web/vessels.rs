use crate::{
    executor,
    protocol::{Protocol, Remote},
    Module,
};
use failure::Error;
use futures::{future::ok, lazy, sync::mpsc::channel, Future, Sink, Stream};
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};
use stdweb::{unstable::TryInto, web::TypedArray};

type WebAssemblyModule = stdweb::Value;

pub(crate) struct WASMModule<T: Protocol + ?Sized + 'static> {
    ty: PhantomData<T>,
    module: WebAssemblyModule,
}

impl<T: Protocol + ?Sized + 'static> Module<T> for WASMModule<T> {
    fn instantiate(&self) -> Box<dyn Future<Item = Box<T>, Error = Error> + Send> {
        let (mut sender, receiver) = channel(0);
        let module = self.module.clone();
        executor::spawn(lazy(move || {
            let mem: Arc<Mutex<Option<stdweb::Value>>> = Arc::new(Mutex::new(None));
            let sn: Arc<
                Mutex<Option<Box<dyn Sink<SinkItem = <T as Protocol>::Response, SinkError = ()>>>>,
            > = Arc::new(Mutex::new(None));
            let o_sn = sn.clone();
            let o_mem = mem.clone();
            js! {
                WebAssembly.instantiate(@{module}, {
                    env: {
                        o: @{move |ptr: u32, len: u32| {
                            let guard = o_mem.lock().unwrap();
                            let memory = guard.as_ref().unwrap();
                            let data: TypedArray<u8> = js! {
                                return new Uint8Array(@{memory}.slice(@{ptr}, @{ptr + len}));
                            }.try_into().unwrap();
                            let data: Vec<_> = data.into();
                            let mut guard = o_sn.lock().unwrap();
                            guard.as_mut().unwrap().start_send(serde_cbor::from_slice(&data).unwrap()).unwrap();
                        }}
                    }
                }).then((instance) => {
                    @{move |instance: stdweb::Value, memory: stdweb::Value| {
                        *mem.clone().lock().unwrap() = Some(memory.clone());
                        let (ret, sinkstream) = T::remote().separate();
                        sender.try_send(ret).unwrap();
                        let (sink, stream) = sinkstream.split();
                        *sn.lock().unwrap() = Some(Box::new(sink));
                        executor::spawn(stream.for_each(move |item| {
                            let data: TypedArray<u8> = serde_cbor::to_vec(&item).unwrap().as_slice().into();
                            let len = data.len();
                            js! {
                                (new Uint8Array(@{&memory})).set(@{data});
                                @{&instance}.exports.i(0, @{len});
                            };
                            Ok(())
                        }));
                    }}(instance, instance.exports.memory.buffer)
                });
            };
            Ok(())
        }));
        Box::new(
            receiver
                .take(1)
                .into_future()
                .map_err(|_| failure::err_msg("temp err"))
                .and_then(|module| Ok(module.0.unwrap())),
        )
    }
}

impl<T: Protocol + ?Sized + 'static> WASMModule<T> {
    pub(crate) fn compile(
        data: Vec<u8>,
    ) -> impl Future<Item = Box<dyn Module<T> + 'static>, Error = Error> {
        lazy(move || {
            let (mut sender, receiver) = channel(0);
            let mut e_sender = sender.clone();
            let buffer: TypedArray<u8> = data.as_slice().into();
            js! {
                WebAssembly.compile(@{buffer}).then((module) => {
                    let is_valid = false;
                    WebAssembly.instantiate(module, {
                        env: {
                            o: (ptr, len) => {}
                        }
                    }).then((instance) => {
                        if (instance.exports.s && instance.exports.s.value) {
                            if (instance.exports.memory.buffer.byteLength >= instance.exports.s.value + 8) {
                                is_valid = @{|data: TypedArray<u8>| {
                                    let data: Vec<u8> = data.into();
                                    if data.len() != 8 {
                                        return false;
                                    }
                                    let mut bytes: [u8; 8] = Default::default();
                                    bytes.copy_from_slice(&data);
                                    u64::from_ne_bytes(bytes) == T::DO_NOT_IMPLEMENT_THIS_TRAIT_MANUALLY
                                }}(new Uint8Array(instance.exports.memory.buffer.slice(instance.exports.s.value, instance.exports.s.value + 8)));
                            }
                        }
                        if (!is_valid) {
                            @{move || {
                                e_sender.try_send(Err(failure::err_msg("invalid module"))).unwrap();
                            }}();
                        } else {
                            @{move |module: WebAssemblyModule| {
                                sender.try_send(Ok(module)).unwrap();
                            }}(module);
                        }
                    });
                });
            };
            Box::new(
                receiver
                    .take(1)
                    .into_future()
                    .map_err(|_| failure::err_msg("temp err"))
                    .and_then(|module| {
                        let module: Box<dyn Module<T>> = Box::new(WASMModule {
                            ty: PhantomData,
                            module: module.0.unwrap()?,
                        });
                        Ok(module)
                    }),
            )
        })
    }
}
