use crate::connection::Connection;
use crate::crypto::token::get_metadata_from_token;
use crate::db::P2PDatabase;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{
    Message, PeerFileGet, PeerSearchRequest, PeerUploadFile, PeerWaitConnection, Protocol,
    StorageReservationRequest, StorageValidTokenRequest, TransportData, TransportPacket,
};
use crate::tunnel::Tunnel;
use std::sync::Arc;

#[derive(Clone)]
pub struct PeerAPI {
    connection: Arc<Connection>,
    db: Arc<P2PDatabase>,
    manager: Arc<ConnectionManager>,
}

impl PeerAPI {
    pub fn new(connection: Arc<Connection>, db: &P2PDatabase, manager: &ConnectionManager) -> Self {
        PeerAPI {
            connection,
            db: Arc::new(db.clone()),
            manager: Arc::new(manager.clone()),
        }
    }

    pub async fn get_file(&self, peer_id: String, session_key: String) -> Result<(), String> {
        let packet = TransportPacket {
            act: "get_file".to_string(),
            to: Some(peer_id),
            data: Some(TransportData::PeerFileGet(PeerFileGet {
                session_key: session_key,
                peer_id: self.db.get_or_create_peer_id().unwrap(),
            })),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn upload_file(&self, peer_id: String, file_path: String) -> Result<(), String> {
        let contents = tokio::fs::read(&file_path)
            .await
            .map_err(|e| e.to_string())?;

        let my_peer_id = self.db.get_or_create_peer_id().unwrap();

        let packet = TransportPacket {
            act: "save_file".to_string(),
            to: Some(peer_id),
            data: Some(TransportData::PeerUploadFile(PeerUploadFile {
                filename: file_path,
                contents: base64::encode(contents),
                peer_id: my_peer_id.clone().to_string(),
            })),
            status: None,
            protocol: Protocol::TURN,
            uuid: my_peer_id.clone().to_string(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn send_message(&self, peer_id: String, message: String) -> Result<(), String> {
        let packet = TransportPacket {
            act: "message".to_string(),
            to: Some(peer_id),
            data: Some(TransportData::Message(Message { text: message })),
            status: None,
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn connect_to_peer(&self, peer_id: String) -> Result<(), String> {
        let tunnel = Tunnel::new().await;
        let tunnel_ip = tunnel.public_ip.clone();
        let tunnel_port = tunnel.public_port.clone();
        self.manager.add_tunnel(peer_id.to_string(), tunnel).await;
        let packet = TransportPacket {
            act: "wait_connection".to_string(),
            to: None,
            data: Some(TransportData::PeerWaitConnection(PeerWaitConnection {
                connect_peer_id: peer_id,
                public_port: tunnel_port,
                public_ip: tunnel_ip,
            })),
            status: None,
            protocol: Protocol::STUN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn request_peer_list(&self) -> Result<(), String> {
        let packet = TransportPacket {
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

    pub async fn reserve_storage(&self, size_in_bytes: u64) -> Result<(), String> {
        let packet = TransportPacket {
            act: "reserve_storage".to_string(),
            to: None,
            data: Some(TransportData::StorageReservationRequest(
                StorageReservationRequest {
                    peer_id: self.db.get_or_create_peer_id().unwrap(),
                    size_in_bytes,
                },
            )),
            status: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }

    pub async fn valid_token(&self, token: String) -> Result<(), String> {
        let metadata = get_metadata_from_token(token.clone().to_string()).await;

        if let Ok(metadata) = metadata {
            let packet = TransportPacket {
                act: "valid_token".to_string(),
                to: Some(metadata.storage_provider),
                data: Some(TransportData::StorageValidTokenRequest(
                    StorageValidTokenRequest {
                        token: token.clone(),
                        peer_id: self.db.get_or_create_peer_id().unwrap(),
                    },
                )),
                status: None,
                protocol: Protocol::SIGNAL,
                uuid: self.db.get_or_create_peer_id().unwrap(),
            };

            self.connection.send_packet(packet).await
        } else {
            Err("Invalid token".to_string())
        }
    }

    pub async fn search_peer(&self, peer_id: String) -> Result<(), String> {
        let packet = TransportPacket {
            act: "search_peer".to_string(),
            to: None,
            data: Some(TransportData::PeerSearchRequest(PeerSearchRequest {
                peer_id: self.db.get_or_create_peer_id().unwrap(),
                search_id: peer_id,
                max_hops: 3,
            })),
            status: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        self.connection.send_packet(packet).await
    }
}
