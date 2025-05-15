#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct PeerWaitConnection {
    pub connect_peer_id: String,
    pub public_ip: String,
    pub public_port: u16
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
pub struct StunSyncPubAddr {
    pub public_addr: String
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StorageValidTokenRequest {
    pub token: String,
    pub peer_id: String,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StorageValidTokenResponse {
    pub peer_id: String,
    pub status: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StorageReservationRequest {
    pub peer_id: String,
    pub size_in_bytes: u64,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StorageToken {
    pub file_size: u64,                  // Размер файла
    pub storage_provider: String,      // Паблик-ключ (или крипто-адрес) хранителя
    pub timestamp: u64,                  // Unix-время создания токена
    pub signature: Vec<u8>,              // Подпись хранителя, подтверждающая согласие
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StorageReservationResponse {
    pub peer_id: String,
    pub token: String,  // base64 encoded StorageToken
    pub size_in_bytes: u64,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct PeerSearchRequest {
    pub search_id: String, // id поиска
    pub peer_id: String, // id пира инициатора поиска
    pub max_hops: u32, // максимальное количество прыжков
    pub path: Vec<SearchPathNode>, // путь поиска от инициатора до текущего узла
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct SearchPathNode {
    pub uuid: String,        // UUID узла
    pub public_addr: String, // Публичный адрес узла
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct PeerSearchResponse {
    pub search_id: String,     // id поиска
    pub peer_id: String,       // id пира инициатора поиска
    pub found_peer_id: String, // id пира найденного
    pub public_addr: String,   // публичный адрес ноды пира
    pub hops: u32,            // количество прыжков
    pub path: Vec<SearchPathNode>, // путь поиска от инициатора до нашедшего пира
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SignalServerInfo {
    pub public_key: String,
    pub public_ip: String,
    pub port: i64,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum TransportData {
    Message(Message),
    PeerInfo(PeerInfo),
    PeerWaitConnection(PeerWaitConnection),
    PeerFileGet(PeerFileGet),
    PeerUploadFile(PeerUploadFile),
    SyncPeerInfoData(SyncPeerInfoData),
    StorageReservationRequest(StorageReservationRequest),
    StorageReservationResponse(StorageReservationResponse),
    StorageValidTokenRequest(StorageValidTokenRequest),
    StorageValidTokenResponse(StorageValidTokenResponse),
    PeerFileSaved(PeerFileSaved),
    StunSyncPubAddr(StunSyncPubAddr),
    FileData(FileData),
    PeerSearchRequest(PeerSearchRequest),
    PeerSearchResponse(PeerSearchResponse),
    SignalServerInfo(SignalServerInfo),
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct TransportPacket {
    pub act: String,         //info, answer, wait_connection,
    pub to: Option<String>,  //кому отправляем данный пакет
    pub data: Option<TransportData>,
    pub protocol: Protocol,     // TURN, STUN, SIGNAL
    pub uuid: String,
}

impl std::fmt::Display for TransportPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "TransportPacket {{ act: {}, to: {:?}, protocol: {:?}, uuid: {} }}",
            self.act, self.to, self.protocol, format!("{:?}...{:?}", &self.uuid[0..5], &self.uuid[30..])
        )
    }
}