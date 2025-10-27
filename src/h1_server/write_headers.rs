use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::{
    configurations::ModifyHeadersConfig,
    h1_server::{LoopBuffer, ProxyServerError},
    h1_utils::{Http1HeadersBuilder, HttpHeaders},
};

pub fn compile_headers(
    http_headers: HttpHeaders,
    h1_headers_builder: &mut Http1HeadersBuilder,
    loop_buffer: &mut LoopBuffer,
    modify_headers: &ModifyHeadersConfig,
    host: Option<&str>,
) -> Result<(), ProxyServerError> {
    h1_headers_builder.clear();
    let data = loop_buffer.get_data();

    h1_headers_builder
        .push_raw_payload(&data[..http_headers.first_line_end + crate::consts::HTTP_CR_LF.len()]);

    let mut pos = http_headers.first_line_end + crate::consts::HTTP_CR_LF.len();

    loop {
        let next_pos = data
            .find_sequence_pos(crate::consts::HTTP_CR_LF, pos)
            .unwrap();

        if next_pos == pos {
            break;
        }

        let header = &data[pos..next_pos];
        println!("{:?}", std::str::from_utf8(header));

        let Some(header_name_end_pos) = header.find_byte_pos(b':', 0) else {
            return Err(ProxyServerError::HeadersParseError(
                "Header does not have end `:` symbol",
            ));
        };

        let header_name = unsafe { std::str::from_utf8_unchecked(&header[..header_name_end_pos]) };

        println!("Header: {}", header_name);

        if header_name.eq_ignore_ascii_case("host") {
            if let Some(host_value) = host {
                h1_headers_builder.add_header("host", host_value);
            }
        } else if !modify_headers.has_to_be_removed(header_name) {
            h1_headers_builder.push_raw_payload(header);
            h1_headers_builder.push_raw_payload(crate::consts::HTTP_CR_LF);
        }

        pos = next_pos + crate::consts::HTTP_CR_LF.len();
    }

    for add_header in modify_headers.iter_add() {
        h1_headers_builder.add_header(add_header.0, add_header.1);
    }
    h1_headers_builder.push_raw_payload(crate::consts::HTTP_CR_LF);

    loop_buffer.commit_read(http_headers.end);

    Ok(())
}
