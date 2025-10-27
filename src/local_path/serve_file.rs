use my_http_server::WebContentType;
use rust_extensions::{file_utils::FilePath, SliceOrVec};

use crate::h1_utils::Http1HeadersBuilder;

pub async fn serve_file(
    files_path: &FilePath,
    path: &str,
    default_file: Option<&str>,
    sender: &tokio::sync::mpsc::Sender<SliceOrVec<'static, u8>>,
) {
    let file_name = if path == "/" {
        if let Some(default_file) = default_file {
            default_file
        } else {
            sender
                .send(crate::error_templates::NOT_FOUND.as_slice().into())
                .await
                .unwrap();
            return;
        }
    } else {
        path
    };

    let mut file_path = files_path.clone();
    file_path.append_segment(file_name);

    match tokio::fs::read(file_path.as_str()).await {
        Ok(content) => {
            let mut headers = Http1HeadersBuilder::new();
            headers.add_http_response_ok();
            if content.len() > 0 {
                headers.add_header("content-length", content.len().to_string().as_str());
            }

            if let Some(content_type) = WebContentType::detect_by_extension(file_path.as_str()) {
                headers.add_header("content-type", content_type.as_str());
            }
            headers.write_cl_cr();
            sender.send(headers.into_bytes().into()).await.unwrap();
            sender.send(content.into()).await.unwrap();
        }
        Err(_) => {
            sender
                .send(crate::error_templates::NOT_FOUND.as_slice().into())
                .await
                .unwrap();
        }
    }
}
