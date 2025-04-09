#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerWaitConnection {
    pub peer_id: String,
    pub connect_peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerFileSaved {
    pub peer_id: String,
    pub filename: String,
    pub session_key: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerFileGet {
    pub peer_id: String,
    pub session_key: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerUploadFile {
    pub peer_id: String,
    pub filename: String,
    pub contents: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SyncPeerInfo {
    pub public_addr: String,
    pub uuid: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct Message {
    pub text: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct FileData {
    pub filename: String,
    pub contents: String,
    pub peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SaveFile {
    pub peer_id: String,
    pub filename: String,
    pub contents: String,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum TransportData {
    PeerInfo(PeerInfo),
    PeerWaitConnection(PeerWaitConnection),
    PeerFileSaved(PeerFileSaved),
    PeerFileGet(PeerFileGet),
    PeerUploadFile(PeerUploadFile),
    SyncPeerInfoData(SyncPeerInfoData),
    Message(Message),
    FileData(FileData),
    SaveFile(SaveFile),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct TransportPacket {
    pub public_addr: String, //к кому будет пытаться подключиться пир
    pub act: String,         //info, answer, wait_connection,
    pub to: Option<String>,  //кому отправляем данный пакет
    pub data: Option<TransportData>,
    pub status: Option<Status>, // success, falied
    pub protocol: Protocol,     // TURN, STUN, SIGNAL
    pub uuid: String,
}

impl std::fmt::Display for TransportPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TransportPacket {{ public_addr: {}, act: {}, to: {:?}, protocol: {:?}, uuid: {} }}",
            self.public_addr, self.act, self.to, self.protocol, self.uuid
        )
    }
}
