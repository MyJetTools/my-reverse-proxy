pub const READ_LOOP_SIZE: usize = 1024 * 1024;

pub fn allocated_read_buffer() -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(READ_LOOP_SIZE);
    unsafe {
        buf.set_len(buf.capacity());
    }

    buf
}
