use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fragment {
    pub uuid_peer: String,
    pub session_key: String,
    pub session: String,
    pub filename: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Storage {
    pub session_key: String,
    pub session: String,
    pub filename: String,
    pub owner_id: String,
    pub storage_peer_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SecretKeyInfo {
    pub owner_id: String,
    pub access_key: String,
    pub size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DHTEntry {
    pub peer_id: String,
    pub session_key: String,
    pub timestamp: u64,
} 