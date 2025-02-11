use std::{
    io::{Read, Write},
    time::Duration,
};

use encryption::aes::AesKey;
use rust_extensions::SliceOrVec;

const PING: u8 = 0;
const PONG: u8 = 1;
const HANDSHAKE_PACKET_ID: u8 = 2;
const CONNECT_PACKET_ID: u8 = 3;
const CONNECT_OK_PACKET_ID: u8 = 4;
const CONNECTION_ERROR_PACKET_ID: u8 = 5;
const SEND_PAYLOAD_PACKET_ID: u8 = 6;
const RECEIVE_PAYLOAD_PACKET_ID: u8 = 7;
const UPDATE_PING_TIME: u8 = 8;
const GET_FILE_REQUEST: u8 = 9;
const GET_FILE_RESPONSE: u8 = 10;

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
    Handshake {
        timestamp: i64,
        support_compression: bool,
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

                let support_compression = payload[8] == 1;
                let gateway_name = std::str::from_utf8(&payload[9..]).map_err(|_| {
                    format!("Can not convert client_name to string during parsing Handshake")
                })?;
                return Ok(Self::Handshake {
                    gateway_name,
                    support_compression,
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

                let compressed = payload[4] == 1;

                let payload = decompress_payload(&payload[5..], compressed)?;

                return Ok(Self::ForwardPayload {
                    connection_id,
                    payload,
                });
            }

            RECEIVE_PAYLOAD_PACKET_ID => {
                let connection_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let compressed = payload[4] == 1;

                let payload = decompress_payload(&payload[5..], compressed)?;
                return Ok(Self::BackwardPayload {
                    connection_id,
                    payload,
                });
            }

            UPDATE_PING_TIME => {
                let micros = u64::from_le_bytes([
                    payload[0], payload[1], payload[2], payload[3], payload[4], payload[5],
                    payload[6], payload[7],
                ]);

                let duration = Duration::from_micros(micros);

                return Ok(Self::UpdatePingTime { duration });
            }

            GET_FILE_REQUEST => {
                let request_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let path = std::str::from_utf8(&payload[4..]).map_err(|_| {
                    format!("Can not convert path to string during parsing GET_FILE_REQUEST.")
                })?;

                return Ok(Self::GetFileRequest { path, request_id });
            }

            GET_FILE_RESPONSE => {
                let request_id =
                    u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

                let status = GetFileStatus::from_u8(payload[4]);

                let content = extract_content(&payload[5..])?;

                return Ok(Self::GetFileResponse {
                    request_id,
                    status,
                    content,
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

    pub fn to_vec(&self, aes_key: &AesKey, support_compression: bool) -> Vec<u8> {
        let mut result = Vec::new();

        match self {
            Self::Handshake {
                gateway_name,
                support_compression,
                timestamp,
            } => {
                result.push(HANDSHAKE_PACKET_ID);
                result.extend_from_slice(&timestamp.to_le_bytes());

                if *support_compression {
                    result.push(1);
                } else {
                    result.push(0);
                }

                result.extend_from_slice(gateway_name.as_bytes());
            }
            Self::Connect {
                connection_id,
                timeout,
                remote_host,
            } => {
                result.push(CONNECT_PACKET_ID);
                push_u32(&mut result, *connection_id);
                result.push(timeout.as_secs() as u8);
                result.extend_from_slice(remote_host.as_bytes());
            }

            Self::Connected { connection_id } => {
                result.push(CONNECT_OK_PACKET_ID);
                push_u32(&mut result, *connection_id);
            }
            Self::ConnectionError {
                connection_id,
                error,
            } => {
                result.push(CONNECTION_ERROR_PACKET_ID);
                push_u32(&mut result, *connection_id);
                result.extend_from_slice(error.as_bytes());
            }

            Self::ForwardPayload {
                connection_id,
                payload,
            } => {
                result.push(SEND_PAYLOAD_PACKET_ID);
                push_u32(&mut result, *connection_id);
                push_content(&mut result, payload.as_slice(), support_compression);
            }
            Self::BackwardPayload {
                connection_id,
                payload,
            } => {
                result.push(RECEIVE_PAYLOAD_PACKET_ID);
                push_u32(&mut result, *connection_id);

                push_content(&mut result, payload.as_slice(), support_compression);
            }

            Self::UpdatePingTime { duration } => {
                let miros = duration.as_micros() as u64;
                result.push(UPDATE_PING_TIME);
                result.extend_from_slice(&miros.to_le_bytes());
            }
            Self::GetFileRequest { path, request_id } => {
                result.push(GET_FILE_REQUEST);
                result.extend_from_slice(request_id.to_le_bytes().as_slice());
                result.extend_from_slice(path.as_bytes());
            }
            Self::GetFileResponse {
                request_id,
                status,
                content,
            } => {
                result.push(GET_FILE_RESPONSE);
                result.extend_from_slice(request_id.to_le_bytes().as_slice());
                result.push(status.as_u8());
                push_content(&mut result, content.as_slice(), support_compression);
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

fn push_content(result: &mut Vec<u8>, payload: &[u8], support_compression: bool) {
    let (compressed, payload) = compress_payload_if_require(payload, support_compression);

    if compressed {
        result.push(1);
    } else {
        result.push(0);
    }

    if payload.get_len() > 0 {
        result.extend_from_slice(payload.as_slice());
    }
}

fn push_u32(result: &mut Vec<u8>, value: u32) {
    result.extend_from_slice(value.to_le_bytes().as_slice());
}

fn extract_content(payload: &[u8]) -> Result<SliceOrVec<'_, u8>, String> {
    let compressed = payload[0] == 1;

    decompress_payload(&payload[1..], compressed)
}

pub fn compress_payload_if_require<'s>(
    payload: &'s [u8],
    try_compress_payload: bool,
) -> (bool, SliceOrVec<'s, u8>) {
    use flate2::{write::GzEncoder, Compression};

    if try_compress_payload {
        if payload.len() < 64 {
            return (false, SliceOrVec::AsSlice(payload));
        }

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(payload).unwrap();
        let compressed = encoder.finish().unwrap();

        if compressed.len() < payload.len() {
            return (true, SliceOrVec::AsVec(compressed));
        }
    }

    (false, SliceOrVec::AsSlice(payload))
}

pub fn decompress_payload<'s>(
    src: &'s [u8],
    compressed: bool,
) -> Result<SliceOrVec<'s, u8>, String> {
    if !compressed {
        return Ok(src.into());
    }

    let mut decompressor = flate2::read::GzDecoder::new(src);

    let mut result = Vec::new();
    let mut buffer = [0u8; 1024 * 4];

    loop {
        let read_amount = decompressor.read(&mut buffer);

        if read_amount.is_err() {
            return Err("Can not decompress deflate payload".to_string());
        }

        let read_amount = read_amount.unwrap();

        if read_amount == 0 {
            return Ok(result.into());
        }

        result.extend_from_slice(&buffer[..read_amount]);
    }
}
