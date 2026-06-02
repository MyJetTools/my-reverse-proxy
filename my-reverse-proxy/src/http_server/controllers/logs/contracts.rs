use my_http_server::macros::MyHttpObjectStructure;
use serde::Serialize;

use crate::app::ProxyLogEntry;

#[derive(Serialize, MyHttpObjectStructure)]
pub struct ProxyLogsHttpModel {
    pub items: Vec<ProxyLogLineHttpModel>,
}

impl ProxyLogsHttpModel {
    /// Builds the model from the stored buffer, newest line first.
    pub fn from_entries(entries: Vec<ProxyLogEntry>) -> Self {
        let items = entries
            .into_iter()
            .rev()
            .map(|entry| entry.into())
            .collect();
        Self { items }
    }
}

#[derive(Serialize, MyHttpObjectStructure)]
pub struct ProxyLogLineHttpModel {
    pub moment: i64,
    pub message: String,
}

impl Into<ProxyLogLineHttpModel> for ProxyLogEntry {
    fn into(self) -> ProxyLogLineHttpModel {
        ProxyLogLineHttpModel {
            moment: self.moment.unix_microseconds,
            message: self.message,
        }
    }
}
