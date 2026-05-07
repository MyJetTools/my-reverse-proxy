pub fn get_cookie<'s>(cookie_string: &'s str, cookie_name: &str) -> Option<&'s str> {
    for itm in cookie_string.split(";") {
        if let Some(eq_index) = itm.find("=") {
            let name = itm[..eq_index].trim();

            if name == cookie_name {
                let value = &itm[eq_index + 1..];
                return Some(value);
            }
        }
    }

    None
}
