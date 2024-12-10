use std::{sync::Arc, time::Duration};

use my_ssh::SshAsyncChannel;
use rust_extensions::date_time::{AtomicDateTimeAsMicroseconds, DateTimeAsMicroseconds};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::Mutex,
};

pub async fn copy_loop(
    mut reader: impl AsyncReadExt + Unpin,
    writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,
    buffer_size: usize,
    debug: bool,
) {
    let mut buf = Vec::with_capacity(buffer_size);

    unsafe {
        buf.set_len(buffer_size);
    }

    let write_timeout = Duration::from_secs(30);

    loop {
        let read_result = reader.read(&mut buf).await;

        let mut writer_access = writer.lock().await;

        if read_result.is_err() {
            let _ = writer_access.shutdown().await;
            break;
        }

        let n = read_result.unwrap();

        if n == 0 {
            let _ = writer_access.shutdown().await;
            break;
        }
        incoming_traffic_moment.update(DateTimeAsMicroseconds::now());

        let write_future = writer_access.write_all(&buf[0..n]);

        let result = tokio::time::timeout(write_timeout, write_future).await;

        //Got Timeout on writer
        if result.is_err() {
            let err = writer_access.shutdown().await;
            if debug {
                println!("Timeout on tcp: Shutdown socket got error: {:?}", err);
            }
            break;
        }

        let result = result.unwrap();

        if result.is_err() {
            let err = writer_access.shutdown().await;

            if debug {
                if let Err(err) = err {
                    println!("Timeout on Shutting down tcp socket: {:?}", err);
                }
            }
        }
    }
}

pub async fn copy_to_ssh_loop(
    mut reader: impl AsyncReadExt + Unpin,
    writer: Arc<Mutex<futures::io::WriteHalf<SshAsyncChannel>>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,
    buffer_size: usize,
) {
    use futures::AsyncWriteExt;
    let mut buf = Vec::with_capacity(buffer_size);

    unsafe {
        buf.set_len(buffer_size);
    }

    let write_timeout = Duration::from_secs(30);

    loop {
        let read_result = reader.read(&mut buf).await;

        let mut writer_access = writer.lock().await;

        if read_result.is_err() {
            let _ = writer_access.close().await;
            break;
        }

        let n = read_result.unwrap();

        if n == 0 {
            let _ = writer_access.close().await;
            break;
        }
        incoming_traffic_moment.update(DateTimeAsMicroseconds::now());

        let write_future = writer_access.write_all(&buf[0..n]);

        let result = tokio::time::timeout(write_timeout, write_future).await;

        //Got Timeout on writer
        if result.is_err() {
            let _ = writer_access.close().await;
            break;
        }

        let result = result.unwrap();

        if result.is_err() {
            let _ = writer_access.close().await;
        }
    }
}

pub async fn copy_from_ssh_loop(
    mut reader: futures::io::ReadHalf<SshAsyncChannel>,
    writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,
    buffer_size: usize,
) {
    use futures::AsyncReadExt;
    let mut buf = Vec::with_capacity(buffer_size);

    unsafe {
        buf.set_len(buffer_size);
    }

    let write_timeout = Duration::from_secs(30);

    loop {
        let read_result = reader.read(&mut buf).await;

        let mut writer_access = writer.lock().await;

        if read_result.is_err() {
            let _ = writer_access.shutdown().await;
            break;
        }

        let n = read_result.unwrap();

        if n == 0 {
            let _ = writer_access.shutdown().await;
            break;
        }
        incoming_traffic_moment.update(DateTimeAsMicroseconds::now());

        let write_future = writer_access.write_all(&buf[0..n]);

        let result = tokio::time::timeout(write_timeout, write_future).await;

        //Got Timeout on writer
        if result.is_err() {
            let _ = writer_access.shutdown().await;
            break;
        }

        let result = result.unwrap();

        if result.is_err() {
            let _ = writer_access.shutdown().await;
        }
    }
}

/*
pub async fn copy_from_remote_loop(
    mut remote_reader: impl AsyncReadExt + Unpin,
    local_writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    buffer_size: usize,
) {
    let mut buf = Vec::with_capacity(buffer_size);

    unsafe {
        buf.set_len(buffer_size);
    }

    loop {
        let read_result = remote_reader.read(&mut buf).await;
        let mut local_writer_access = local_writer.lock().await;

        if read_result.is_err() {
            let _ = local_writer_access.shutdown().await;
            break;
        }

        let n = read_result.unwrap();

        if n == 0 {
            let _ = local_writer_access.shutdown().await;
            break;
        }
        let result = local_writer_access.write_all(&buf[0..n]).await;

        if result.is_err() {
            let _ = local_writer_access.shutdown().await;
        }
    }
}
 */
pub async fn await_while_alive(
    local_writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    remote_writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,

    print_detected: impl Fn() -> (),
) {
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let now = DateTimeAsMicroseconds::now();

        let last_incoming_traffic =
            DateTimeAsMicroseconds::new(incoming_traffic_moment.get_unix_microseconds());

        if now
            .duration_since(last_incoming_traffic)
            .as_positive_or_zero()
            > Duration::from_secs(60)
        {
            print_detected();

            {
                let mut remote_writer = remote_writer.lock().await;
                let _ = remote_writer.shutdown().await;
            }

            {
                let mut local_writer = local_writer.lock().await;
                let _ = local_writer.shutdown().await;
            }

            break;
        }
    }
}

pub async fn await_while_alive_with_ssh(
    local_writer: Arc<Mutex<impl AsyncWriteExt + Unpin>>,
    remote_writer: Arc<Mutex<futures::io::WriteHalf<SshAsyncChannel>>>,
    incoming_traffic_moment: Arc<AtomicDateTimeAsMicroseconds>,

    print_detected: impl Fn() -> (),
) {
    use futures::AsyncWriteExt;
    loop {
        tokio::time::sleep(Duration::from_secs(10)).await;

        let now = DateTimeAsMicroseconds::now();

        let last_incoming_traffic =
            DateTimeAsMicroseconds::new(incoming_traffic_moment.get_unix_microseconds());

        if now
            .duration_since(last_incoming_traffic)
            .as_positive_or_zero()
            > Duration::from_secs(60)
        {
            print_detected();

            {
                let mut remote_writer = remote_writer.lock().await;
                let _ = remote_writer.close().await;
            }

            {
                let mut local_writer = local_writer.lock().await;
                let _ = local_writer.shutdown().await;
            }

            break;
        }
    }
}
