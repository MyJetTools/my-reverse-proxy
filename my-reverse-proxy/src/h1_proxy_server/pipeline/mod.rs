//! Task-decomposed HTTP/1.1 byte-pipe path (work in progress).
//!
//! Replaces the single `serve_reverse_proxy` loop + self-spawned
//! `response_read_loop` + shared `H1ServerWritePart` mutex-FIFO with three
//! linear roles connected by channels:
//! - a client-reader task: parses request heads, resolves the route, streams
//!   the request body to a per-request worker;
//! - a per-request worker task: owns an upstream connection, sends the head
//!   (reconnecting a stale reused socket via [`ReconnectPolicy`]), pumps the
//!   request body, reads the response, and emits [`ResponseEvent`]s;
//! - a client-writer task: the sole owner of the client write half, draining a
//!   FIFO of per-slot receivers in order.
//!
//! Built additively alongside the existing path; wired in once complete.
#![allow(dead_code)]

mod response_event;
pub use response_event::*;
mod client_writer;
pub use client_writer::*;
mod worker;
pub use worker::*;
mod reader;
pub use reader::*;
mod ws_tunnel;
pub use ws_tunnel::*;
