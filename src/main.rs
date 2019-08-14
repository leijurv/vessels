use futures::Future;
use vessels::{executor, protocol::protocol, Module};

#[protocol]
pub trait TestProtocol {
    fn add_one(&self, number: u64) -> u64;
}

fn main() {
    executor::run(
        Module::compile(include_bytes!("test.wasm").to_vec())
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
