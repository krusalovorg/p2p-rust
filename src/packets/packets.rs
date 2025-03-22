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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SyncPeerInfo {
    pub public_addr: String,
    pub uuid: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SyncPeerInfoData {
    pub peers: Vec<SyncPeerInfo>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub enum Protocol {
    TURN,
    STUN,
    SIGNAL,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum Status {
    ERROR,
    SUCCESS,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct TransportPacket {
    pub public_addr: String, //к кому будет пытаться подключиться пир
    pub act: String, //info, answer, wait_connection,
    pub to: Option<String>, //кому отправляем данный пакет
    pub data: Option<serde_json::Value>,
    pub status: Option<Status>, // success, falied
    pub protocol: Protocol,     // TURN, STUN, SIGNAL
}