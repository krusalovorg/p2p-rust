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