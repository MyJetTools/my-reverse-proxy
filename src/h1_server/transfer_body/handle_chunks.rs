use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::{
    h1_server::{LoopBuffer, ProxyServerError},
    network_stream::*,
};

use super::*;

pub async fn transfer_chunked_body<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
>(
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
) -> Result<(), ProxyServerError> {
    loop {
        // Read chunk header line
        let chunk_header = read_chunk_header(read_stream, loop_buffer).await?;

        let len = chunk_header.len;
        transfer_chunk_data(read_stream, write_stream, loop_buffer, chunk_header).await?;

        if len == 0 {
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

        let read_size = read_stream
            .read_with_timeout(loop_buffer.get_mut()?, crate::consts::READ_TIMEOUT)
            .await?;

        if read_size == 0 {
            return Err("Connection closed while reading chunk header".into());
        }
    }
}

async fn transfer_chunk_data<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
>(
    read_stream: &mut ReadPart,
    remote_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    chunk_header: ChunkHeader,
) -> Result<(), ProxyServerError> {
    let mut remain_to_send =
        chunk_header.len + chunk_header.chunk_size + crate::consts::HTTP_CR_LF.len() * 2;

    while remain_to_send > 0 {
        {
            let buf = loop_buffer.get_data();
            if buf.len() > 0 {
                let to_send = if buf.len() < remain_to_send {
                    buf.len()
                } else {
                    remain_to_send
                };

                remote_stream
                    .write_all_with_timeout(&buf[..to_send], crate::consts::WRITE_TIMEOUT)
                    .await;
                loop_buffer.commit_read(to_send);
                remain_to_send -= to_send;
            }
        }

        if remain_to_send == 0 {
            break;
        }

        read_stream
            .read_with_timeout(loop_buffer.get_mut()?, crate::consts::READ_TIMEOUT)
            .await?;
    }

    Ok(())
}
