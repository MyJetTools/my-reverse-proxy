use std::{sync::Arc, time::Duration};

use hyper::Uri;
use my_ssh::{SshCredentials, SshSession};
use tokio::sync::Mutex;

use crate::http_proxy_pass::ProxyPassError;

use super::{RequestExecutor, RequestExecutorResult, WebContentType};

pub struct PathOverSshContentSource {
    use_connection_pool: bool,
    ssh_credentials: Arc<SshCredentials>,
    home_value: Arc<Mutex<Option<String>>>,
    default_file: Option<String>,
    pub file_path: String,
    execute_timeout: Duration,
}

impl PathOverSshContentSource {
    pub fn new(
        use_connection_pool: bool,
        ssh_credentials: Arc<SshCredentials>,
        file_path: String,
        default_file: Option<String>,
        execute_timeout: Duration,
    ) -> Self {
        Self {
            use_connection_pool,
            ssh_credentials,
            file_path,
            home_value: Arc::new(Mutex::new(None)),
            default_file,
            execute_timeout,
        }
    }

    pub async fn get_request_executor(
        &self,
        uri: &Uri,
    ) -> Result<Arc<dyn RequestExecutor + Send + Sync + 'static>, ProxyPassError> {
        let ssh_session = if self.use_connection_pool {
            my_ssh::SSH_SESSIONS_POOL
                .get_or_create(&self.ssh_credentials)
                .await
        } else {
            Arc::new(SshSession::new(self.ssh_credentials.clone()))
        };

        let file_path = if uri.path() == "/" {
            if let Some(default_file) = self.default_file.as_ref() {
                format!("{}/{}", self.file_path, default_file)
            } else {
                format!("{}{}", self.file_path, uri.path())
            }
        } else {
            format!("{}{}", self.file_path, uri.path())
        };

        let result = FileOverSshRequestExecutor {
            file_path,
            ssh_session,
            home_value: self.home_value.clone(),
            execute_timeout: self.execute_timeout,
        };
        Ok(Arc::new(result))
    }
}

pub struct FileOverSshRequestExecutor {
    ssh_session: Arc<SshSession>,
    file_path: String,
    home_value: Arc<Mutex<Option<String>>>,
    execute_timeout: Duration,
}

#[async_trait::async_trait]
impl RequestExecutor for FileOverSshRequestExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError> {
        let file_path = if self.file_path.contains("~") {
            let mut home_value = self.home_value.lock().await;

            if home_value.is_none() {
                let (home, _) = self
                    .ssh_session
                    .execute_command("echo $HOME", self.execute_timeout)
                    .await?;
                home_value.replace(home.trim().to_string());
            }

            let result = self.file_path.replace("~", home_value.as_ref().unwrap());
            result
        } else {
            self.file_path.clone()
        };

        let result = self
            .ssh_session
            .download_remote_file(&file_path, self.execute_timeout)
            .await;

        match result {
            Ok(content) => Ok(RequestExecutorResult {
                status_code: 200,
                content_type: WebContentType::detect_by_extension(&file_path),
                body: content,
            }),
            Err(err) => {
                println!("{} -> Error: {:?}", file_path, err);
                match &err {
                    my_ssh::SshSessionError::SshError(ssh_err) => {
                        if let Some(ssh2_error) = ssh_err.as_ssh2() {
                            if let my_ssh::ssh2::ErrorCode::Session(value) = ssh2_error.code() {
                                if value == -28 {
                                    return Ok(RequestExecutorResult {
                                        status_code: 404,
                                        content_type: None,
                                        body: "Not found".as_bytes().to_vec(),
                                    });
                                }
                            }
                        }
                    }
                    _ => {}
                }

                Err(ProxyPassError::SshSessionError(err))
            }
        }
    }
}
