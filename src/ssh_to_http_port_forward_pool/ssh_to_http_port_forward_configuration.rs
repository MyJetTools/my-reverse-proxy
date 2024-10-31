use std::sync::Arc;

use my_ssh::{SshCredentials, SshPortForwardTunnel};

pub struct SshToHttpPortForwardConfiguration {
    pub listen_port: u16,
    pub ssh_credentials: Arc<SshCredentials>,
    pub tunnel: Arc<SshPortForwardTunnel>,
}

impl SshToHttpPortForwardConfiguration {
    //pub fn get_unix_socket_path(&self) -> String {
    //    generate_unix_socket(self.listen_port)
    // }

    pub fn get_listen_host_port(&self) -> String {
        format!("127.0.0.1:{}", self.listen_port)
    }
}

pub fn generate_unix_socket(listen_port: u16) -> String {
    let home = std::env::var("HOME").unwrap();
    format!("{}/my-reverse-proxy-{}.sock", home, listen_port)
}

impl Drop for SshToHttpPortForwardConfiguration {
    fn drop(&mut self) {
        println!(
            "Dropped prot_forward connection: {}",
            self.tunnel.listen_string
        );
        if self.tunnel.listen_string.starts_with("/") {
            let _ = std::fs::remove_file(&self.tunnel.listen_string);
        }
    }
}
