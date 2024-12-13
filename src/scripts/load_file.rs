use std::{sync::Arc, time::Duration};

use http::Method;
use http_body_util::BodyExt;
use my_http_client::http1::{MyHttpClient, MyHttpRequestBuilder};
use my_settings_reader::flurl::FlUrl;
use my_ssh::{ssh_settings::OverSshConnectionSettings, SshCredentials};
use rust_extensions::remote_endpoint::RemoteEndpoint;

use crate::{app::AppContext, configurations::LocalFilePath};

pub async fn load_file(
    app: &Arc<AppContext>,
    content_source: &OverSshConnectionSettings,
) -> Result<Vec<u8>, String> {
    if let Some(scheme) = content_source.get_remote_endpoint().get_scheme() {
        match scheme {
            rust_extensions::remote_endpoint::Scheme::Http => {
                load_from_http_or_https(
                    app,
                    content_source.get_remote_endpoint(),
                    content_source.ssh_credentials.as_ref(),
                )
                .await
            }
            rust_extensions::remote_endpoint::Scheme::Https => {
                load_from_http_or_https(
                    app,
                    content_source.get_remote_endpoint(),
                    content_source.ssh_credentials.as_ref(),
                )
                .await
            }
            _ => {
                panic!(
                    "File can be loaded from the url: {}",
                    content_source.get_remote_endpoint().as_str()
                );
            }
        }
    } else {
        if let Some(ssh_credentials) = content_source.ssh_credentials.as_ref() {
            return loading_file_from_ssh(
                app,
                ssh_credentials,
                content_source.get_remote_endpoint(),
            )
            .await;
        } else {
            println!(
                "Loading file {}",
                content_source.remote_resource_string.as_str()
            );
            let file_name = LocalFilePath::new(content_source.remote_resource_string.to_string());

            let result = tokio::fs::read(file_name.get_value().as_str())
                .await
                .map_err(|err| {
                    format!(
                        "Error reading file: {:?}, error: {:?}",
                        file_name.get_value().as_str(),
                        err
                    )
                })?;

            Ok(result)
        }
    }
}

async fn load_from_http_or_https<'s>(
    app: &Arc<AppContext>,
    remote_endpoint: RemoteEndpoint<'s>,
    ssh_credentials: Option<&Arc<SshCredentials>>,
) -> Result<Vec<u8>, String> {
    if let Some(ssh_credentials) = ssh_credentials {
        return load_content_from_http_via_ssh(app, ssh_credentials, remote_endpoint).await;
    }

    let response = FlUrl::new(remote_endpoint.as_str())
        .get()
        .await
        .map_err(|err| format!("Error loading file from HTTP. Error: {:?}", err))?;

    let result = response
        .receive_body()
        .await
        .map_err(|itm| format!("Error loading file from HTTP. Error: {:?}", itm))?;

    Ok(result)
}

async fn loading_file_from_ssh<'s>(
    app: &Arc<AppContext>,
    ssh_credentials: &Arc<SshCredentials>,
    remote_endpoint: RemoteEndpoint<'s>,
) -> Result<Vec<u8>, String> {
    let ssh_session = super::ssh::get_ssh_session(app, ssh_credentials).await?;

    let result = ssh_session
        .download_remote_file(remote_endpoint.as_str(), Duration::from_secs(5))
        .await;

    if let Err(err) = result {
        return Err(format!(
            "Can not download file from remote resource {}->{}. Error: {:?}",
            ssh_credentials.to_string(),
            remote_endpoint.as_str(),
            err
        ));
    }

    let result = result.unwrap();

    Ok(result)
}

async fn load_content_from_http_via_ssh<'s>(
    app: &Arc<AppContext>,
    ssh_credentials: &Arc<SshCredentials>,
    remote_endpoint: RemoteEndpoint<'s>,
) -> Result<Vec<u8>, String> {
    use crate::http_client_connectors::HttpOverSshConnector;
    use my_ssh::*;

    let ssh_session = super::ssh::get_ssh_session(app, ssh_credentials).await?;

    let connector = HttpOverSshConnector {
        ssh_session,
        remote_endpoint: remote_endpoint.to_owned(),
        debug: false,
    };

    let http_client: MyHttpClient<SshAsyncChannel, HttpOverSshConnector> =
        MyHttpClient::new(connector);

    let http_request = MyHttpRequestBuilder::new(Method::GET, remote_endpoint.as_str()).build();

    let response = http_client
        .do_request(&http_request, Duration::from_secs(5))
        .await
        .map_err(|err| format!("{:?}", err))?;

    let response = response.into_response();

    let body = response.into_body();

    let body = body.collect().await.map_err(|err| format!("{:?}", err))?;

    let body = body.to_bytes();

    Ok(body.into())
}
