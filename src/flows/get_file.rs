use std::time::Duration;

use my_settings_reader::flurl::FlUrl;
use my_ssh::SshSession;

use crate::settings::{FileName, FileSource};

pub async fn get_file(src: &FileSource) -> Vec<u8> {
    match src {
        FileSource::File(file_name) => {
            println!("Loading file {}", file_name);
            let file_name = FileName::new(&file_name);

            let result = tokio::fs::read(file_name.get_value().as_str())
                .await
                .unwrap();

            result
        }
        FileSource::Http(path) => {
            let response = FlUrl::new(path).get().await.unwrap();
            let result = response.receive_body().await.unwrap();
            result
        }
        FileSource::Ssh(ssh_credentials) => match &ssh_credentials.remote_content {
            crate::settings::SshContent::Socket(_) => {
                panic!("Reading file is not supported from socket yet");
            }
            crate::settings::SshContent::FilePath(path) => {
                println!(
                    "Loading file {}->{}",
                    ssh_credentials.credentials.to_string(),
                    path
                );
                let ssh_session = SshSession::new(ssh_credentials.credentials.clone().into());

                let result = ssh_session
                    .download_remote_file(&path, Duration::from_secs(5))
                    .await
                    .unwrap();

                result
            }
        },
    }
}
