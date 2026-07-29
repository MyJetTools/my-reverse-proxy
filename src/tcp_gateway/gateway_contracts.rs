use std::time::Duration;

use encryption::aes::AesKey;
use rust_extensions::SliceOrVec;

const PING: u8 = 0;
const PONG: u8 = 1;
const CONNECT_PACKET_ID: u8 = 3;
const CONNECT_OK_PACKET_ID: u8 = 4;
const CONNECTION_ERROR_PACKET_ID: u8 = 5;
const SEND_PAYLOAD_PACKET_ID: u8 = 6;
const RECEIVE_PAYLOAD_PACKET_ID: u8 = 7;
const UPDATE_PING_TIME_PACKET_ID: u8 = 8;
const GET_FILE_REQUEST_PACKET_ID: u8 = 9;
const GET_FILE_RESPONSE_PACKET_ID: u8 = 10;
const SYNC_SSL_CERTIFICATES_PACKET_ID: u8 = 11;
const SYNC_SSL_CERTIFICATES_REQUEST_PACKET_ID: u8 = 12;
const SYNC_SSL_CERTIFICATE_NOT_FOUND_PACKET_ID: u8 = 13;
pub const COMPRESSED_BATCH_PACKET_ID: u8 = 20;

pub const COMPRESSION_ALGO_ZSTD: u8 = 0;

#[derive(Debug)]
pub enum GetFileStatus {
    Ok,
    Error,
}

impl GetFileStatus {
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::Ok => 0,
            Self::Error => 1,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Ok,
            _ => Self::Error,
        }
    }
}

#[derive(Debug)]
pub enum TcpGatewayContract<'s> {
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
        payload: SliceOrVec<'s, u8>,
    },
    BackwardPayload {
        connection_id: u32,
        payload: SliceOrVec<'s, u8>,
    },
    Ping,
    Pong,
    UpdatePingTime {
        duration: Duration,
    },
    GetFileRequest {
        path: &'s str,
        request_id: u32,
    },
    GetFileResponse {
        request_id: u32,
        status: GetFileStatus,
        content: SliceOrVec<'s, u8>,
    },
    SyncSslCertificates {
        cert_id: &'s str,
        cert_pem: SliceOrVec<'s, u8>,
        private_key_pem: SliceOrVec<'s, u8>,
    },
    SyncSslCertificatesRequest {
        cert_ids: Vec<&'s str>,
    },
    SyncSslCertificateNotFound {
        cert_id: &'s str,
    },
}

impl<'s> TcpGatewayContract<'s> {
    /// Parse a single inner frame body: `[u8 TYPE][PAYLOAD]`. The caller has
    /// already decrypted and (if needed) decompressed the bytes. Frames of
    /// type `COMPRESSED_BATCH` must be unwrapped by the caller before being
    /// passed here.
    pub fn parse(payload: &'s [u8]) -> Result<Self, String> {
        if payload.is_empty() {
            return Err("Empty packet".to_string());
        }
        let packet_type = payload[0];
        let payload = &payload[1..];
        match packet_type {
            CONNECT_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let timeout = payload[4];

                let remote_host = convert_to_string(&payload[5..], "CONNECT")?;

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

                let error = convert_to_string(&payload[4..], "CONNECTION")?;
                return Ok(Self::ConnectionError {
                    connection_id,
                    error,
                });
            }

            SEND_PAYLOAD_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let payload_bytes: SliceOrVec<'_, u8> = (&payload[4..]).into();

                return Ok(Self::ForwardPayload {
                    connection_id,
                    payload: payload_bytes,
                });
            }

            RECEIVE_PAYLOAD_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let payload_bytes: SliceOrVec<'_, u8> = (&payload[4..]).into();

                return Ok(Self::BackwardPayload {
                    connection_id,
                    payload: payload_bytes,
                });
            }

            UPDATE_PING_TIME_PACKET_ID => {
                let micros = u64::from_le_bytes([
                    payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
                    payload[6], payload[7],
                ]);

                let duration = Duration::from_micros(micros);

                return Ok(Self::UpdatePingTime { duration });
            }

            GET_FILE_REQUEST_PACKET_ID => {
                let request_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let path = convert_to_string(&payload[4..], "GET_FILE_REQUEST")?;

                return Ok(Self::GetFileRequest { path, request_id });
            }

            GET_FILE_RESPONSE_PACKET_ID => {
                let request_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let status = GetFileStatus::from_u8(payload[4]);

                let content: SliceOrVec<'_, u8> = (&payload[5..]).into();

                return Ok(Self::GetFileResponse {
                    request_id,
                    status,
                    content,
                });
            }

            SYNC_SSL_CERTIFICATE_NOT_FOUND_PACKET_ID => {
                let mut offset = 0usize;
                let id_len = read_u32(payload, offset)? as usize;
                offset += 4;
                let id_end = offset + id_len;
                if payload.len() < id_end {
                    return Err("SYNC_SSL_CERTIFICATE_NOT_FOUND: truncated cert_id".to_string());
                }
                let cert_id = std::str::from_utf8(&payload[offset..id_end]).map_err(|_| {
                    "SYNC_SSL_CERTIFICATE_NOT_FOUND: invalid UTF-8 in cert_id".to_string()
                })?;

                return Ok(Self::SyncSslCertificateNotFound { cert_id });
            }

            SYNC_SSL_CERTIFICATES_REQUEST_PACKET_ID => {
                let mut offset = 0usize;
                let count = read_u32(payload, offset)? as usize;
                offset += 4;

                let mut cert_ids = Vec::with_capacity(count);
                for _ in 0..count {
                    let id_len = read_u32(payload, offset)? as usize;
                    offset += 4;
                    let id_end = offset + id_len;
                    if payload.len() < id_end {
                        return Err("CLIENT_SSL_CERT_REQUEST: truncated cert_id".to_string());
                    }
                    let id = std::str::from_utf8(&payload[offset..id_end]).map_err(|_| {
                        "CLIENT_SSL_CERT_REQUEST: invalid UTF-8 in cert_id".to_string()
                    })?;
                    offset = id_end;
                    cert_ids.push(id);
                }

                return Ok(Self::SyncSslCertificatesRequest { cert_ids });
            }

            SYNC_SSL_CERTIFICATES_PACKET_ID => {
                let mut offset = 0usize;

                let cert_id_len = read_u32(payload, offset)? as usize;
                offset += 4;
                let cert_id_end = offset + cert_id_len;
                if payload.len() < cert_id_end {
                    return Err("SERVER_SSL_CERT_PUSH: truncated cert_id".to_string());
                }
                let cert_id = std::str::from_utf8(&payload[offset..cert_id_end])
                    .map_err(|_| "SERVER_SSL_CERT_PUSH: invalid UTF-8 in cert_id".to_string())?;
                offset = cert_id_end;

                let cert_pem_len = read_u32(payload, offset)? as usize;
                offset += 4;
                let cert_pem_end = offset + cert_pem_len;
                if payload.len() < cert_pem_end {
                    return Err("SERVER_SSL_CERT_PUSH: truncated cert_pem".to_string());
                }
                let cert_pem = &payload[offset..cert_pem_end];
                offset = cert_pem_end;

                let pk_pem_len = read_u32(payload, offset)? as usize;
                offset += 4;
                let pk_pem_end = offset + pk_pem_len;
                if payload.len() < pk_pem_end {
                    return Err("SERVER_SSL_CERT_PUSH: truncated private_key_pem".to_string());
                }
                let private_key_pem = &payload[offset..pk_pem_end];

                return Ok(Self::SyncSslCertificates {
                    cert_id,
                    cert_pem: SliceOrVec::AsSlice(cert_pem),
                    private_key_pem: SliceOrVec::AsSlice(private_key_pem),
                });
            }

            PING => {
                return Ok(Self::Ping);
            }
            PONG => {
                return Ok(Self::Pong);
            }

            COMPRESSED_BATCH_PACKET_ID => {
                return Err(
                    "COMPRESSED_BATCH must be unwrapped by the read loop, not by parse()"
                        .to_string(),
                );
            }

            _ => {
                return Err(format!("Unknown packet type: {}", packet_type));
            }
        }
    }

    /// Serialize the contract into its plaintext inner-frame form:
    /// `[u32 LEN][u8 TYPE][PAYLOAD]`. Encryption and outer framing happen at
    /// the writer task on flush, after deciding whether to wrap multiple
    /// frames into a `COMPRESSED_BATCH`.
    pub fn to_plain_frame(&self) -> Vec<u8> {
        let mut body = Vec::new();

        match self {
            Self::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                body.push(CONNECT_PACKET_ID);
                push_u32(&mut body, *connection_id);
                body.push(timeout.as_secs() as u8);
                body.extend_from_slice(remote_host.as_bytes());
            }

            Self::Connected { connection_id } => {
                body.push(CONNECT_OK_PACKET_ID);
                push_u32(&mut body, *connection_id);
            }
            Self::ConnectionError {
                connection_id,
                error,
            } => {
                body.push(CONNECTION_ERROR_PACKET_ID);
                push_u32(&mut body, *connection_id);
                body.extend_from_slice(error.as_bytes());
            }

            Self::ForwardPayload {
                connection_id,
                payload,
            } => {
                body.push(SEND_PAYLOAD_PACKET_ID);
                push_u32(&mut body, *connection_id);
                body.extend_from_slice(payload.as_slice());
            }
            Self::BackwardPayload {
                connection_id,
                payload,
            } => {
                body.push(RECEIVE_PAYLOAD_PACKET_ID);
                push_u32(&mut body, *connection_id);
                body.extend_from_slice(payload.as_slice());
            }

            Self::UpdatePingTime { duration } => {
                let micros = duration.as_micros() as u64;
                body.push(UPDATE_PING_TIME_PACKET_ID);
                body.extend_from_slice(&micros.to_le_bytes());
            }
            Self::GetFileRequest { path, request_id } => {
                body.push(GET_FILE_REQUEST_PACKET_ID);
                body.extend_from_slice(request_id.to_le_bytes().as_slice());
                body.extend_from_slice(path.as_bytes());
            }
            Self::GetFileResponse {
                request_id,
                status,
                content,
            } => {
                body.push(GET_FILE_RESPONSE_PACKET_ID);
                body.extend_from_slice(request_id.to_le_bytes().as_slice());
                body.push(status.as_u8());
                body.extend_from_slice(content.as_slice());
            }
            Self::SyncSslCertificates {
                cert_id,
                cert_pem,
                private_key_pem,
            } => {
                body.push(SYNC_SSL_CERTIFICATES_PACKET_ID);

                let id_bytes = cert_id.as_bytes();
                push_u32(&mut body, id_bytes.len() as u32);
                body.extend_from_slice(id_bytes);

                let cp = cert_pem.as_slice();
                push_u32(&mut body, cp.len() as u32);
                body.extend_from_slice(cp);

                let pk = private_key_pem.as_slice();
                push_u32(&mut body, pk.len() as u32);
                body.extend_from_slice(pk);
            }
            Self::SyncSslCertificatesRequest { cert_ids } => {
                body.push(SYNC_SSL_CERTIFICATES_REQUEST_PACKET_ID);
                push_u32(&mut body, cert_ids.len() as u32);
                for id in cert_ids {
                    let bytes = id.as_bytes();
                    push_u32(&mut body, bytes.len() as u32);
                    body.extend_from_slice(bytes);
                }
            }
            Self::SyncSslCertificateNotFound { cert_id } => {
                body.push(SYNC_SSL_CERTIFICATE_NOT_FOUND_PACKET_ID);
                let bytes = cert_id.as_bytes();
                push_u32(&mut body, bytes.len() as u32);
                body.extend_from_slice(bytes);
            }
            Self::Ping => {
                body.push(PING);
            }
            Self::Pong => {
                body.push(PONG);
            }
        }

        let len = body.len();
        let mut result = Vec::with_capacity(4 + len);
        result.extend_from_slice(&(len as u32).to_le_bytes());
        result.extend_from_slice(&body);
        result
    }
}

/// Encrypt a plaintext inner-frame body and frame it for the wire.
/// `body` is `[u8 TYPE][PAYLOAD]` (without the length prefix).
pub fn encrypt_frame(body: &[u8], aes: &AesKey) -> Vec<u8> {
    let encrypted = aes.encrypt(body);
    let encrypted = encrypted.as_slice();
    let len = (encrypted.len() as u32).to_le_bytes();
    let mut out = Vec::with_capacity(4 + encrypted.len());
    out.extend_from_slice(&len);
    out.extend_from_slice(encrypted);
    out
}

fn push_u32(result: &mut Vec<u8>, value: u32) {
    result.extend_from_slice(value.to_le_bytes().as_slice());
}

fn read_u32(payload: &[u8], offset: usize) -> Result<u32, String> {
    if payload.len() < offset + 4 {
        return Err("truncated u32".to_string());
    }
    Ok(u32::from_le_bytes([
        payload[offset],
        payload[offset + 1],
        payload[offset + 2],
        payload[offset + 3],
    ]))
}

fn convert_to_string<'s>(payload: &'s [u8], packet_type: &str) -> Result<&'s str, String> {
    if payload.is_empty() {
        return Ok("");
    }

    std::str::from_utf8(payload)
        .map_err(|_| format!("Can not convert path to string during parsing [{packet_type}]."))
}
