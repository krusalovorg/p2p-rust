use crate::connection::Connection;
use crate::crypto::token::get_metadata_from_token;
use crate::db::P2PDatabase;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{
    EncryptedData, Message, PeerFileGet, PeerSearchRequest, PeerUploadFile, PeerWaitConnection, Protocol, SearchPathNode, StorageReservationRequest, StorageValidTokenRequest, TransportData, TransportPacket
};
use crate::tunnel::Tunnel;
use std::sync::Arc;

use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;

#[derive(Debug)]
pub enum UploadError {
    FileNotFound(String),
    NoTokensAvailable,
    InsufficientSpace { required: u64, available: u64 },
    DatabaseError(String),
    IoError(String),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::FileNotFound(path) => write!(f, "Файл не найден: {}", path),
            UploadError::NoTokensAvailable => write!(f, "Нет доступных токенов для загрузки. Используйте команду reserve для получения нового токена."),
            UploadError::InsufficientSpace { required, available } => write!(
                f,
                "Недостаточно места для загрузки файла. Требуется: {} байт, Доступно: {} байт. Используйте команду reserve для получения дополнительного места.",
                required, available
            ),
            UploadError::DatabaseError(e) => write!(f, "Ошибка базы данных: {}", e),
            UploadError::IoError(e) => write!(f, "Ошибка ввода/вывода: {}", e),
        }
    }
}

impl std::error::Error for UploadError {}

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

    pub async fn get_file(&self, filename: String) -> Result<(), String> {
        let my_peer_id = self.db.get_or_create_peer_id().unwrap();
        let files = self.db.get_myfile_fragments().unwrap();
        let file = files.get(&filename);
        if file.is_none() {
            return Err(format!("Файл не найден: {}", filename));
        }
        let file = file.unwrap();
        let token = file.token.clone();
        let uuid_peer = file.uuid_peer.clone();

        let packet = TransportPacket {
            act: "get_file".to_string(),
            to: Some(uuid_peer),
            data: Some(TransportData::PeerFileGet(PeerFileGet {
                token: token,
                peer_id: my_peer_id.clone(),
            })),
            protocol: Protocol::TURN,
            uuid: my_peer_id,
            nodes: vec![],
        };

        self.connection.send_packet(packet).await
    }

    pub async fn upload_file(&self, file_path: String) -> Result<(), UploadError> {
        println!("Uploading file: {}", file_path);
        let file_size = tokio::fs::metadata(&file_path)
            .await
            .map_err(|e| UploadError::FileNotFound(e.to_string()))?
            .len();

        println!("File size: {}", file_size);

        let (owner_peer_id, token_info) = self
            .db
            .get_best_token(file_size)
            .map_err(|e| UploadError::DatabaseError(e.to_string()))?
            .ok_or(UploadError::NoTokensAvailable)?;

        println!("Owner peer id: {}", owner_peer_id);
        println!("Token info: {}", token_info.token);

        let metadata = get_metadata_from_token(token_info.token.clone()).await;

        let token_provider = metadata.unwrap().storage_provider;

        if !self
            .manager
            .have_connection_with_peer(token_provider.clone())
            .await
        {
            self.connect_to_peer(token_provider.clone())
                .await
                .map_err(|e| UploadError::IoError(format!("Failed to connect to peer: {}", e)))?;

            let mut attempts = 0;
            let max_attempts = 30;

            while attempts < max_attempts {
                if self
                    .manager
                    .have_connection_with_peer(token_provider.clone())
                    .await
                {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                attempts += 1;
            }

            if attempts >= max_attempts {
                return Err(UploadError::IoError(
                    "Failed to establish connection with peer".to_string(),
                ));
            }
        }

        // ⏬ Сжатие файла
        let contents = tokio::fs::read(&file_path)
            .await
            .map_err(|e| UploadError::IoError(e.to_string()))?;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(&contents)
            .map_err(|e| UploadError::IoError(e.to_string()))?;
        let compressed_contents = encoder
            .finish()
            .map_err(|e| UploadError::IoError(e.to_string()))?;

        println!("Compressed size: {}", compressed_contents.len());

        let encrypted_contents = self
            .db
            .encrypt_data(&compressed_contents)
            .map_err(|e| UploadError::DatabaseError(e.to_string()))?;

        let final_content = serde_json::to_string(&EncryptedData {
            nonce: encrypted_contents.1,
            content: encrypted_contents.0,
        }).unwrap();

        let my_peer_id = self
            .db
            .get_or_create_peer_id()
            .map_err(|e| UploadError::DatabaseError(e.to_string()))?;

        let packet = TransportPacket {
            act: "save_file".to_string(),
            to: Some(token_provider),
            data: Some(TransportData::PeerUploadFile(PeerUploadFile {
                filename: file_path,
                contents: base64::encode(final_content.to_string().as_bytes()),
                peer_id: my_peer_id.clone(),
                token: token_info.token,
            })),
            protocol: Protocol::TURN,
            uuid: my_peer_id,
            nodes: vec![],
        };

        self.connection
            .send_packet(packet)
            .await
            .map_err(|e| UploadError::IoError(e.to_string()))
    }

    pub async fn send_message(&self, peer_id: String, message: String) -> Result<(), String> {
        let packet = TransportPacket {
            act: "message".to_string(),
            to: Some(peer_id),
            data: Some(TransportData::Message(Message {
                text: message,
                nonce: None,
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
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
            protocol: Protocol::STUN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        self.connection.send_packet(packet).await
    }

    pub async fn request_peer_list(&self) -> Result<(), String> {
        let packet = TransportPacket {
            act: "peer_list".to_string(),
            to: None,
            data: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
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
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
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
                protocol: Protocol::SIGNAL,
                uuid: self.db.get_or_create_peer_id().unwrap(),
                nodes: vec![],
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
                path: vec![SearchPathNode {
                    uuid: self.db.get_or_create_peer_id().unwrap(),
                    public_ip: self.connection.ip.clone(),
                    public_port: self.connection.port.clone(),
                }],
            })),
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        self.connection.send_packet(packet).await
    }
}
