mod generate_login_page;
mod resolve_email;
pub use resolve_email::*;

pub use generate_login_page::*;
mod generate_authorized_page;
pub use generate_authorized_page::*;
mod generate_logout_page;
pub use generate_logout_page::*;

pub const AUTHORIZED_PATH: &str = "/authorized";
pub const LOGOUT_PATH: &str = "/logout";
mod html;
pub mod token;

pub fn generate_redirect_url<
    THostPort: crate::http_proxy_pass::HostPort + Send + Sync + 'static,
>(
    req: &THostPort,
) -> String {
    format!("{}{}", req.get_host_port().as_str(), AUTHORIZED_PATH)
}
