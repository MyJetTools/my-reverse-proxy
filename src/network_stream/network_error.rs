#![allow(warnings)]
use std::time::Duration;

#[derive(Debug)]
pub enum NetworkError {
    Timeout(Duration),
    Disconnected,
    IoError(std::io::Error),
    Other(&'static str),
}
