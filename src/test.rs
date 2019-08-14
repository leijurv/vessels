#[no_mangle]
pub static s: u64 = 11342963367118314059;

#[no_mangle]
pub extern "C" fn i(ptr: i32, len: i32) {
    unsafe { o(ptr as *const u8, len as u32) };
}

extern "C" {
    fn o(ptr: *const u8, len: u32);
}
