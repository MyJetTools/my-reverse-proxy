use crate::http_proxy_pass::HostPort;

pub fn generate_logout_page<THostPort: HostPort + Send + Sync + 'static>(
    req: &THostPort,
) -> String {
    return super::html::generate_with_template(|| {
        format!(
            r###"<h2>User is log outed</h2><a class="btn btn-primary" href="https://{}">Ok</a>
            <script>
            var cookies = document.cookie.split(";");

            for (let i = 0; i < cookies.length; i++) {{
                var cookie = cookies[i];
                var eqPos = cookie.indexOf("=");
                var name = eqPos > -1 ? cookie.substr(0, eqPos) : cookie;
                document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT";
            }}
            </script>
            
            "###,
            req.get_host_port()
        )
    });
}
