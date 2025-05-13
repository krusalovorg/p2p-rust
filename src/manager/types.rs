#[derive(Debug, Clone)]
pub struct ConnectionTurnStatus {
    pub connected: bool,
    pub turn_connection: bool,
}

pub enum ConnectionType {
    Signal(String),
    Stun,
}

#[derive(Debug, Clone)]
pub struct PeerOpenNetInfo {
    pub ip: String,
    pub port: u16,
}
