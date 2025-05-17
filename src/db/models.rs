use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Fragment {
    pub uuid_peer: String,
    pub token: String,
    pub filename: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Storage {
    pub token: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TokenInfo {
    pub token: String,
    pub free_space: u64,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ValidatorStorageInfo {
    pub peer_id: String,
    pub free_space: u64,
    pub total_space: u64,
    pub last_update: u64,
    pub is_online: bool,
} 