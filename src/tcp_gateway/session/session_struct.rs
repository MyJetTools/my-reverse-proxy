use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicI64, AtomicU32, Ordering},
        Arc,
    },
};

use encryption::aes::AesKey;
use parking_lot::RwLock;
use rust_extensions::date_time::DateTimeAsMicroseconds;
use tokio::sync::{mpsc, oneshot};

pub type ConnectReplyTx = oneshot::Sender<Result<(), String>>;

pub struct Session {
    pub session_id: u64,

    pub write_tx: mpsc::Sender<Vec<u8>>,

    pub control_tx: mpsc::Sender<Vec<u8>>,

    pub slots: RwLock<HashMap<u32, mpsc::Sender<Vec<u8>>>>,

    pub pending: RwLock<HashMap<u32, ConnectReplyTx>>,

    pub aes_key: Arc<AesKey>,

    pub support_compression: bool,

    pub last_incoming_micros: AtomicI64,

    next_cid: AtomicU32,
}

impl Session {
    pub fn new(
        session_id: u64,
        write_tx: mpsc::Sender<Vec<u8>>,
        control_tx: mpsc::Sender<Vec<u8>>,
        aes_key: Arc<AesKey>,
        support_compression: bool,
    ) -> Self {
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        Self {
            session_id,
            write_tx,
            control_tx,
            slots: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            aes_key,
            support_compression,
            last_incoming_micros: AtomicI64::new(now),
            next_cid: AtomicU32::new(1),
        }
    }

    pub fn next_cid(&self) -> u32 {
        self.next_cid.fetch_add(1, Ordering::Relaxed)
    }

    pub fn mark_incoming(&self) {
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        self.last_incoming_micros.store(now, Ordering::Relaxed);
    }

    pub fn idle_micros(&self) -> i64 {
        let now = DateTimeAsMicroseconds::now().unix_microseconds;
        now - self.last_incoming_micros.load(Ordering::Relaxed)
    }

    pub fn insert_slot(&self, cid: u32, tx: mpsc::Sender<Vec<u8>>) {
        self.slots.write().insert(cid, tx);
    }

    pub fn remove_slot(&self, cid: u32) -> Option<mpsc::Sender<Vec<u8>>> {
        self.slots.write().remove(&cid)
    }

    pub fn get_slot(&self, cid: u32) -> Option<mpsc::Sender<Vec<u8>>> {
        self.slots.read().get(&cid).cloned()
    }

    pub fn insert_pending(&self, cid: u32, reply_to: ConnectReplyTx) {
        self.pending.write().insert(cid, reply_to);
    }

    pub fn take_pending(&self, cid: u32) -> Option<ConnectReplyTx> {
        self.pending.write().remove(&cid)
    }

    pub fn drain_pending(&self) -> Vec<(u32, ConnectReplyTx)> {
        self.pending.write().drain().collect()
    }

    pub fn drop_all_slots(&self) {
        self.slots.write().clear();
    }
}
