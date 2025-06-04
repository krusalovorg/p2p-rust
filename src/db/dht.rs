use crate::db::{models::DHTEntry, tables::DHT_TABLE};
use redb::{Error, ReadableTable};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct PeerConnection {
    pub peer_id: String,
    pub connected_peer_id: String,
    pub timestamp: u64,
    pub is_active: bool,
}

use super::P2PDatabase;

impl P2PDatabase {

} 