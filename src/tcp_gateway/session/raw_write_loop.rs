use std::{future::Future, pin::Pin};

use tokio::sync::mpsc;

use crate::network_stream::{MyOwnedWriteHalf, NetworkStreamWritePart};

type WriteFuture =
    Pin<Box<dyn Future<Output = (MyOwnedWriteHalf, Result<(), std::io::Error>)> + Send>>;

/// Raw passthrough writer task — used by `TcpGatewayForwardConnection` to
/// shovel bytes toward a downstream service. No encryption, no framing.
pub async fn raw_write_loop(write_half: MyOwnedWriteHalf, mut rx: mpsc::Receiver<Vec<u8>>) {
    let mut buffer: Vec<u8> = Vec::new();
    let mut write_fut: Option<WriteFuture> = None;
    let mut write_half_holder: Option<MyOwnedWriteHalf> = Some(write_half);

    loop {
        if write_fut.is_none() && !buffer.is_empty() {
            let batch = std::mem::take(&mut buffer);
            let mut owned_write_half = write_half_holder.take().expect("write_half was borrowed");
            write_fut = Some(Box::pin(async move {
                let res = owned_write_half.write_to_socket(&batch).await;
                (owned_write_half, res)
            }));
        }

        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some(payload) => buffer.extend_from_slice(&payload),
                    None => break,
                }
            }

            res = async { write_fut.as_mut().unwrap().as_mut().await }, if write_fut.is_some() => {
                let (returned_half, io_res) = res;
                write_half_holder = Some(returned_half);
                write_fut = None;
                if let Err(err) = io_res {
                    eprintln!("raw_write_loop: write failure: {:?}", err);
                    break;
                }
            }
        }
    }

    if let Some(fut) = write_fut.take() {
        let (returned_half, _) = fut.await;
        write_half_holder = Some(returned_half);
    }
    if let Some(mut half) = write_half_holder.take() {
        half.shutdown_socket().await;
    }
}
