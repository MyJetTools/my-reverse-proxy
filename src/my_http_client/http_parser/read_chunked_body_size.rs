use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use super::HttpParseError;

pub struct ChunkedBodySizeResult {
    pub chunk_size: usize,
    pub tcp_size: usize,
}

pub fn read_chunked_body_size(payload: &[u8]) -> Result<ChunkedBodySizeResult, HttpParseError> {
    let pos = payload.find_byte_pos(b'\r', 0);

    if pos.is_none() {
        return Err(HttpParseError::GetMoreData);
    }
    let pos = pos.unwrap();

    let body_size = std::str::from_utf8(&payload[..pos]).unwrap();

    let body_size: usize = match body_size.parse() {
        Ok(size) => size,
        Err(_) => {
            return Err(HttpParseError::Error(format!(
                "Invalid chunk size {}",
                body_size
            )))
        }
    };

    if pos > payload.len() {
        return Err(HttpParseError::GetMoreData);
    }

    Ok(ChunkedBodySizeResult {
        chunk_size: body_size,
        tcp_size: pos + 1,
    })
}
