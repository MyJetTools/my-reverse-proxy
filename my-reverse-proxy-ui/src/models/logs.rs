use serde::{Deserialize, Serialize};

/// Mirror of `ProxyLogsHttpModel` returned by the `/api/logs/*` endpoints in
/// `my-reverse-proxy`. Lines come newest-first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProxyLogsModel {
    pub items: Vec<ProxyLogLineModel>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProxyLogLineModel {
    /// Unix microseconds.
    pub moment: i64,
    /// Source IP of the event, when it could be resolved.
    pub ip: Option<String>,
    pub message: String,
}
