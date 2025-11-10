use crate::{
    app::SshSessionHandler, network_stream::*, tcp_utils::LoopBuffer,
    types::AcceptedServerConnection,
};

pub async fn handle_port_forward<TRemoteNetworkStream: NetworkStream + Send + Sync + 'static>(
    server_stream: AcceptedServerConnection,
    remote_stream: TRemoteNetworkStream,
    ssh_session_handler: Option<SshSessionHandler>,
) {
    let (remote_reader, remote_writer) = remote_stream.split();
    match server_stream {
        AcceptedServerConnection::Tcp { network_stream, .. } => {
            let (server_reader, server_writer) = tokio::io::split(network_stream);
            tokio::spawn(crate::tcp_utils::copy_streams(
                server_reader,
                remote_writer,
                LoopBuffer::new(),
                ssh_session_handler,
            ));
            tokio::spawn(crate::tcp_utils::copy_streams(
                remote_reader,
                server_writer,
                LoopBuffer::new(),
                None,
            ));
        }
        AcceptedServerConnection::Unix(unix_stream) => {
            let (server_reader, server_writer) = tokio::io::split(unix_stream);
            tokio::spawn(crate::tcp_utils::copy_streams(
                server_reader,
                remote_writer,
                LoopBuffer::new(),
                ssh_session_handler,
            ));
            tokio::spawn(crate::tcp_utils::copy_streams(
                remote_reader,
                server_writer,
                LoopBuffer::new(),
                None,
            ));
        }
    }
}
