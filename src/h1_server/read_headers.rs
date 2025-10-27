use crate::{
    h1_server::{LoopBuffer, ProxyServerError},
    h1_utils::HttpHeaders,
    network_stream::NetworkStreamReadPart,
};

pub async fn read_headers<TServerStream: NetworkStreamReadPart + Send + Sync + 'static>(
    read_part: &mut TServerStream,
    loop_buffer: &mut LoopBuffer,
) -> Result<HttpHeaders, ProxyServerError> {
    loop {
        {
            let buf = loop_buffer.get_data();

            if buf.len() > 0 {
                let headers = HttpHeaders::parse(buf);

                if let Some(headers) = headers {
                    return Ok(headers);
                }
            }
        }

        let read_size = read_part
            .read_with_timeout(loop_buffer.get_mut()?, crate::consts::READ_TIMEOUT)
            .await?;

        loop_buffer.advance(read_size);
    }
}
