pub const READ_LOOP_SIZE: usize = 1024 * 1024;

pub fn allocated_read_buffer(size: Option<usize>) -> Vec<u8> {
    let size = size.unwrap_or(READ_LOOP_SIZE);
    let mut buf: Vec<u8> = Vec::with_capacity(size);
    unsafe {
        buf.set_len(buf.capacity());
    }

    buf
}
