use rust_extensions::remote_endpoint::RemoteEndpointOwned;

#[derive(Debug, Clone)]
pub enum SshRemoteContent {
    RemoteHost(RemoteEndpointOwned),
    FilePath(String),
}

impl SshRemoteContent {
    pub fn parse(remote_part: &str) -> Result<Self, String> {
        if remote_part.starts_with("/")
            || remote_part.starts_with("~")
            || remote_part.starts_with(".")
        {
            Ok(Self::FilePath(remote_part.to_string()))
        } else {
            Ok(Self::RemoteHost(RemoteEndpointOwned::try_parse(
                remote_part.to_string(),
            )?))
        }
    }

    #[cfg(test)]
    pub fn unwrap_as_remote_host(&self) -> &RemoteEndpointOwned {
        match self {
            Self::RemoteHost(remote_host) => remote_host,
            Self::FilePath(file) => {
                panic!("Ssh remote content must be not a local file {}", file);
            }
        }
    }
    #[cfg(test)]
    pub fn unwrap_as_file_path(&self) -> &str {
        match self {
            Self::RemoteHost(remote_host) => {
                panic!("Unwrapping as file but has http: {}", remote_host.as_str());
            }
            Self::FilePath(file) => file.as_str(),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::RemoteHost(remote_host) => remote_host.as_str(),
            Self::FilePath(file) => file.as_str(),
        }
    }
}
