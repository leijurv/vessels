use futures::Future;
use test_protocol::TestProtocol;
use vessels::{executor, Module};

fn main() {
    executor::run(
        Module::compile(
            include_bytes!("../target/wasm32-unknown-unknown/debug/test_vessel.wasm").to_vec(),
        )
        .and_then(|module: Box<dyn Module<dyn TestProtocol>>| {
            module.instantiate().and_then(|instance| {
                println!("nice: {}", instance.add_one(68));
                Ok(())
            })
        })
        .map_err(|err| println!("{:?}", err))
        .then(|_| Ok(())),
    );
}
