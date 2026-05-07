mod generate_login_page;
mod resolve_email;
pub use resolve_email::*;

pub use generate_login_page::*;
mod generate_authenticated_user;
pub use generate_authenticated_user::*;
mod generate_logout_page;
pub use generate_logout_page::*;

pub const AUTHORIZED_PATH: &str = "/authorized";
pub const LOGOUT_PATH: &str = "/logout";
mod handle_google_auth;
mod html;
pub mod token;
pub use handle_google_auth::*;

mod result;
pub use result::*;

mod utils;
pub use utils::*;
