use rust_extensions::StrOrString;

pub trait HttpRequestReader {
    fn get_cookie<'s>(&'s self, cookie_name: &str) -> Option<&'s str>;

    fn get_query_string<'s>(&'s self) -> Option<&'s str>;

    fn get_host<'s>(&'s self) -> Option<&'s str>;

    fn get_path<'s>(&'s self) -> &'s str;

    fn get_path_and_query<'s>(&'s self) -> Option<&'s str>;

    fn get_query_string_param<'s>(&'s self, param: &str) -> Option<StrOrString<'s>> {
        let query = self.get_query_string()?;

        for itm in query.split("&") {
            let mut parts = itm.split("=");

            let left = parts.next().unwrap().trim();

            if let Some(right) = parts.next() {
                if left == param {
                    return Some(url_utils::decode_from_url_string(right.trim()));
                }
            }
        }

        None
    }

    fn get_authorization_token(&self) -> Option<&str> {
        let result = self.get_cookie(crate::consts::AUTHORIZED_COOKIE_NAME);
        result
    }
}
