use std::sync::Arc;

use my_ssh::{SshCredentials, SshSession};

use super::SshToHttpPortForwardConfiguration;

pub async fn create_port_forward(
    ssh_credentials: &Arc<SshCredentials>,
    remote_host: &str,
    remote_port: u16,
    next_port: impl Fn() -> u64,
) -> Arc<SshToHttpPortForwardConfiguration> {
    let listen_port = next_port();

    println!(
        "Allocating listen port: {} for http port forward {}->{}:{}",
        listen_port,
        ssh_credentials.to_string(),
        remote_host,
        remote_port
    );

    let listen_host_port = super::generate_unix_socket(listen_port);

    let ssh_session = SshSession::new(ssh_credentials.clone());

    let result = ssh_session
        .start_port_forward(listen_host_port, remote_host.to_string(), remote_port)
        .await
        .unwrap();

    let configuration = SshToHttpPortForwardConfiguration {
        listen_port,
        tunnel: result.clone(),
        _ssh_session: ssh_session,
    };

    Arc::new(configuration)
}
