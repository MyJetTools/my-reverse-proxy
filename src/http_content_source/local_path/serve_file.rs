use my_http_server::WebContentType;
use rust_extensions::{file_utils::FilePath, SliceOrVec};

use crate::h1_utils::Http1ResponseBuilder;

pub async fn serve_file(
    files_path: &FilePath,
    path: &str,
    default_file: Option<&str>,
) -> SliceOrVec<'static, u8> {
    let file_name = if path == "/" {
        if let Some(default_file) = default_file {
            default_file
        } else {
            return crate::error_templates::NOT_FOUND.as_slice().into();
        }
    } else {
        path
    };

    let mut file_path = files_path.clone();
    file_path.append_segment(file_name);

    match tokio::fs::read(file_path.as_str()).await {
        Ok(content) => {
            let mut result = Http1ResponseBuilder::new_as_ok_result();

            if let Some(content_type) = WebContentType::detect_by_extension(file_path.as_str()) {
                result = result.add_content_type(content_type.as_str());
            }

            return result.build_with_body(&content).into();
        }
        Err(_) => {
            return crate::error_templates::NOT_FOUND.as_slice().into();
        }
    }
}
