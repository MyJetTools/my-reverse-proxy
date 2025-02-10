pub fn merge_path_and_file(
    files_path: &str,
    req_path: &str,
    default_file: &Option<String>,
) -> String {
    let mut result = files_path.to_string();

    let req_path = if req_path == "/" {
        if let Some(default_file) = default_file {
            default_file
        } else {
            return result;
        }
    } else {
        return result;
    };

    if result.ends_with('/') {
        if req_path.starts_with('/') {
            result.push_str(&req_path[1..]);
        } else {
            result.push_str(req_path);
        }
    } else {
        if req_path.starts_with('/') {
            result.push_str(req_path);
        } else {
            result.push('/');
            result.push_str(req_path);
        }
    }

    result
}
