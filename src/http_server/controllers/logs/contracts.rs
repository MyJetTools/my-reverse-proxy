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
    pub ip: Option<String>,
    pub country: Option<String>,
    pub message: String,
}

impl Into<ProxyLogLineHttpModel> for ProxyLogEntry {
    fn into(self) -> ProxyLogLineHttpModel {
        // Resolve the country (ISO-3, flag file name) from the stored IP here at
        // the API layer — it is not kept in the in-memory log entry.
        let country = self
            .ip
            .as_ref()
            .and_then(|ip| ip.parse::<std::net::IpAddr>().ok())
            .and_then(crate::ip_db::lookup_country_iso3);

        ProxyLogLineHttpModel {
            moment: self.moment.unix_microseconds,
            ip: self.ip,
            country,
            message: self.message,
        }
    }
}
