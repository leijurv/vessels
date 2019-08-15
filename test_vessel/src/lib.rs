use test_protocol::TestProtocol;
use vessels::export;

#[derive(Default)]
struct Test;

#[export]
impl TestProtocol for Test {
    fn add_one(&self, input: u64) -> u64 {
        input + 1
    }
}
