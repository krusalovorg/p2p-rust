use std::sync::Arc;
use serde_json::json;
use crate::connection::Connection;
use crate::packets::{Protocol, TransportPacket};
use crate::db::P2PDatabase;

#[derive(Clone)]
pub struct PeerAPI {
    connection: Arc<Connection>,
    my_public_addr: Arc<String>,
    db: Arc<P2PDatabase>,
}

impl PeerAPI {
    pub fn new(connection: Arc<Connection>, my_public_addr: Arc<String>, db: &P2PDatabase) -> Self {
        PeerAPI {
            connection,
            my_public_addr,
            db: Arc::new(db.clone()),
        }
    }

    pub async fn get_file(&self, peer_id: String, session_key: String) -> Result<(), String> {
        let packet = TransportPacket {
            public_addr: self.my_public_addr.to_string(),
            act: "get_file".to_string(),
            to: Some(peer_id),
            data: Some(json!({"session_key": session_key})),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn upload_file(&self, peer_id: String, file_path: String) -> Result<(), String> {
        let contents = tokio::fs::read(&file_path).await.map_err(|e| e.to_string())?;
        
        let peer_upload_file = json!({
            "filename": file_path,
            "contents": base64::encode(contents),
            "peer_id": self.db.get_or_create_peer_id().unwrap(),
        });

        let packet = TransportPacket {
            public_addr: self.my_public_addr.to_string(),
            act: "save_file".to_string(),
            to: Some(peer_id),
            data: Some(peer_upload_file),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn send_message(&self, peer_id: String, message: String) -> Result<(), String> {
        let packet = TransportPacket {
            public_addr: self.my_public_addr.to_string(),
            act: "message".to_string(),
            to: Some(peer_id),
            data: Some(json!({"text": message})),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn connect_to_peer(&self, peer_id: String) -> Result<(), String> {
        let packet = TransportPacket {
            public_addr: self.my_public_addr.to_string(),
            act: "wait_connection".to_string(),
            to: None,
            data: Some(json!({
                "connect_peer_id": peer_id,
                "peer_id": self.db.get_or_create_peer_id().unwrap()
            })),
            status: None,
            protocol: Protocol::STUN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn request_peer_list(&self) -> Result<(), String> {
        let packet = TransportPacket {
            public_addr: self.my_public_addr.to_string(),
            act: "peer_list".to_string(),
            to: None,
            data: None,
            status: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };
        println!("{}", format!("[Peer] Sending peer list to signal server"));
        self.connection.send_packet(packet).await
    }
} 