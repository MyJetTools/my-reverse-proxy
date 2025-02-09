use std::time::Duration;

use encryption::aes::AesKey;

const PING: u8 = 0;
const PONG: u8 = 1;
const HANDSHAKE_PACKET_ID: u8 = 2;
const CONNECT_PACKET_ID: u8 = 3;
const CONNECT_OK_PACKET_ID: u8 = 4;
const CONNECTION_ERROR_PACKET_ID: u8 = 5;
const SEND_PAYLOAD_PACKET_ID: u8 = 6;
const RECEIVE_PAYLOAD_PACKET_ID: u8 = 7;

#[derive(Debug)]
pub enum TcpGatewayContract<'s> {
    Handshake {
        timestamp: i64,
        gateway_name: &'s str,
    },
    Connect {
        connection_id: u32,
        timeout: Duration,
        remote_host: &'s str,
    },
    Connected {
        connection_id: u32,
    },
    ConnectionError {
        connection_id: u32,
        error: &'s str,
    },
    ForwardPayload {
        connection_id: u32,
        payload: &'s [u8],
    },
    BackwardPayload {
        connection_id: u32,
        payload: &'s [u8],
    },
    Ping,
    Pong,
}

impl<'s> TcpGatewayContract<'s> {
    pub fn parse(payload: &'s [u8]) -> Result<Self, String> {
        let packet_type = payload[0];
        let payload = &payload[1..];
        match packet_type {
            HANDSHAKE_PACKET_ID => {
                let timestamp = i64::from_le_bytes([
                    payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
                    payload[6], payload[7],
                ]);
                let gateway_name = std::str::from_utf8(&payload[8..]).map_err(|_| {
                    format!("Can not convert client_name to string during parsing Handshake")
                })?;
                return Ok(Self::Handshake {
                    gateway_name,
                    timestamp,
                });
            }

            CONNECT_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let timeout = payload[4];

                let remote_host = std::str::from_utf8(&payload[5..]).map_err(|_| {
                    format!("Can not convert remote_host to string during parsing Connect.")
                })?;
                return Ok(Self::Connect {
                    connection_id,
                    remote_host,
                    timeout: Duration::from_secs(timeout as u64),
                });
            }
            CONNECT_OK_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                return Ok(Self::Connected { connection_id });
            }
            CONNECTION_ERROR_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let error = std::str::from_utf8(&payload[4..]).map_err(|_| {
                    format!("Can not convert remote_host to string during parsing Connect.")
                })?;
                return Ok(Self::ConnectionError {
                    connection_id,
                    error,
                });
            }

            SEND_PAYLOAD_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let payload = &payload[4..];
                return Ok(Self::ForwardPayload {
                    connection_id,
                    payload,
                });
            }

            RECEIVE_PAYLOAD_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let payload = &payload[4..];
                return Ok(Self::BackwardPayload {
                    connection_id,
                    payload,
                });
            }

            PING => {
                return Ok(Self::Ping);
            }
            PONG => {
                return Ok(Self::Pong);
            }

            _ => {
                return Err(format!("Unknown packet type: {}", packet_type));
            }
        }
    }

    pub fn to_vec(&self, aes_key: &AesKey) -> Vec<u8> {
        let mut result = Vec::new();

        match self {
            Self::Handshake {
                gateway_name,
                timestamp,
            } => {
                result.push(HANDSHAKE_PACKET_ID);
                result.extend_from_slice(&timestamp.to_le_bytes());
                result.extend_from_slice(gateway_name.as_bytes());
            }
            Self::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                result.push(CONNECT_PACKET_ID);
                result.extend_from_slice(&connection_id.to_le_bytes());
                result.push(timeout.as_secs() as u8);
                result.extend_from_slice(remote_host.as_bytes());
            }

            Self::Connected { connection_id } => {
                result.push(CONNECT_OK_PACKET_ID);
                result.extend_from_slice(&connection_id.to_le_bytes());
            }
            Self::ConnectionError {
                connection_id,
                error,
            } => {
                result.push(CONNECTION_ERROR_PACKET_ID);
                result.extend_from_slice(&connection_id.to_le_bytes());
                result.extend_from_slice(error.as_bytes());
            }

            Self::ForwardPayload {
                connection_id,
                payload,
            } => {
                result.push(SEND_PAYLOAD_PACKET_ID);
                result.extend_from_slice(&connection_id.to_le_bytes());
                result.extend_from_slice(payload);
            }
            Self::BackwardPayload {
                connection_id,
                payload,
            } => {
                result.push(RECEIVE_PAYLOAD_PACKET_ID);
                result.extend_from_slice(&connection_id.to_le_bytes());
                result.extend_from_slice(payload);
            }
            Self::Ping => {
                result.push(PING);
            }
            Self::Pong => {
                result.push(PONG);
            }
        }

        let encrypted = aes_key.encrypt(&result);

        let encrypted = encrypted.as_slice();

        let len = encrypted.len();

        let mut result = Vec::with_capacity(len + 4);

        let len = (len as u32).to_le_bytes();

        result.extend_from_slice(&len);
        result.extend_from_slice(encrypted);

        result
    }
}
