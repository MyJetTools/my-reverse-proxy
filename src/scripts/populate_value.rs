use rust_common::placeholders::*;
use rust_extensions::StrOrString;

use crate::{
    h1_server::HttpConnectionInfo, http_proxy_pass::HttpProxyPassIdentity, types::HttpRequestReader,
};

pub fn populate_value<'s>(
    req: impl HttpRequestReader,
    http_connection_info: &HttpConnectionInfo,
    identity: &Option<HttpProxyPassIdentity>,
    value: &'s str,
) -> StrOrString<'s> {
    if !value.contains("${") {
        return value.into();
    }

    let mut result = String::new();

    for token in PlaceholdersIterator::new(value, "${", "}") {
        match token {
            ContentToken::Text(text) => result.push_str(text),
            ContentToken::Placeholder(placeholder) => match placeholder {
                "ENDPOINT_IP" => {
                    let ip = format!("{}", http_connection_info.socket_addr);
                    result.push_str(ip.as_str());
                }

                "CLIENT_CERT_CN" => {
                    if let Some(value) = identity {
                        result.push_str(value.as_str());
                    }
                }

                "PATH_AND_QUERY" => {
                    if let Some(path_and_query) = req.get_path_and_query() {
                        result.push_str(path_and_query);
                    }
                }

                "HOST_PORT" => {
                    if let Some(host) = req.get_host() {
                        result.push_str(host);
                    }
                }

                "ENDPOINT_SCHEMA" => {
                    if http_connection_info
                        .listen_config
                        .listen_endpoint_type
                        .is_https()
                    {
                        result.push_str("https");
                    } else {
                        result.push_str("http");
                    }
                }
                _ => {
                    if let Ok(value) = std::env::var(placeholder) {
                        result.push_str(&value);
                    }
                }
            },
        }
    }

    result.into()
}

/*
pub trait RequestInfo {
    fn get_endpoint_ip(&self) -> SocketAddr;

    fn get_identity(&self) -> Option<&str>;
    fn get_endpoint_schema(&self) -> ListenHttpEndpointType;

    fn get_host(&self) -> Option<&str>;
    fn get_path_and_query(&self) -> &str;


}
 */
