use bytes::Bytes;

use http_body_util::{combinators::BoxBody, Full};

use rust_extensions::TaskCompletion;
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, ReadHalf, WriteHalf},
    sync::Mutex,
};

use crate::http_proxy_pass::ProxyPassError;

use super::{BodyReader, HeadersReader, MyHttpClientWritePart, TcpBuffer};

const READ_TIMEOUT: Duration = Duration::from_secs(120);

pub enum HttpTask<TStream: tokio::io::AsyncRead + Send + Sync + 'static> {
    Response(hyper::Response<BoxBody<Bytes, String>>),
    WebsocketUpgrade {
        response: hyper::Response<BoxBody<Bytes, String>>,
        read_part: ReadHalf<TStream>,
    },
}

impl<TStream: tokio::io::AsyncRead + Send + Sync + 'static> HttpTask<TStream> {
    pub fn unwrap_response(self) -> hyper::Response<BoxBody<Bytes, String>> {
        match self {
            HttpTask::Response(response) => response,
            HttpTask::WebsocketUpgrade { response, .. } => response,
        }
    }

    pub fn unwrap_websocket_upgrade(
        self,
    ) -> (hyper::Response<BoxBody<Bytes, String>>, ReadHalf<TStream>) {
        match self {
            HttpTask::WebsocketUpgrade {
                response,
                read_part,
            } => (response, read_part),
            HttpTask::Response(_) => panic!("Can not unwrap as websocket upgrade"),
        }
    }
}

pub struct MyHttpClient<
    TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static,
> {
    writer: Arc<MyHttpClientWritePart<TStream>>,
    pub queue_of_requests: Arc<Mutex<VecDeque<TaskCompletion<HttpTask<TStream>, ProxyPassError>>>>,
}

impl<TStream: tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + 'static>
    MyHttpClient<TStream>
{
    pub fn new(stream: TStream) -> Self {
        let (reader, writer) = tokio::io::split(stream);

        let queue_of_requests = Arc::new(Mutex::new(VecDeque::new()));

        let queue_of_requests_spawned = queue_of_requests.clone();

        tokio::spawn(async move {
            read_task(reader, queue_of_requests_spawned).await;
        });

        let (sender, receiver) = tokio::sync::mpsc::channel(1024);
        let result = Self {
            writer: Arc::new(MyHttpClientWritePart::new(writer, sender)),
            queue_of_requests,
        };

        let writer_spawned = result.writer.clone();

        tokio::spawn(async move {
            super::my_http_client_write_part::write_loop(writer_spawned, receiver).await;
        });

        result
    }

    pub async fn send(
        &self,
        req: hyper::Request<Full<Bytes>>,
    ) -> Result<hyper::Response<BoxBody<Bytes, String>>, ProxyPassError> {
        let mut task = TaskCompletion::new();

        let awaiter = task.get_awaiter();

        {
            let mut queue = self.queue_of_requests.lock().await;
            queue.push_back(task);
        }

        self.writer.send(req).await;

        let result = awaiter.get_result().await?;
        Ok(result.unwrap_response())
    }

    pub async fn upgrade_to_web_socket(
        &self,
        req: hyper::Request<Full<Bytes>>,
        reunite: impl Fn(ReadHalf<TStream>, WriteHalf<TStream>) -> TStream,
    ) -> Result<(TStream, hyper::Response<BoxBody<Bytes, String>>), ProxyPassError> {
        let mut task = TaskCompletion::new();

        let awaiter = task.get_awaiter();

        {
            let mut queue = self.queue_of_requests.lock().await;
            queue.push_back(task);
        }

        self.writer.send(req).await;

        let result = awaiter.get_result().await?;

        let (response, read_part) = result.unwrap_websocket_upgrade();

        let write_part = self.writer.get_write_part().await;

        if write_part.is_none() {
            return Err(ProxyPassError::Disconnected);
        }

        let write_part = write_part.unwrap();

        let stream = reunite(read_part, write_part);

        Ok((stream, response))
    }
}

async fn read_task<TStream: tokio::io::AsyncRead + Send + Sync + 'static>(
    mut read: ReadHalf<TStream>,
    responses: Arc<Mutex<VecDeque<TaskCompletion<HttpTask<TStream>, ProxyPassError>>>>,
) {
    let mut tcp_buffer = TcpBuffer::new();

    let mut read_mode = ReadModel::Header(HeadersReader::new());

    let mut do_read_to_buffer = true;

    loop {
        if do_read_to_buffer {
            let result = read_to_buffer(&mut read, &mut tcp_buffer).await;
            if result.is_none() {
                break;
            }

            do_read_to_buffer = false;
        }

        match &mut read_mode {
            ReadModel::Header(headers_reader) => match headers_reader.read(&mut tcp_buffer) {
                Ok(mut body_reader) => {
                    if let Some(upgrade_response) = body_reader.try_into_web_socket_upgrade() {
                        let mut responses = responses.lock().await;
                        responses
                            .pop_front()
                            .unwrap()
                            .set_ok(HttpTask::WebsocketUpgrade {
                                response: upgrade_response,
                                read_part: read,
                            });

                        return;
                    }

                    read_mode = ReadModel::Body(body_reader);
                }
                Err(err) => match err {
                    super::HttpParseError::GetMoreData => {
                        do_read_to_buffer = true;
                    }
                    super::HttpParseError::Error(err) => {
                        println!("Http parser error: {}", err);
                        break;
                    }
                },
            },
            ReadModel::Body(body_reader) => {
                let response = body_reader.try_extract_response(&mut tcp_buffer);

                match response {
                    Ok(response) => {
                        {
                            let mut responses = responses.lock().await;
                            responses
                                .pop_front()
                                .unwrap()
                                .set_ok(HttpTask::Response(response));
                        }
                        read_mode = ReadModel::Header(HeadersReader::new());
                    }
                    Err(err) => match err {
                        super::HttpParseError::GetMoreData => {
                            do_read_to_buffer = true;
                        }
                        super::HttpParseError::Error(err) => {
                            println!("Http parser error: {}", err);
                            break;
                        }
                    },
                }
            }
        }
    }

    println!("Http client read task is done");
}

async fn read_to_buffer<TStream: tokio::io::AsyncRead>(
    read: &mut ReadHalf<TStream>,
    tcp_buffer: &mut TcpBuffer,
) -> Option<usize> {
    let write_buf = tcp_buffer.get_write_buf();

    if write_buf.len() == 0 {
        println!("Http Payload is too big");
        return None;
    }

    let result = tokio::time::timeout(READ_TIMEOUT, read.read(write_buf)).await;

    if result.is_err() {
        println!("Http client Read timeout");
        return None;
    }

    let result = result.unwrap();

    if let Err(err) = result {
        println!("Http client Read error: {:?}", err);
        return None;
    }

    let result = result.unwrap();

    if result == 0 {
        println!("Http client Read EOF");
        return None;
    }

    /*
    println!(
        "Read: [{}]",
        std::str::from_utf8(&write_buf[..result]).unwrap()
    );
     */

    tcp_buffer.add_read_amount(result);

    Some(result)
}

pub enum ReadModel {
    Header(HeadersReader),
    Body(BodyReader),
}
