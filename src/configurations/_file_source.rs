use super::*;

#[derive(Debug, Clone)]
pub enum FileSource {
    File(String),
    Http(String),
    Ssh(SshContentSource),
}

impl FileSource {
    pub fn from_str(src: &str) -> Result<Self, String> {
        let mut check_ssh = src.split("->");
        let left_part = check_ssh.next().unwrap();
        let right_part = check_ssh.next();

        if let Some(right_part) = right_part {
            let content_src = SshContentSource::parse(left_part, right_part)?;
            return Ok(FileSource::Ssh(content_src));
        }

        if src.starts_with("http") {
            return Ok(FileSource::Http(src.to_string()));
        }

        Ok(Self::File(src.to_string()))
    }

    /*
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
     */
}
