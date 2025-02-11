use std::sync::Arc;

use my_http_client::{MyHttpClientConnector, MyHttpClientError};
use rust_extensions::remote_endpoint::{RemoteEndpoint, RemoteEndpointOwned};
use tokio::io::{ReadHalf, WriteHalf};

use crate::{
    consts::DEFAULT_HTTP_CONNECT_TIMEOUT,
    tcp_gateway::forwarded_connection::TcpGatewayProxyForwardStream,
};

pub struct HttpOverGatewayConnector {
    pub remote_endpoint: Arc<RemoteEndpointOwned>,

    pub gateway_id: Arc<String>,
}

#[async_trait::async_trait]
impl MyHttpClientConnector<TcpGatewayProxyForwardStream> for HttpOverGatewayConnector {
    async fn connect(&self) -> Result<TcpGatewayProxyForwardStream, MyHttpClientError> {
        let gateway = crate::app::APP_CTX
            .get_gateway_by_id_with_next_connection_id(self.gateway_id.as_str())
            .await;

        if gateway.is_none() {
            return Err(MyHttpClientError::CanNotConnectToRemoteHost(format!(
                "Gateway {} is not found",
                self.gateway_id
            )));
        }
        let (gateway, connection_id) = gateway.unwrap();

        match gateway
            .connect_to_forward_proxy_connection(
                self.remote_endpoint.clone(),
                DEFAULT_HTTP_CONNECT_TIMEOUT,
                connection_id,
            )
            .await
        {
            Ok(result) => Ok(result),
            Err(err) => Err(MyHttpClientError::CanNotConnectToRemoteHost(err)),
        }
    }
    fn get_remote_endpoint(&self) -> RemoteEndpoint {
        self.remote_endpoint.to_ref()
    }
    fn is_debug(&self) -> bool {
        false
    }

    fn reunite(
        read: ReadHalf<TcpGatewayProxyForwardStream>,
        write: WriteHalf<TcpGatewayProxyForwardStream>,
    ) -> TcpGatewayProxyForwardStream {
        read.unsplit(write)
    }
}
