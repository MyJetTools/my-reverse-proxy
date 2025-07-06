use std::sync::Arc;

use hyper::Uri;
use my_http_server::WebContentType;

use crate::http_content_source::{RequestExecutor, RequestExecutorResult};
use crate::http_proxy_pass::{content_source::HttpResponse, ProxyPassError};

use crate::configurations::*;

pub struct LocalPathContentSrc {
    pub file_path: String,
    default_file: Option<String>,
}

impl LocalPathContentSrc {
    pub fn new(file_path: &LocalFilePath, default_file: Option<String>) -> Self {
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

    pub async fn execute(
        &self,
        req: http::Request<http_body_util::Full<bytes::Bytes>>,
    ) -> Result<HttpResponse, ProxyPassError> {
        println!("Executing as local path");
        let request_executor = self.get_request_executor(&req.uri())?;
        let result = request_executor.execute_request().await?;
        Ok(HttpResponse::Response(result.into()))
    }
}

pub struct FileRequestExecutor {
    file_path: String,
}

#[async_trait::async_trait]
impl RequestExecutor for FileRequestExecutor {
    async fn execute_request(&self) -> Result<RequestExecutorResult, ProxyPassError> {
        let result = match tokio::fs::read(&self.file_path).await {
            Ok(content) => RequestExecutorResult {
                status_code: 200,
                content_type: WebContentType::detect_by_extension(&self.file_path),
                body: content,
            },
            Err(_) => RequestExecutorResult {
                status_code: 404,
                content_type: None,
                body: "Not found".as_bytes().to_vec(),
            },
        };

        Ok(result)
    }
}
