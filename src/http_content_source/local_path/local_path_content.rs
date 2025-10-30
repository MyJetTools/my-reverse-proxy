use std::collections::BTreeMap;

use futures::lock::Mutex;
use rust_extensions::{file_utils::FilePath, SliceOrVec};

use crate::h1_utils::*;

pub struct LocalPathContent {
    files_path: FilePath,
    default_file: Option<String>,
    requests: Mutex<BTreeMap<u64, (String, String)>>,
}

impl LocalPathContent {
    pub fn new(files_path: &str, default_file: Option<String>) -> Self {
        Self {
            files_path: FilePath::from_str(files_path),
            default_file,
            requests: Mutex::new(Default::default()),
        }
    }

    pub async fn send_headers(&self, request_id: u64, h1_headers: &Http1HeadersBuilder) {
        let first_line = h1_headers.get_first_line();
        let (verb, path) = first_line.get_verb_and_path();
        self.requests
            .lock()
            .await
            .insert(request_id, (verb.to_string(), path.to_string()));
    }

    pub async fn get_content(&self, request_id: u64) -> SliceOrVec<'static, u8> {
        let verb_and_path = self.requests.lock().await.remove(&request_id);

        let Some(verb_and_path) = verb_and_path else {
            return crate::error_templates::NOT_FOUND.as_slice().into();
        };

        let (verb, path) = verb_and_path;

        if verb != "GET" {
            return crate::error_templates::NOT_FOUND.as_slice().into();
        }

        super::serve_file::serve_file(
            &self.files_path,
            path.as_str(),
            self.default_file.as_deref(),
        )
        .await
    }
}
