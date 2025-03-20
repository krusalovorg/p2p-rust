#[derive(serde::Serialize, serde::Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PeerWaitConnection {
    pub peer_id: String,
    pub connect_peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PeerFileSaved {
    pub peer_id: String,
    pub filename: String,
    pub session_key: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PeerFileGet {
    pub peer_id: String,
    pub filename: String,
    pub session_key: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PeerUploadFile {
    pub peer_id: String,
    pub filename: String,
    pub contents: String,
}