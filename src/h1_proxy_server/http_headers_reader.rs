use rust_extensions::slice_of_u8_utils::SliceOfU8Ext;

use crate::h1_utils::*;

#[derive(Clone, Copy)]
pub struct HttpHeadersReader<'s> {
    pub http_headers: &'s Http1Headers,
    pub payload: &'s [u8],
}

impl<'l> crate::types::HttpRequestReader for HttpHeadersReader<'l> {
    fn get_cookie<'s>(&'s self, cookie_name: &str) -> Option<&'s str> {
        let cookie = self.http_headers.cookie_value.as_ref()?;

        let host = &self.payload[cookie.start..cookie.end];

        let cookie_string = unsafe { std::str::from_utf8_unchecked(host) };

        crate::utils::get_cookie(cookie_string, cookie_name)
    }

    fn get_query_string<'s>(&'s self) -> Option<&'s str> {
        let index_start = self.payload.find_byte_pos(b' ', 0).unwrap() + 1;
        let index_end = self.payload.find_byte_pos(b' ', index_start).unwrap();

        let path_and_query =
            unsafe { std::str::from_utf8_unchecked(&self.payload[index_start..index_end]) };

        match path_and_query.find('?') {
            Some(index) => Some(&path_and_query[index + 1..]),
            None => None,
        }
    }

    fn get_host<'s>(&'s self) -> Option<&'s str> {
        let host_value = self.http_headers.host_value.as_ref()?;

        let host = &self.payload[host_value.start..host_value.end];

        let host = unsafe { std::str::from_utf8_unchecked(host) };

        Some(host)
    }

    fn get_path_and_query<'s>(&'s self) -> Option<&'s str> {
        let index_start = self.payload.find_byte_pos(b' ', 0).unwrap() + 1;
        let index_end = self.payload.find_byte_pos(b' ', index_start).unwrap();

        let path_and_query =
            unsafe { std::str::from_utf8_unchecked(&self.payload[index_start..index_end]) };

        Some(path_and_query)
    }

    fn get_path<'s>(&'s self) -> &'s str {
        let Some(path_and_query) = self.get_path_and_query() else {
            return "/";
        };

        match path_and_query.find("?") {
            Some(index) => &path_and_query[..index],
            None => path_and_query,
        }
    }
}
