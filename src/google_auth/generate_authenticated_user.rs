pub fn generate_authenticated_user(
    req: &impl crate::types::HttpRequestReader,
    email: &str,
) -> String {
    return super::html::generate_with_template(|| {
        format!(
            r###"<h2>Authenticated user: {}</h2><a class="btn btn-primary" href="https://{}">Ok</a>"###,
            email,
            req.get_host().unwrap_or_default()
        )
    });
}
