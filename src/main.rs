use futures::Future;
use vessels::{executor, protocol::protocol, Module};

#[macro_use]
extern crate stdweb;

#[protocol]
pub trait TestProtocol {
    fn add_one(&self, number: u64) -> u64;
}

fn main() {
    std::panic::set_hook(Box::new(|info: &std::panic::PanicInfo| {
        let mut msg = info.to_string();
        js! {
            console.error((new Error()).stack);
        };
        console!(error, msg);
    }));
    executor::run(
        Module::compile(include_bytes!("test.wasm").to_vec())
            .and_then(|module: Box<dyn Module<dyn TestProtocol>>| {
                module.instantiate().and_then(|instance| {
                    console!(log, format!("nice: {}", instance.add_one(68)));
                    Ok(())
                })
            })
            .map_err(|err| console!(log, format!("{:?}", err)))
            .then(|_| Ok(())),
    );
}
