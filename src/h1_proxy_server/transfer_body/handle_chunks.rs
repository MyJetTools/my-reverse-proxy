use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::{h1_proxy_server::*, network_stream::*, tcp_utils::*};

use super::*;

pub async fn transfer_chunked_body<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    request_id: u64,
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
) -> Result<(), ProxyServerError> {
    loop {
        // Read chunk header line
        let chunk_header = read_chunk_header(read_stream, loop_buffer).await?;

        let chunk_size = chunk_header.chunk_size;

        println!("Chunked ReqId: {}. Size: {}", request_id, chunk_size);

        transfer_chunk_data(
            request_id,
            read_stream,
            write_stream,
            loop_buffer,
            chunk_header,
        )
        .await?;

        if chunk_size == 0 {
            return Ok(());
        }
    }
}

async fn read_chunk_header<ReadPart: NetworkStreamReadPart + Send + Sync + 'static>(
    read_stream: &mut ReadPart,
    loop_buffer: &mut LoopBuffer,
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
            .read_with_timeout(buffer, crate::consts::READ_TIMEOUT)
            .await?;

        loop_buffer.advance(read_size);
    }
}

async fn transfer_chunk_data<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    request_id: u64,
    read_stream: &mut ReadPart,
    remote_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    chunk_header: ChunkHeader,
) -> Result<(), ProxyServerError> {
    let mut remain_to_send =
        chunk_header.len + chunk_header.chunk_size + crate::consts::HTTP_CR_LF.len() * 2;

    println!("Chunks. ReqId{}. Remaining: {}", request_id, remain_to_send);

    while remain_to_send > 0 {
        {
            let buf = loop_buffer.get_data();
            if buf.len() > 0 {
                let to_send = if buf.len() < remain_to_send {
                    buf.len()
                } else {
                    remain_to_send
                };

                let to_send_buf = &buf[..to_send];

                if chunk_header.chunk_size == 0 {
                    println!("{:?}", std::str::from_utf8(to_send_buf));
                }

                let write_error = remote_stream
                    .write_http_payload(request_id, to_send_buf, crate::consts::WRITE_TIMEOUT)
                    .await;

                if let Err(write_error) = write_error {
                    return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(
                        write_error,
                    ));
                }

                loop_buffer.commit_read(to_send);
                remain_to_send -= to_send;
                println!("Chunks. ReqId:{}. Sent: {}", request_id, to_send);
            }
        }

        if remain_to_send == 0 {
            break;
        }

        let Some(buffer) = loop_buffer.get_mut() else {
            println!("Buffer allocation fail - transfer_chunk_data");
            return Err(ProxyServerError::BufferAllocationFail);
        };

        let read_size = read_stream
            .read_with_timeout(buffer, crate::consts::READ_TIMEOUT)
            .await?;

        println!("Chunks. ReqId:{}. Uploaded: {}", request_id, read_size);

        loop_buffer.advance(read_size);
    }

    Ok(())
}
