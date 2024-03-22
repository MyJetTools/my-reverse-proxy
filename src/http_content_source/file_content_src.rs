use std::sync::Arc;

use hyper::Uri;

use crate::{http_proxy_pass::ProxyPassError, settings::LocalFilePath};

use super::{RequestExecutor, WebContentType};

pub struct LocalPathContentSrc {
    file_path: String,
    default_file: Option<String>,
}

impl LocalPathContentSrc {
    pub fn new(file_path: LocalFilePath, default_file: Option<String>) -> Self {
        let mut file_path = file_path.get_value().to_string();

        let last_char = *file_path.as_bytes().last().unwrap() as char;
        if last_char == std::path::MAIN_SEPARATOR {
            file_path.pop();
        }

        Self {
            file_path,
            default_file,
        }
    }

    pub fn get_request_executor(
        &self,
        uri: &Uri,
    ) -> Result<Arc<dyn RequestExecutor + Send + Sync + 'static>, ProxyPassError> {
        let file_path = if uri.path() == "/" {
            if let Some(default_file) = self.default_file.as_ref() {
                format!("{}/{}", self.file_path, default_file)
            } else {
                format!("{}{}", self.file_path, uri.path())
            }
        } else {
            format!("{}{}", self.file_path, uri.path())
        };

        let result = FileRequestExecutor { file_path };
        Ok(Arc::new(result))
    }
}

pub struct FileRequestExecutor {
    file_path: String,
}

#[async_trait::async_trait]
impl RequestExecutor for FileRequestExecutor {
    async fn execute_request(
        &self,
    ) -> Result<Option<(Vec<u8>, Option<WebContentType>)>, ProxyPassError> {
        let content_type = WebContentType::detect_by_extension(&self.file_path);
        match tokio::fs::read(&self.file_path).await {
            Ok(content) => Ok(Some((content, content_type))),
            Err(_) => Ok(None),
        }
    }
}
