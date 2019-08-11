use crate::{
    executor,
    protocol::{Protocol, Remote},
    Module,
};
use failure::Error;
use futures::{lazy, Future, Sink, Stream};
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};
use wasmer_runtime::{func, imports, Ctx, Instance, Module as WASMModule, Value};

struct WasmerModuleState<T: Protocol + ?Sized + 'static> {
    module: WASMModule,
    protocol_type: PhantomData<T>,
}

pub(crate) struct WasmerModule<T: Protocol + ?Sized + 'static> {
    state: Arc<Mutex<WasmerModuleState<T>>>,
}

unsafe impl Send for InstanceHandler {}

struct InstanceHandler {
    instance: Mutex<Instance>,
}

impl InstanceHandler {
    fn new<T: Protocol + ?Sized + 'static>(module: &WASMModule) -> Box<T> {
        let (rem, rss) = T::remote().separate();
        let (rsink, rstream) = rss.split();
        let rsink = Mutex::new(rsink);
        let handler = move |ctx: &mut Ctx, len: u32| {
            let memory = ctx.memory(0);
            let data: Vec<_> = memory.view()[0..len as usize]
                .iter()
                .map(|cell| cell.get())
                .collect();
            rsink
                .lock()
                .unwrap()
                .start_send(serde_cbor::from_slice(&data).unwrap())
                .unwrap();
        };
        let import_object = imports! {
            "env" => {
                "o" => func!(handler),
            },
        };
        let instance = module.instantiate(&import_object).unwrap();
        let instance = InstanceHandler {
            instance: Mutex::new(instance),
        };
        executor::spawn(rstream.for_each(move |item| {
            instance.call(serde_cbor::to_vec(&item).unwrap());
            Ok(())
        }));
        rem
    }
    fn call(&self, data: Vec<u8>) {
        let mut instance = self.instance.lock().unwrap();
        let memory = instance.context_mut().memory(0);
        for (byte, cell) in data
            .iter()
            .copied()
            .zip(memory.view()[0..data.len()].iter())
        {
            cell.set(byte);
        }
        instance
            .call("i", &[Value::I64(data.len() as i64)])
            .unwrap();
    }
}

impl<T: Protocol + ?Sized + 'static> Module<T> for WasmerModule<T> {
    fn instantiate(&self) -> Box<dyn Future<Item = Box<T>, Error = Error> + Send> {
        let state = self.state.clone();
        Box::new(lazy(move || {
            let state = state.lock().unwrap();
            Ok(InstanceHandler::new(&state.module))
        }))
    }
}

impl<T: Protocol + ?Sized + 'static> WasmerModule<T> {
    pub(crate) fn compile(
        data: Vec<u8>,
    ) -> impl Future<Item = Box<dyn Module<T> + 'static>, Error = Error> {
        lazy(move || {
            let module: Box<dyn Module<T>> = Box::new(WasmerModule {
                state: Arc::new(Mutex::new(WasmerModuleState {
                    module: wasmer_runtime::compile(data.as_slice())?,
                    protocol_type: PhantomData,
                })),
            });
            Ok(module)
        })
    }
}
