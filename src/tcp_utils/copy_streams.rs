use crate::{app::SshSessionHandler, network_stream::*, tcp_utils::LoopBuffer};

pub enum WsDirection {
    ClientToServer,
    ServerToClient,
}

pub struct WsTrafficRecorder {
    pub domain: String,
    pub direction: WsDirection,
}

impl WsTrafficRecorder {
    pub fn record(&self, bytes: u64) {
        match self.direction {
            WsDirection::ClientToServer => crate::app::APP_CTX
                .traffic
                .record_ws_c2s(&self.domain, bytes),
            WsDirection::ServerToClient => crate::app::APP_CTX
                .traffic
                .record_ws_s2c(&self.domain, bytes),
        }
    }
}

pub async fn copy_streams<
    Reader: NetworkStreamReadPart + Send + 'static,
    Writer: NetworkStreamWritePart + Send + 'static,
>(
    mut reader: Reader,
    mut writer: Writer,
    mut loop_buffer: LoopBuffer,
    _ssh_session_handler: Option<SshSessionHandler>,
    recorder: Option<WsTrafficRecorder>,
) {
    loop {
        {
            let buf = loop_buffer.get_data();

            if buf.len() > 0 {
                let len = buf.len();
                let write_result = writer
                    .write_all_with_timeout(buf, crate::consts::WRITE_TIMEOUT)
                    .await;

                if write_result.is_err() {
                    break;
                }

                if let Some(rec) = recorder.as_ref() {
                    rec.record(len as u64);
                }

                loop_buffer.commit_read(len);
            }
        }

        let read_result = reader
            .read_with_timeout(loop_buffer.get_mut().unwrap(), crate::consts::READ_TIMEOUT)
            .await;

        let Ok(read_size) = read_result else {
            writer.shutdown_socket().await;
            break;
        };

        loop_buffer.advance(read_size);
    }
}
