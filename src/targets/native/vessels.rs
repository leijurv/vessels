use crate::{
    executor,
    protocol::{Protocol, Remote, RemoteSinkStream},
    Module,
};
use failure::Error;
use futures::{lazy, stream::SplitSink, Future, Sink, Stream};
use std::{
    ffi::c_void,
    marker::PhantomData,
    sync::{Arc, Mutex},
};
use wasmer_runtime::{func, imports, Ctx, Func, Instance, Module as WASMModule, Value};
use wasmer_runtime_core::{module::ExportIndex, types::Initializer};

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

struct SinkContainer(Box<dyn Sink<SinkItem = Vec<u8>, SinkError = ()>>);

fn o(ctx: &mut Ctx, ptr: u32, len: u32) {
    let memory = ctx.memory(0);
    let data: Vec<u8> = memory.view()[ptr as usize..(len + ptr) as usize]
        .iter()
        .map(|cell| cell.get())
        .collect();
    let ptr = ctx.data as *mut SinkContainer;
    unsafe { Box::from_raw(ptr) }.0.start_send(data).unwrap();
}

impl InstanceHandler {
    fn new<T: Protocol + ?Sized + 'static>(module: &WASMModule) -> Box<T> {
        let (rem, rss) = T::remote().separate();
        let (rsink, rstream) = rss.split();
        let mut instance = module
            .instantiate(
                &(imports! {
                    "env" => {
                        "o" => func!(o),
                    },
                }),
            )
            .unwrap();
        instance.context_mut().data = Box::into_raw(Box::new(SinkContainer(Box::new(
            rsink.with(|data: Vec<u8>| serde_cbor::from_slice(&data).map_err(|_| ())),
        )))) as *mut c_void;
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
            .call("i", &[Value::I32(0), Value::I32(data.len() as i32)])
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
            let module = wasmer_runtime::compile(data.as_slice())?;
            let m_info = module.info();
            let sig_idx = m_info
                .exports
                .get("s")
                .ok_or_else(|| failure::err_msg("temp error lol"))?;
            if let ExportIndex::Global(s) = sig_idx {
                if let Initializer::Const(Value::I32(s)) = &m_info
                    .globals
                    .get(
                        s.local_or_import(&m_info)
                            .local()
                            .expect("is this actually impossible?"),
                    )
                    .unwrap()
                    .init
                {
                    let data = &m_info
                        .data_initializers
                        .first()
                        .ok_or_else(|| failure::err_msg("temp error lol"))?;
                    if let Initializer::Const(Value::I32(b)) = data.base {
                        if data.data.len() < (s - b + 8) as usize {
                            Err(failure::err_msg("temp error lol"))?;
                        }
                        let mut bytes: [u8; 8] = Default::default();
                        bytes.copy_from_slice(&data.data[(s - b) as usize..(s - b + 8) as usize]);
                        if u64::from_ne_bytes(bytes) != T::DO_NOT_IMPLEMENT_THIS_TRAIT_MANUALLY {
                            Err(failure::err_msg("invalid wasm lol we really need to make proper errors for this stuff"))?;
                        }
                    } else {
                        Err(failure::err_msg("temp error lol"))?;
                    }
                } else {
                    Err(failure::err_msg("temp error lol"))?;
                }
            } else {
                Err(failure::err_msg("temp error lol"))?;
            }
            let module: Box<dyn Module<T>> = Box::new(WasmerModule {
                state: Arc::new(Mutex::new(WasmerModuleState {
                    module,
                    protocol_type: PhantomData,
                })),
            });
            Ok(module)
        })
    }
}
