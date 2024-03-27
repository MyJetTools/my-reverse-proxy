use crate::http_proxy_pass::HostPort;

pub fn generate_authorized_page<THostPort: HostPort + Send + Sync + 'static>(
    req: &THostPort,
    email: &str,
) -> String {
    return super::html::generate_with_template(|| {
        format!(
            r###"<h2>Authenticated user: {}</h2><a class="btn btn-primary" href="https://{}">Ok</a>"###,
            email,
            req.get_host_port()
        )
    });
}
