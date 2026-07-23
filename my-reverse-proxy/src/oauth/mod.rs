//! The proxy acting as an OAuth 2.1 authorization server in front of an
//! endpoint, so the "Add custom connector" dialog on claude.ai — which offers a
//! client id and secret and nothing else — can reach an MCP server that does no
//! authentication of its own.
//!
//! Everything here is transport-independent on purpose: `handle_oauth_request`
//! takes a method, a route, headers and a body and returns a status, headers and
//! a body. The h1 byte pipeline and the hyper-based h2 path both call it, so
//! there is one implementation of the protocol rather than one per pipeline.

mod auth_codes;
pub use auth_codes::*;
mod base_url;
pub use base_url::*;
mod auth_codes_inner;
pub use auth_codes_inner::*;
mod bearer_gate;
pub use bearer_gate::*;
mod consent_page;
pub use consent_page::*;
mod form;
pub use form::*;
mod handle_authorize;
pub use handle_authorize::*;
mod handle_oauth_request;
pub use handle_oauth_request::*;
mod handle_token;
pub use handle_token::*;
mod hmac_sha256;
pub use hmac_sha256::*;
mod metadata;
pub use metadata::*;
mod oauth_context;
pub use oauth_context::*;
mod oauth_error;
pub use oauth_error::*;
mod oauth_request;
pub use oauth_request::*;
mod oauth_response;
pub use oauth_response::*;
mod oauth_route;
pub use oauth_route::*;
mod pkce;
pub use pkce::*;
mod redirect_uri;
pub use redirect_uri::*;
mod secrets;
pub use secrets::*;
mod signing_key_storage;
pub use signing_key_storage::*;
mod token_signer;
pub use token_signer::*;
mod unix_time;
pub use unix_time::*;
