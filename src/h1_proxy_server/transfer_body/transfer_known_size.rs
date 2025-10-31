use crate::{h1_proxy_server::*, network_stream::*, tcp_utils::*};

pub async fn transfer_known_size<
    ReadPart: NetworkStreamReadPart + Send + Sync + 'static,
    WritePart: H1Writer + Send + Sync + 'static,
>(
    request_id: u64,
    read_stream: &mut ReadPart,
    write_stream: &mut WritePart,
    loop_buffer: &mut LoopBuffer,
    mut remaining_size: usize,
) -> Result<(), ProxyServerError> {
    let size = remaining_size;
    loop {
        {
            let read_buf = loop_buffer.get_data();

            if read_buf.len() > 0 {
                let to_send = if read_buf.len() < remaining_size {
                    read_buf.len()
                } else {
                    remaining_size
                };

                let result = write_stream
                    .write_http_payload(
                        request_id,
                        &read_buf[..to_send],
                        crate::consts::WRITE_TIMEOUT,
                    )
                    .await;

                if let Err(err) = result {
                    return Err(ProxyServerError::CanNotWriteContentToRemoteConnection(err));
                }

                remaining_size -= to_send;
                loop_buffer.commit_read(to_send);

                println!(
                    "Read Body: ReqId:{}. Size: {} Sent: {}. Remains: {}",
                    request_id, size, to_send, remaining_size
                );
            }
        }

        if remaining_size == 0 {
            break;
        }

        let Some(buffer) = loop_buffer.get_mut() else {
            println!("Buffer allocation fail - transfer_known_size");
            return Err(ProxyServerError::BufferAllocationFail);
        };

        println!("Read Body: ReqId:{}. Size: {} Reading", request_id, size);
        let read_size = read_stream
            .read_with_timeout(buffer, crate::consts::READ_TIMEOUT)
            .await?;

        println!(
            "Read Body: ReqId:{}. Size: {} Uploaded: {}",
            request_id, size, read_size
        );

        loop_buffer.advance(read_size);
    }

    Ok(())
}
