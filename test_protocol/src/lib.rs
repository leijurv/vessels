use vessels::protocol::protocol;

#[protocol]
pub trait TestProtocol {
    fn add_one(&self, number: u64) -> u64;
}
