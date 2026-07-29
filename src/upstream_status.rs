use std::sync::atomic::{AtomicU8, Ordering};

/// Result of the most recent upstream interaction (connect / revive / health
/// ping). Surfaced to the admin UI to flag pools whose last attempt failed.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpstreamStatus {
    Unknown = 0,
    Ok = 1,
    Error = 2,
}

impl UpstreamStatus {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Ok,
            2 => Self::Error,
            _ => Self::Unknown,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Default)]
pub struct AtomicUpstreamStatus(AtomicU8);

impl AtomicUpstreamStatus {
    pub fn new() -> Self {
        Self(AtomicU8::new(UpstreamStatus::Unknown.as_u8()))
    }

    pub fn set(&self, status: UpstreamStatus) {
        self.0.store(status.as_u8(), Ordering::Relaxed);
    }

    pub fn get(&self) -> UpstreamStatus {
        UpstreamStatus::from_u8(self.0.load(Ordering::Relaxed))
    }
}
