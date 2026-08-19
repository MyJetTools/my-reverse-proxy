use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::{h1_proxy_server::*, network_stream::*, tcp_utils::*, types::HttpTimeouts};

use super::*;

pub async fn transfer_chunked_body<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    connection_id: u64,
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    timeouts: HttpTimeouts,
) -> Result<usize, ProxyServerError> {
    let mut total = 0usize;
    loop {
        // Read chunk header line
        let chunk_header = read_chunk_header(read_stream, loop_buffer, timeouts).await?;

        let chunk_size = chunk_header.chunk_size;

        let transferred = transfer_chunk_data(
            connection_id,
            read_stream,
            write_stream,
            loop_buffer,
            chunk_header,
            timeouts,
        )
        .await?;
        total += transferred;

        if chunk_size == 0 {
            return Ok(total);
        }
    }
}

async fn read_chunk_header<ReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    read_stream: &mut ReadPart,
    loop_buffer: &mut LoopBuffer,
    timeouts: HttpTimeouts,
) -> Result<ChunkHeader, ProxyServerError> {
    loop {
        {
            let read_buf = loop_buffer.get_data();

            if read_buf.len() > 2 {
                // Look for \r\n in the buffer
                if let Some(crlf_pos) = read_buf.find_sequence_pos(crate::consts::HTTP_CR_LF, 0) {
                    return ChunkHeader::new(crlf_pos, read_buf);
                }
            }
        }

        let Some(buffer) = loop_buffer.get_mut() else {
            println!("Buffer allocation fail - read_chunk_header");
            return Err(ProxyServerError::BufferAllocationFail);
        };

        let read_size = read_stream
            .read_with_timeout(buffer, timeouts.read_timeout)
            .await?;

        loop_buffer.advance(read_size);
    }
}

async fn transfer_chunk_data<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    connection_id: u64,
    read_stream: &mut ReadPart,
    remote_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    chunk_header: ChunkHeader,
    timeouts: HttpTimeouts,
) -> Result<usize, ProxyServerError> {
    let total = chunk_header.len + chunk_header.chunk_size + crate::consts::HTTP_CR_LF.len() * 2;
    let mut remain_to_send = total;

    while remain_to_send > 0 {
        // Same rule as the known-size path: everything already in the buffer goes
        // out first, in capped steps, and nothing is read while unforwarded bytes
        // remain.
        while remain_to_send > 0 {
            let buf = loop_buffer.get_data();

            if buf.is_empty() {
                break;
            }

            let to_send = buf
                .len()
                .min(remain_to_send)
                .min(crate::consts::BODY_RELAY_CHUNK_SIZE);

            let write_error = remote_stream
                .write_http_payload(connection_id, &buf[..to_send], timeouts.write_timeout)
                .await;

            if let Err(write_error) = write_error {
                return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                    write_error,
                ));
            }

            loop_buffer.commit_read(to_send);
            remain_to_send -= to_send;
        }

        if remain_to_send == 0 {
            break;
        }

        let Some(buffer) = loop_buffer.get_mut() else {
            println!("Buffer allocation fail - transfer_chunk_data");
            return Err(ProxyServerError::BufferAllocationFail);
        };

        let read_size = read_stream
            .read_with_timeout(buffer, timeouts.read_timeout)
            .await?;

        loop_buffer.advance(read_size);
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::super::test_utils::*;
    use super::*;

    const CAP: usize = crate::consts::BODY_RELAY_CHUNK_SIZE;

    /// One chunk larger than the relay cap, already fully buffered: it is relayed
    /// verbatim (framing included) in capped steps, with no read in between — the
    /// cap splits the write, it does not make the pump wait for bytes that will
    /// never come.
    #[tokio::test]
    async fn a_buffered_chunk_bigger_than_the_cap_is_relayed_without_another_read() {
        let payload = vec![b'z'; CAP + 4096];
        let mut wire = format!("{:x}\r\n", payload.len()).into_bytes();
        let header_len = wire.len();
        wire.extend_from_slice(&payload);
        wire.extend_from_slice(crate::consts::HTTP_CR_LF);
        wire.extend_from_slice(b"0\r\n\r\n");

        let mut loop_buffer = preloaded_buffer(&wire);
        let mut source = FakeSource::exhausted();
        let mut sink = RelaySink::new();

        let transferred =
            transfer_chunked_body(0, &mut source, &mut sink, &mut loop_buffer, test_timeouts())
                .await
                .unwrap();

        assert_eq!(
            sink.written(),
            wire,
            "the chunked framing is relayed verbatim"
        );
        assert_eq!(transferred, wire.len());
        assert_eq!(
            source.reads, 0,
            "nothing may be read while unforwarded bytes are still in the buffer"
        );
        let first_chunk_on_wire = header_len + payload.len() + crate::consts::HTTP_CR_LF.len();
        assert_eq!(
            sink.write_sizes(),
            vec![CAP, first_chunk_on_wire - CAP, "0\r\n\r\n".len()]
        );
    }

    /// A chunk that trickles in is forwarded as it arrives rather than gathered.
    #[tokio::test]
    async fn a_trickling_chunk_is_forwarded_as_it_arrives() {
        let wire = b"5\r\nhello\r\n0\r\n\r\n".to_vec();
        let mut loop_buffer = crate::tcp_utils::LoopBuffer::new();
        let mut source = FakeSource::with_max_read(wire.clone(), 4);
        let mut sink = RelaySink::new();

        let transferred =
            transfer_chunked_body(0, &mut source, &mut sink, &mut loop_buffer, test_timeouts())
                .await
                .unwrap();

        assert_eq!(sink.written(), wire);
        assert_eq!(transferred, wire.len());
        assert!(
            sink.writes.len() > 1,
            "arriving pieces are relayed as they come, not gathered into one write"
        );
    }
}
