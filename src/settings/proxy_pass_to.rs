use my_ssh::ssh_settings::OverSshConnectionSettings;

pub struct StaticContentModel {
    pub status_code: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

impl StaticContentModel {
    pub fn to_string(&self) -> String {
        format!(
            "status_code: {}, content_type: {:?}, body: {}bytes",
            self.status_code,
            self.content_type,
            self.body.len()
        )
    }
}

pub struct ProxyPassFilesPathModel {
    pub files_path: OverSshConnectionSettings,
    pub default_file: Option<String>,
}

impl ProxyPassFilesPathModel {
    pub fn to_string(&self) -> String {
        if let Some(ssh_credentials) = self.files_path.ssh_credentials.as_ref() {
            format!(
                "ssh:{}@{}:{}->{}",
                ssh_credentials.get_user_name(),
                ssh_credentials.get_host_port().0,
                ssh_credentials.get_host_port().1,
                self.files_path.remote_resource_string
            )
        } else {
            self.files_path.remote_resource_string.clone()
        }
    }
}

pub enum ProxyPassTo {
    Http1(OverSshConnectionSettings),
    Http2(OverSshConnectionSettings),
    FilesPath(ProxyPassFilesPathModel),
    Static(StaticContentModel),
}

impl ProxyPassTo {
    pub fn to_string(&self) -> String {
        match self {
            ProxyPassTo::Http1(remote_host) => remote_host.to_string(),
            ProxyPassTo::Http2(remote_host) => remote_host.to_string(),
            ProxyPassTo::FilesPath(model) => model.to_string(),

            ProxyPassTo::Static(model) => model.to_string(),
        }
    }

    pub fn get_type_as_str(&self) -> &'static str {
        match self {
            ProxyPassTo::Http1(_) => "http1",
            ProxyPassTo::Http2(_) => "http2",
            ProxyPassTo::FilesPath(_) => "files_path",
            ProxyPassTo::Static(_) => "static",
        }
    }
}
