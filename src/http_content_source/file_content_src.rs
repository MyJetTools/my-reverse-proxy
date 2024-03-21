use hyper::Uri;

use crate::http_proxy_pass::ProxyPassError;

pub struct FileContentSrc {
    file_path: String,
}

impl FileContentSrc {
    pub fn new(mut file_path: String) -> Self {
        let last_char = *file_path.as_bytes().last().unwrap() as char;
        if last_char == std::path::MAIN_SEPARATOR {
            file_path.pop();
        }

        Self { file_path }
    }

    pub fn get_request_executor(&self, uri: &Uri) -> Result<RequestExecutor, ProxyPassError> {
        let result = RequestExecutor {
            file_path: format!("{}{}", self.file_path, uri.path()),
        };
        Ok(result)
    }
}

pub struct RequestExecutor {
    file_path: String,
}

impl RequestExecutor {
    pub async fn execute_request(&self) -> Result<Vec<u8>, ProxyPassError> {
        let result = tokio::fs::read(&self.file_path).await?;
        Ok(result)
    }
}
