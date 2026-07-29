use tokio::sync::mpsc;

use crate::network_stream::*;

use super::{ResponseEvent, ResponseSlot};

/// The single owner of the client write half. It drains an ordered queue of
/// response slots (FIFO = response order) and writes each slot's events to the
/// client to completion before taking the next slot. Because it is the sole
/// writer, there is no shared mutex on the socket — ordering is structural (the
/// queue), not bookkept.
///
/// Returns `Some(write_part)` when the queue closes cleanly (the reader dropped
/// its sender with no in-flight failure) — the connection entry takes the write
/// half back to build a websocket tunnel. Returns `None` when the connection had
/// to be closed (write failure, aborted or incomplete response): the write half
/// has been shut down and must not be reused.
pub async fn run_client_writer<WritePart: NetworkStreamWritePart + Send + Sync + 'static>(
    mut write_part: WritePart,
    mut queue: mpsc::Receiver<ResponseSlot>,
) -> Option<WritePart> {
    while let Some(mut slot) = queue.recv().await {
        let mut finished_cleanly = false;
        while let Some(event) = slot.events.recv().await {
            match event {
                ResponseEvent::Chunk(bytes) => {
                    if write_part
                        .write_all_with_timeout(&bytes, slot.write_timeout)
                        .await
                        .is_err()
                    {
                        // Client gone — abandon the whole connection.
                        return None;
                    }
                }
                ResponseEvent::Done => {
                    finished_cleanly = true;
                    break;
                }
                ResponseEvent::Abort => {
                    // Response truncated mid-stream; the only correct H1 move is
                    // to close the connection.
                    write_part.shutdown_socket().await;
                    return None;
                }
            }
        }

        if !finished_cleanly {
            // The worker dropped its sender without Done/Abort (it died): the
            // response is incomplete, so close rather than serve the next slot
            // on a desynced connection.
            write_part.shutdown_socket().await;
            return None;
        }
    }

    let _ = write_part.flush_it().await;
    Some(write_part)
}
