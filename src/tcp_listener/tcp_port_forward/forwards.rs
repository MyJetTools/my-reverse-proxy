use crate::{
    app::SshSessionHandler, network_stream::*, tcp_listener::AcceptedTcpConnection,
    tcp_utils::LoopBuffer,
};

pub async fn handle_port_forward<TRemoteNetworkStream: NetworkStream + Send + Sync + 'static>(
    server_stream: AcceptedTcpConnection,
    remote_stream: TRemoteNetworkStream,
    ssh_session_handler: Option<SshSessionHandler>,
) {
    let (remote_reader, remote_writer) = remote_stream.split();
    match server_stream.network_stream {
        MyNetworkStream::Tcp(tcp_stream) => {
            let (server_reader, server_writer) = tokio::io::split(tcp_stream);
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
        MyNetworkStream::UnixSocket(unix_stream) => {
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
