use std::sync::Arc;

use rust_extensions::slice_of_u8_utils::*;

use crate::{
    configurations::{
        HttpEndpointInfo, HttpListenPortConfiguration, ModifyHeadersConfig, ProxyPassLocationConfig,
    },
    h1_proxy_server::*,
    h1_utils::{Http1Headers, Http1HeadersBuilder, HttpContentLength},
    http_proxy_pass::HttpProxyPassIdentity,
    network_stream::*,
    types::HttpTimeouts,
};

use crate::tcp_utils::LoopBuffer;

pub struct H1Reader<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static> {
    read_part: TNetworkReadPart,
    pub loop_buffer: LoopBuffer,
    pub h1_headers_builder: Http1HeadersBuilder,
    pub timeouts: HttpTimeouts,
}

impl<TNetworkReadPart: NetworkStreamReadPart + Send + Sync + 'static> H1Reader<TNetworkReadPart> {
    pub fn new(read_part: TNetworkReadPart, timeouts: HttpTimeouts) -> Self {
        Self {
            read_part,
            loop_buffer: LoopBuffer::new(),
            h1_headers_builder: Http1HeadersBuilder::new(),
            timeouts,
        }
    }

    pub async fn read_headers(&mut self) -> Result<Http1Headers, ProxyServerError> {
        loop {
            {
                let buf = self.loop_buffer.get_data();

                if buf.len() > 0 {
                    let headers = Http1Headers::parse(buf);

                    if let Some(headers) = headers {
                        return Ok(headers);
                    }
                }
            }

            let Some(buffer) = self.loop_buffer.get_mut() else {
                return Err(ProxyServerError::BufferAllocationFail);
            };

            let read_size = self
                .read_part
                .read_with_timeout(buffer, self.timeouts.read_timeout)
                .await?;

            self.loop_buffer.advance(read_size);
        }
    }

    pub fn try_find_endpoint_info(
        &self,
        http_headers: &Http1Headers,
        listen_config: &Arc<HttpListenPortConfiguration>,
    ) -> Option<Arc<HttpEndpointInfo>> {
        let buffer = self.loop_buffer.get_data();

        let Some(header_position) = http_headers.host_value.as_ref() else {
            return listen_config.get_http_endpoint_info(None);
        };

        let Ok(host) = std::str::from_utf8(&buffer[header_position.start..header_position.end])
        else {
            return listen_config.get_http_endpoint_info(None);
        };

        match host.find(':') {
            Some(index) => listen_config.get_http_endpoint_info(Some(&host[..index])),
            None => listen_config.get_http_endpoint_info(Some(host)),
        }
    }

    pub async fn find_location<'s>(
        &self,
        http_headers: &Http1Headers,
        connection_info: &'s HttpConnectionInfo,
    ) -> Result<(&'s ProxyPassLocationConfig, &'s Arc<HttpEndpointInfo>), ProxyServerError> {
        let Some(endpoint_info) = &connection_info.endpoint_info else {
            return Err(ProxyServerError::HttpConfigurationIsNotFound);
        };

        let first_line = http_headers.get_first_line(self.loop_buffer.get_data());
        let path = first_line.get_path_and_query();

        //println!("Path: '{}'", path);

        let Some(location) = endpoint_info.find_location(path) else {
            return Err(ProxyServerError::LocationIsNotFound);
        };

        Ok((location, endpoint_info))
    }

    pub fn compile_headers(
        &mut self,
        http_headers: Http1Headers,
        modify_headers: &ModifyHeadersConfig,
        http_connection_info: &HttpConnectionInfo,
        identity: &Option<HttpProxyPassIdentity>,
        mcp_path: Option<&str>,
    ) -> Result<bool, ProxyServerError> {
        self.h1_headers_builder.clear();
        let data = self.loop_buffer.get_data();

        if let Some(mcp_path) = mcp_path {
            println!("Pushing mcp path {}", mcp_path);
            http_headers.push_first_line_with_other_path(
                data,
                mcp_path,
                &mut self.h1_headers_builder,
            );
            self.h1_headers_builder.push_cl_cr();
        } else {
            self.h1_headers_builder.push_raw_payload(
                &data[..http_headers.first_line_end + crate::consts::HTTP_CR_LF.len()],
            );
        }

        let mut pos = http_headers.first_line_end + crate::consts::HTTP_CR_LF.len();

        loop {
            let next_pos = data
                .find_sequence_pos(crate::consts::HTTP_CR_LF, pos)
                .unwrap();

            if next_pos == pos {
                break;
            }

            let header = &data[pos..next_pos];
            //println!("{:?}", std::str::from_utf8(header));

            let Some(header_name_end_pos) = header.find_byte_pos(b':', 0) else {
                return Err(ProxyServerError::HeadersParseError(
                    "Header does not have end `:` symbol",
                ));
            };

            let header_name =
                unsafe { std::str::from_utf8_unchecked(&header[..header_name_end_pos]) };

            if !modify_headers.has_to_be_removed(header_name) {
                self.h1_headers_builder.push_raw_payload(header);
                self.h1_headers_builder
                    .push_raw_payload(crate::consts::HTTP_CR_LF);
            }

            pos = next_pos + crate::consts::HTTP_CR_LF.len();
        }

        let http_request_reader = HttpHeadersReader {
            http_headers: &http_headers,
            payload: self.loop_buffer.get_data(),
        };

        for add_header in modify_headers.iter_add() {
            let value = crate::scripts::populate_value(
                http_request_reader,
                http_connection_info,
                identity,
                add_header.1.as_str(),
            );
            self.h1_headers_builder
                .push_header(add_header.0, value.as_str());
        }
        self.h1_headers_builder.push_cl_cr();

        let mut web_socket_upgrade = false;

        if let Some(upgrade_position) = http_headers.upgrade_value.as_ref() {
            let upgrade_value = &data[upgrade_position.start..upgrade_position.end];

            web_socket_upgrade =
                crate::h1_utils::compare_case_insensitive(upgrade_value, b"websocket");
        }

        self.loop_buffer.commit_read(http_headers.end);

        Ok(web_socket_upgrade)
    }

    pub async fn transfer_body<WritePart: H1Writer + Send + Sync + 'static>(
        &mut self,
        request_id: u64,
        write_stream: &mut WritePart,
        content_length: HttpContentLength,
    ) -> Result<(), ProxyServerError> {
        match content_length {
            HttpContentLength::None => return Ok(()),
            HttpContentLength::Known(size) => {
                if size == 0 {
                    return Ok(());
                }

                let result = super::transfer_body::transfer_known_size(
                    request_id,
                    &mut self.read_part,
                    write_stream,
                    &mut self.loop_buffer,
                    size,
                )
                .await;

                result
            }
            HttpContentLength::Chunked => {
                let result = super::transfer_body::transfer_chunked_body(
                    request_id,
                    &mut self.read_part,
                    write_stream,
                    &mut self.loop_buffer,
                )
                .await;
                result
            }
        }
    }

    pub fn into_read_part(self) -> (TNetworkReadPart, LoopBuffer) {
        (self.read_part, self.loop_buffer)
    }
}
