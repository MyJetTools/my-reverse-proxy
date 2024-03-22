use rust_extensions::StrOrString;

use super::SshConfiguration;

pub enum FileSource {
    File(String),
    Http(String),
    Ssh(SshConfiguration),
}

impl FileSource {
    pub fn from_src(src: StrOrString) -> Self {
        if src.as_str().starts_with("http") {
            return FileSource::Http(src.to_string());
        }

        if src.as_str().starts_with("ssh") {
            return FileSource::Ssh(SshConfiguration::parse(src));
        }

        Self::File(src.to_string())
    }

    pub fn as_str<'s>(&'s self) -> StrOrString<'s> {
        match self {
            FileSource::File(s) => s.into(),
            FileSource::Http(s) => s.into(),
            FileSource::Ssh(s) => format!(
                "{}->{}",
                s.credentials.to_string(),
                s.remote_content.as_str()
            )
            .into(),
        }
    }
}
