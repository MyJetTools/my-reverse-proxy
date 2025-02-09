use std::time::Duration;

use encryption::*;
use rust_extensions::date_time::DateTimeAsMicroseconds;

use crate::{app::AppContext, types::*};

#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AuthToken {
    #[prost(string, tag = "1")]
    pub email: String,
    #[prost(int64, tag = "2")]
    pub expires: i64,
}

pub fn generate(app: &AppContext, email: &str) -> String {
    let auth_token = AuthToken {
        email: email.to_string(),
        expires: DateTimeAsMicroseconds::now()
            .add(Duration::from_secs(60 * 60 * 24))
            .unix_microseconds,
    };

    let mut dest: Vec<u8> = Vec::new();
    prost::Message::encode(&auth_token, &mut dest).unwrap();

    let result = app.token_secret_key.encrypt(&dest);

    result.as_base_64()
}

pub fn resolve(app: &AppContext, token_str: &str) -> Option<Email> {
    let aes = AesEncryptedDataOwned::from_base_64(token_str).ok()?;

    let token = app.token_secret_key.decrypt(&aes).ok()?;

    let result: AuthToken = prost::Message::decode(token.as_slice()).ok()?;

    let now = DateTimeAsMicroseconds::now();

    if result.expires < now.unix_microseconds {
        return None;
    }

    Some(Email::new(result.email))
}
