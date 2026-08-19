use crate::{h1_proxy_server::*, network_stream::*, tcp_utils::*, types::HttpTimeouts};

pub async fn transfer_known_size<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    connection_id: u64,
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    mut remaining_size: usize,
    timeouts: HttpTimeouts,
) -> Result<usize, ProxyServerError> {
    let total = remaining_size;
    loop {
        // Forward everything that has ALREADY arrived, in BODY_RELAY_CHUNK_SIZE
        // steps. The cap only splits a big buffer across several writes — it never
        // makes the pump wait for more bytes to round a step up, and the source is
        // never read again while unforwarded bytes are still sitting here (that
        // would stall a fully-buffered body until the read timeout).
        while remaining_size > 0 {
            let read_buf = loop_buffer.get_data();

            if read_buf.is_empty() {
                break;
            }

            let to_send = read_buf
                .len()
                .min(remaining_size)
                .min(crate::consts::BODY_RELAY_CHUNK_SIZE);

            let result = write_stream
                .write_http_payload(connection_id, &read_buf[..to_send], timeouts.write_timeout)
                .await;

            if let Err(err) = result {
                return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(err));
            }

            remaining_size -= to_send;
            loop_buffer.commit_read(to_send);
        }

        if remaining_size == 0 {
            break;
        }

        let Some(buffer) = loop_buffer.get_mut() else {
            println!("Buffer allocation fail - transfer_known_size");
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

    /// The regression the cap could have introduced: a body that has ALREADY been
    /// read into the loop buffer (a fast client lands the head plus hundreds of KB
    /// in one read) must be relayed in full, in capped steps, without the pump
    /// going back to the socket — there is nothing more to come, so a read there
    /// would stall the request until the read timeout.
    #[tokio::test]
    async fn a_buffered_body_bigger_than_the_cap_is_relayed_without_another_read() {
        let body = vec![b'x'; CAP * 2 + 12345];
        let mut loop_buffer = preloaded_buffer(&body);
        let mut source = FakeSource::exhausted();
        let mut sink = RelaySink::new();

        let transferred = transfer_known_size(
            0,
            &mut source,
            &mut sink,
            &mut loop_buffer,
            body.len(),
            test_timeouts(),
        )
        .await
        .unwrap();

        assert_eq!(transferred, body.len());
        assert_eq!(sink.written(), body);
        assert_eq!(
            source.reads, 0,
            "nothing may be read while unforwarded bytes are still in the buffer"
        );
        // Capped, never rounded up: two full steps and the remainder as it is.
        assert_eq!(sink.write_sizes(), vec![CAP, CAP, 12345]);
    }

    /// The cap is not a fill target: each piece that arrives is forwarded on its
    /// own instead of being gathered into a 256 KB write.
    #[tokio::test]
    async fn every_piece_is_forwarded_as_it_arrives() {
        let body = b"0123456789".to_vec();
        let mut loop_buffer = crate::tcp_utils::LoopBuffer::new();
        let mut source = FakeSource::with_max_read(body.clone(), 4);
        let mut sink = RelaySink::new();

        let transferred = transfer_known_size(
            0,
            &mut source,
            &mut sink,
            &mut loop_buffer,
            body.len(),
            test_timeouts(),
        )
        .await
        .unwrap();

        assert_eq!(transferred, body.len());
        assert_eq!(sink.written(), body);
        assert_eq!(sink.write_sizes(), vec![4, 4, 2]);
    }

    /// Only the declared content-length is taken: a pipelined next request sitting
    /// behind the body in the same buffer is left for the reader.
    #[tokio::test]
    async fn the_relay_stops_at_the_declared_length() {
        let mut wire = b"BODY".to_vec();
        wire.extend_from_slice(b"GET /next HTTP/1.1\r\n\r\n");
        let mut loop_buffer = preloaded_buffer(&wire);
        let mut source = FakeSource::exhausted();
        let mut sink = RelaySink::new();

        transfer_known_size(
            0,
            &mut source,
            &mut sink,
            &mut loop_buffer,
            4,
            test_timeouts(),
        )
        .await
        .unwrap();

        assert_eq!(sink.written(), b"BODY".to_vec());
        assert_eq!(loop_buffer.get_data(), b"GET /next HTTP/1.1\r\n\r\n");
        assert_eq!(source.reads, 0);
    }
}
