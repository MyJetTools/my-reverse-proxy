use crate::{
    h1_server::{LoopBuffer, ProxyServerError},
    h1_utils::HttpContentLength,
    network_stream::*,
};

pub async fn transfer_body<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
>(
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    content_length: HttpContentLength,
    loop_buffer: &mut LoopBuffer,
) -> Result<(), ProxyServerError> {
    match content_length {
        HttpContentLength::None => return Ok(()),
        HttpContentLength::Known(size) => {
            transfer_known_size(read_stream, write_stream, loop_buffer, size).await
        }
        HttpContentLength::Chunked => {
            super::transfer_chunked_body(read_stream, write_stream, loop_buffer).await
        }
    }
}

async fn transfer_known_size<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: NetworkStreamWritePart + Send + Sync + 'static,
>(
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    mut remaining_size: usize,
) -> Result<(), ProxyServerError> {
    loop {
        {
            let read_buf = loop_buffer.get_data();

            if read_buf.len() > 0 {
                let to_send = if read_buf.len() < remaining_size {
                    read_buf.len()
                } else {
                    remaining_size
                };

                write_stream
                    .write_all_with_timeout(read_buf, crate::consts::WRITE_TIMEOUT)
                    .await;
                remaining_size -= to_send;
                loop_buffer.commit_read(to_send);
            }
        }

        if remaining_size == 0 {
            break;
        }

        let read_size = read_stream
            .read_with_timeout(loop_buffer.get_mut()?, crate::consts::READ_TIMEOUT)
            .await?;

        loop_buffer.advance(read_size);
    }

    Ok(())
}
