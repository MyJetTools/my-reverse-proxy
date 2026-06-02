use crate::{
    app::SshSessionHandler, network_stream::*, tcp_utils::LoopBuffer, types::HttpTimeouts,
};

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
    log_scope: Option<crate::app::ProxyLogScope>,
    timeouts: HttpTimeouts,
) {
    let direction_label = recorder
        .as_ref()
        .map(|r| match r.direction {
            WsDirection::ClientToServer => "c2s",
            WsDirection::ServerToClient => "s2c",
        })
        .unwrap_or("?");
    // For the websocket pump `log_scope` carries the endpoint + location + IP,
    // so route pump errors into that location's in-memory log instead of the
    // console. Raw TCP port-forward copies have no scope and stay on stderr.
    let log = |message: String| match &log_scope {
        Some(scope) => scope.write_always(message),
        None => eprintln!("{}", message),
    };

    loop {
        {
            let buf = loop_buffer.get_data();

            if buf.len() > 0 {
                let len = buf.len();
                let write_result = writer
                    .write_all_with_timeout(buf, timeouts.write_timeout)
                    .await;

                if let Err(err) = write_result {
                    log(format!(
                        "ws-pump {dir} write {n} bytes failed: {err:?}",
                        dir = direction_label,
                        n = len,
                        err = err,
                    ));
                    break;
                }

                if let Some(rec) = recorder.as_ref() {
                    rec.record(len as u64);
                }

                loop_buffer.commit_read(len);
            }
        }

        let read_result = reader
            .read_with_timeout(loop_buffer.get_mut().unwrap(), timeouts.read_timeout)
            .await;

        let read_size = match read_result {
            Ok(0) => {
                // Peer closed cleanly: forward the half-close so the other
                // side sees EOF too instead of spinning forever on Ok(0).
                writer.shutdown_socket().await;
                break;
            }
            Ok(n) => n,
            Err(err) => {
                log(format!(
                    "ws-pump {dir} read failed: {err:?}",
                    dir = direction_label,
                    err = err,
                ));
                writer.shutdown_socket().await;
                break;
            }
        };

        loop_buffer.advance(read_size);
    }
}
