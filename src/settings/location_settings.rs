use std::{collections::HashMap, str::FromStr};

use serde::*;

use super::{
    LocalFilePath, LocalPathModel, ModifyHttpHeadersSettings, ProxyPassTo, RemoteHost,
    SshConfigSettings, SshConfiguration, SshProxyPassModel, StaticContentModel,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocationSettings {
    pub path: Option<String>,
    pub proxy_pass_to: String,
    #[serde(rename = "type")]
    pub location_type: Option<String>,
    pub modify_http_headers: Option<ModifyHttpHeadersSettings>,
    pub default_file: Option<String>,
    pub status_code: Option<u16>,
    pub content_type: Option<String>,
    pub body: Option<String>,
    pub whitelisted_ip: Option<String>,
}

impl LocationSettings {
    fn get_status_code(&self, endpoint_str: &str) -> Result<u16, String> {
        match self.status_code {
            Some(status_code) => Ok(status_code),
            None => Err(format!(
                "status_code is required for static content for endpoint {}",
                endpoint_str
            )),
        }
    }

    pub fn get_proxy_pass(
        &self,
        endpoint_str: &str,
        variables: &Option<HashMap<String, String>>,
        ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
    ) -> Result<ProxyPassTo, String> {
        let proxy_pass_to =
            crate::populate_variable::populate_variable(self.proxy_pass_to.trim(), variables);

        if proxy_pass_to.as_str().trim() == "static" {
            return Ok(ProxyPassTo::Static(StaticContentModel {
                status_code: self.get_status_code(endpoint_str)?,
                content_type: self.content_type.clone(),
                body: if let Some(body) = self.body.as_ref() {
                    body.as_bytes().to_vec()
                } else {
                    vec![]
                },
            }));
        }

        if proxy_pass_to.as_str().starts_with(super::SSH_PREFIX) {
            return Ok(ProxyPassTo::Ssh(SshProxyPassModel {
                ssh_config: SshConfiguration::parse(proxy_pass_to.as_str(), &ssh_configs)?,
                http2: self.is_http2()?,
                default_file: self.default_file.clone(),
            }));
        }

        if proxy_pass_to.as_str().starts_with("http") {
            if self.is_http2()? {
                return Ok(ProxyPassTo::Http2(RemoteHost::new(
                    proxy_pass_to.to_string(),
                )));
            } else {
                return Ok(ProxyPassTo::Http(RemoteHost::new(
                    proxy_pass_to.to_string(),
                )));
            }
        }

        if proxy_pass_to.as_str().starts_with("~")
            || proxy_pass_to.as_str().starts_with("/")
            || proxy_pass_to.as_str().starts_with(".")
        {
            return Ok(ProxyPassTo::LocalPath(LocalPathModel {
                local_path: LocalFilePath::new(proxy_pass_to.to_string()),
                default_file: self.default_file.clone(),
            }));
        }

        Ok(ProxyPassTo::Tcp(
            std::net::SocketAddr::from_str(proxy_pass_to.as_str()).unwrap(),
        ))
    }

    /*
      todo!("Remote it")
      pub fn get_http_content_source<'s>(
          &'s self,
          app: &AppContext,
          host: &str,
          location_id: i64,
          variables: &Option<HashMap<String, String>>,
          ssh_configs: &Option<HashMap<String, SshConfigSettings>>,
          debug: bool,
      ) -> Result<Option<HttpProxyPassContentSource>, String> {
          let proxy_pass_to = self.get_proxy_pass(variables, ssh_configs)?;

          match proxy_pass_to {
              ProxyPassTo::Static => {
                  return Ok(HttpProxyPassContentSource::Static(StaticContentSrc::new(
                      match self.status_code {
                          Some(status_code) => status_code,
                          None => panic!(
                              "status_code is required for static content in host {}",
                              host
                          ),
                      },
                      self.content_type.clone(),
                      if let Some(body) = self.body.as_ref() {
                          body.as_bytes().to_vec()
                      } else {
                          vec![]
                      },
                  ))
                  .into())
              }
              ProxyPassTo::Http(remote_host) => {
                  if self.is_http2()? {
                      return Ok(
                          HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                              location_id,
                              HttpProxyPassRemoteEndpoint::Http2(RemoteHost::new(
                                  remote_host.to_string(),
                              )),
                              debug,
                          ))
                          .into(),
                      );
                  } else {
                      return Ok(
                          HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                              location_id,
                              HttpProxyPassRemoteEndpoint::Http(RemoteHost::new(
                                  remote_host.to_string(),
                              )),
                              debug,
                          ))
                          .into(),
                      );
                  }
              }
              ProxyPassTo::LocalPath(file_path) => {
                  return Ok(
                      HttpProxyPassContentSource::LocalPath(LocalPathContentSrc::new(
                          file_path,
                          self.default_file.clone(),
                      ))
                      .into(),
                  );
              }
              ProxyPassTo::Ssh(ssh_configuration) => match ssh_configuration.remote_content {
                  super::SshContent::RemoteHost(remote_host) => {
                      if self.is_http2()? {
                          return Ok(
                              HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                                  location_id,
                                  HttpProxyPassRemoteEndpoint::Http2OverSsh {
                                      ssh_credentials: ssh_configuration.credentials,
                                      remote_host,
                                  },
                                  debug,
                              ))
                              .into(),
                          );
                      } else {
                          return Ok(
                              HttpProxyPassContentSource::Http(RemoteHttpContentSource::new(
                                  location_id,
                                  HttpProxyPassRemoteEndpoint::Http1OverSsh {
                                      ssh_credentials: ssh_configuration.credentials,
                                      remote_host,
                                  },
                                  debug,
                              ))
                              .into(),
                          );
                      }
                  }
                  super::SshContent::FilePath(file_path) => {
                      return Ok(HttpProxyPassContentSource::PathOverSsh(
                          PathOverSshContentSource::new(
                              ssh_configuration.credentials,
                              file_path,
                              self.default_file.clone(),
                              app.connection_settings.remote_connect_timeout,
                          ),
                      )
                      .into());
                  }
              },
              ProxyPassTo::Tcp(_) => {
                  return Ok(None);
              }
          }
      }
    */
    pub fn is_http2(&self) -> Result<bool, String> {
        if let Some(location_type) = self.location_type.as_ref() {
            match location_type.as_str() {
                "http" => return Ok(false),
                "http2" => return Ok(true),
                _ => return Err(format!("Unknown remote location type {}", location_type)),
            }
        }

        Ok(false)
    }
}
