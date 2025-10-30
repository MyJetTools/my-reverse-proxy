use crate::{app::SshSessionHandler, network_stream::*, tcp_utils::LoopBuffer};

pub async fn copy_streams<
    Reader: NetworkStreamReadPart + Send + 'static,
    Writer: NetworkStreamWritePart + Send + 'static,
>(
    mut reader: Reader,
    mut writer: Writer,
    mut loop_buffer: LoopBuffer,
    _ssh_session_handler: Option<SshSessionHandler>,
    debug: Option<&'static str>,
) {
    loop {
        {
            let buf = loop_buffer.get_data();

            if let Some(debug) = debug {
                println!("[{}] buf len: {}", debug, buf.len());
            }

            if buf.len() > 0 {
                let write_result = writer
                    .write_all_with_timeout(buf, crate::consts::WRITE_TIMEOUT)
                    .await;

                if write_result.is_err() {
                    break;
                }

                loop_buffer.commit_read(buf.len());
            }
        }

        let read_result = reader
            .read_with_timeout(loop_buffer.get_mut().unwrap(), crate::consts::READ_TIMEOUT)
            .await;

        if let Some(debug) = debug {
            println!("[{}] buf read: {:?}", debug, read_result);
        }

        let Ok(read_size) = read_result else {
            writer.shutdown_socket().await;
            break;
        };

        loop_buffer.advance(read_size);
    }

    println!("Copy loop is done");
}
