use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{base64_url_decode, random_secret};

/// How many bytes of key material a freshly generated signing key gets — the
/// HMAC-SHA256 block size, so the key is never stretched or hashed down.
const SIGNING_KEY_BYTES: usize = 64;

/// The signing key, kept on disk as one small JSON file.
///
/// It has to outlive a restart: the access and refresh tokens are stateless and
/// verified purely by their signature, so a regenerated key silently invalidates
/// every token Claude is holding and the connector asks the user to authorize
/// again.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SigningKeyFile {
    #[serde(default)]
    pub oauth_signing_key: Option<String>,
}

impl SigningKeyFile {
    /// Reads the key for an oauth block, generating and persisting one on first
    /// use.
    ///
    /// A file that exists but can not be parsed is an error rather than a silent
    /// reset: quietly regenerating the key would log the connector out, and it
    /// is better to say so than to leave the user wondering why.
    pub async fn load_or_create(path: &Path) -> Result<Vec<u8>, String> {
        let existing = Self::load(path).await?;

        if let Some(key) = existing.oauth_signing_key.as_deref() {
            let key = key.trim();
            if !key.is_empty() {
                return base64_url_decode(key).map_err(|err| {
                    format!(
                        "The oauth signing key in '{}' is not valid base64url. Err: {}",
                        path.display(),
                        err
                    )
                });
            }
        }

        let generated = random_secret(SIGNING_KEY_BYTES);

        let to_save = Self {
            oauth_signing_key: Some(generated.clone()),
        };

        to_save.save(path).await?;

        base64_url_decode(&generated)
    }

    async fn load(path: &Path) -> Result<Self, String> {
        let content = match tokio::fs::read(path).await {
            Ok(content) => content,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(err) => {
                return Err(format!(
                    "Can not read the oauth signing key file '{}'. Err: {}",
                    path.display(),
                    err
                ))
            }
        };

        if content.is_empty() {
            return Ok(Self::default());
        }

        serde_json::from_slice(&content).map_err(|err| {
            format!(
                "The oauth signing key file '{}' is not valid JSON. Err: {}. Fix or remove it — \
                 removing it regenerates the key, which forces every connector using this oauth \
                 block to be authorized again",
                path.display(),
                err
            )
        })
    }

    /// Writes the file, replacing it atomically so an interrupted write can not
    /// leave a half-written file that fails to parse on the next start.
    async fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(folder) = path.parent() {
            tokio::fs::create_dir_all(folder).await.map_err(|err| {
                format!(
                    "Can not create the oauth signing key folder '{}'. Err: {}",
                    folder.display(),
                    err
                )
            })?;
        }

        let content = serde_json::to_vec_pretty(self)
            .map_err(|err| format!("Can not serialize the oauth signing key file. Err: {}", err))?;

        let temporary = temporary_path(path);

        tokio::fs::write(&temporary, &content)
            .await
            .map_err(|err| {
                format!(
                    "Can not write the oauth signing key file '{}'. Err: {}",
                    temporary.display(),
                    err
                )
            })?;

        // It holds a signing key, so keep it to the owner.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let _ = tokio::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o600))
                .await;
        }

        tokio::fs::rename(&temporary, path).await.map_err(|err| {
            format!(
                "Can not replace the oauth signing key file '{}'. Err: {}",
                path.display(),
                err
            )
        })?;

        Ok(())
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut file_name = path.file_name().unwrap_or_default().to_os_string();
    file_name.push(".tmp");

    match path.parent() {
        Some(folder) => folder.join(file_name),
        None => PathBuf::from(file_name),
    }
}

/// Where an oauth block keeps its signing key when the settings do not say.
pub fn default_signing_key_file(oauth_id: &str) -> String {
    let safe_id: String = oauth_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect();

    format!("~/.my-reverse-proxy-oauth/{}.json", safe_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join("my-reverse-proxy-oauth-tests");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[tokio::test]
    async fn a_key_is_generated_on_first_use_and_kept_afterwards() {
        let path = temp_file("first-use.json");

        let first = SigningKeyFile::load_or_create(&path).await.unwrap();
        let second = SigningKeyFile::load_or_create(&path).await.unwrap();

        assert_eq!(first.len(), SIGNING_KEY_BYTES);
        // The same key comes back — this is what keeps issued tokens valid
        // across a restart.
        assert_eq!(first, second);

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn two_blocks_get_different_keys() {
        let first_path = temp_file("block-a.json");
        let second_path = temp_file("block-b.json");

        let first = SigningKeyFile::load_or_create(&first_path).await.unwrap();
        let second = SigningKeyFile::load_or_create(&second_path).await.unwrap();

        assert_ne!(first, second);

        let _ = std::fs::remove_file(&first_path);
        let _ = std::fs::remove_file(&second_path);
    }

    #[tokio::test]
    async fn a_corrupt_file_is_an_error_rather_than_a_silent_reset() {
        let path = temp_file("corrupt.json");
        tokio::fs::write(&path, b"{ this is not json")
            .await
            .unwrap();

        assert!(SigningKeyFile::load_or_create(&path).await.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[tokio::test]
    async fn saving_leaves_no_temporary_file_behind() {
        let path = temp_file("atomic.json");

        SigningKeyFile::load_or_create(&path).await.unwrap();

        assert!(path.exists());
        assert!(!temporary_path(&path).exists());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_default_key_file_is_named_after_the_block() {
        assert_eq!(
            default_signing_key_file("claude"),
            "~/.my-reverse-proxy-oauth/claude.json"
        );
        // A block id is a YAML key and can hold anything — it must not be able
        // to steer the path.
        assert_eq!(
            default_signing_key_file("../../etc/passwd"),
            "~/.my-reverse-proxy-oauth/______etc_passwd.json"
        );
    }
}
