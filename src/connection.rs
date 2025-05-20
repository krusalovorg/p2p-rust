use std::sync::Arc;
use std::time::Duration;

use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task;
use tokio::time::sleep;

use crate::crypto::crypto::generate_uuid;
use crate::packets::{Protocol, TransportPacket, TransportData, PeerInfo};
use crate::db::P2PDatabase;

const SHOW_LOGS: bool = false;

fn log(message: &str) {
    if SHOW_LOGS {
        println!("{}", message);
    }
}

#[derive(Debug)]
pub enum Message {
    SendData(TransportPacket),
    GetResponse {
        tx: oneshot::Sender<TransportPacket>,
    },
}

#[derive(Clone)]
pub struct Connection {
    pub tx: mpsc::Sender<Message>,
    writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
    reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
    pub ip: String,
    pub port: i64,
    db: Arc<P2PDatabase>,
}

impl Connection {
    pub async fn new(
        signal_server_ip: String,
        signal_server_port: i64,
        db: &P2PDatabase,
    ) -> Connection {
        let (tx, rx) = mpsc::channel(16);

        let stream = TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port))
            .await
            .unwrap();
        let (reader, writer) = split(stream);

        let reader = Arc::new(RwLock::new(reader));
        let writer = Arc::new(RwLock::new(writer));

        // Отправляем пакет при создании соединения
        let connect_packet = TransportPacket {
            act: "info".to_string(),
            to: None,
            data: Some(
                TransportData::PeerInfo(PeerInfo {
                    is_signal_server: false,
                    total_space: db.get_storage_size().await.unwrap_or(0),
                    free_space: db.get_storage_free_space().await.unwrap_or(0),
                    stored_files: Vec::new(),
                    public_key: db.get_or_create_peer_id().unwrap(),
                }),
            ),
            protocol: Protocol::SIGNAL,
            peer_key: db.get_or_create_peer_id().unwrap(),
            uuid: generate_uuid(),
            nodes: vec![],
        };

        if let Err(e) = Self::write_packet(&writer, &connect_packet).await {
            log(&format!("[Connection] Failed to send connect packet: {}", e));
        } else {
            log("[Connection] Connect packet sent successfully");
        }

        task::spawn(Self::process_messages(
            tx.clone(),
            rx,
            reader.clone(),
            writer.clone(),
            Arc::new(db.clone()),
        ));

        Connection { 
            tx, 
            writer, 
            reader,
            ip: signal_server_ip,
            port: signal_server_port,
            db: Arc::new(db.clone()),
        }
    }
    
    async fn write_packet(
        writer: &Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        packet: &TransportPacket,
    ) -> Result<(), String> {
        let packet_str = serde_json::to_string(&packet).unwrap();
        let packet_len = packet_str.len() as u32;
        let mut writer = writer.write().await;

        // Отправляем длину сообщения (4 байта)
        let len_bytes = packet_len.to_be_bytes();
        if let Err(e) = writer.write_all(&len_bytes).await {
            return Err(format!("Failed to send packet length: {}", e));
        }

        // Отправляем само сообщение
        match writer.write_all(packet_str.as_bytes()).await {
            Ok(_) => {
                log("[Connection] Packet sent successfully");
                Ok(())
            }
            Err(e) => {
                log(&format!("[Connection] Failed to send packet: {}", e));
                Err(e.to_string())
            }
        }
    }

    async fn process_messages(
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
        reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
        writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        db: Arc<P2PDatabase>,
    ) {
        log("[Connection] Processing messages started");

        sleep(Duration::from_millis(100)).await;

        match Self::send_peer_info_request(&writer, &db).await {
            Ok(_) => log("[Connection] Peer info request sent successfully"),
            Err(e) => {
                log(&format!("[Connection] Failed to send peer info request: {}", e));
            }
        }

        log("[Connection] Starting message processing loop");
        while let Some(message) = rx.recv().await {
            log("[Connection] Received message from channel");
            match message {
                Message::SendData(packet) => {
                    log(&format!("[Connection] Processing SendData message: {:?}", packet));
                    if let Err(e) = Self::write_packet(&writer, &packet).await {
                        log(&format!("[Connection] Failed to send packet: {}", e));
                    } else {
                        log("[Connection] Packet sent successfully");
                    }
                }
                Message::GetResponse { tx } => {
                    log("[Connection] Processing GetResponse message");
                    let response = match Self::receive_message(&reader).await {
                        Ok(response) => response,
                        Err(e) => {
                            log(&format!("[Connection] Failed to receive message: {}", e));
                            continue;
                        }
                    };
                    if let Err(e) = tx.send(response) {
                        log(&format!("[Connection] Failed to send response to channel: {:?}", e));
                    } else {
                        log("[Connection] Response sent successfully");
                    }
                }
            }
        }
        log("[Connection] Message processing loop ended");
    }

    pub async fn send_peer_info_request(
        writer: &Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        db: &P2PDatabase,
    ) -> Result<(), String> {
        let fragments = db.get_storage_fragments().unwrap_or(Vec::new());
        let mut stored_files = Vec::new();
        for fragment in fragments {
            stored_files.push(fragment.file_hash.clone());
        }
        let connect_packet = TransportPacket {
            act: "info".to_string(),
            to: None,
            data: Some(
                TransportData::PeerInfo(PeerInfo {
                    is_signal_server: false,
                    total_space: db.get_total_space().unwrap_or(0),
                    free_space: db.get_storage_free_space().await.unwrap_or(0),
                    stored_files: stored_files,
                    public_key: db.get_or_create_peer_id().unwrap(),
                }),
            ),
            protocol: Protocol::STUN,
            peer_key: db.get_or_create_peer_id().unwrap(),
            uuid: generate_uuid(),
            nodes: vec![],
        };

        Self::write_packet(writer, &connect_packet).await
    }

    pub async fn receive_message(
        reader: &Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
    ) -> Result<TransportPacket, String> {
        let mut reader = reader.write().await;
        
        // Читаем длину сообщения (4 байта)
        let mut len_bytes = [0u8; 4];
        if let Err(e) = reader.read_exact(&mut len_bytes).await {
            if e.kind() == std::io::ErrorKind::ConnectionReset {
                log(&format!("[Connection] Connection reset by peer: {}", e));
                return Err("Connection reset by peer".to_string());
            }
            return Err(format!("Failed to read message length: {}", e));
        }
        let packet_len = u32::from_be_bytes(len_bytes) as usize;
        
        // Читаем само сообщение
        let mut packet_bytes = vec![0u8; packet_len];
        if let Err(e) = reader.read_exact(&mut packet_bytes).await {
            if e.kind() == std::io::ErrorKind::ConnectionReset {
                log(&format!("[Connection] Connection reset by peer: {}", e));
                return Err("Connection reset by peer".to_string());
            }
            return Err(format!("Failed to read message: {}", e));
        }
        
        let data = String::from_utf8_lossy(&packet_bytes);
        
        match serde_json::from_str(&data) {
            Ok(packet) => {
                Ok(packet)
            }
            Err(e) => {
                log(&format!("[Connection] Failed to parse JSON: {}", e));
                Err(format!("Failed to parse JSON: {}", e))
            }
        }
    }

    pub async fn send_peer_info_request_self(&self) -> Result<(), String> {
        Self::send_peer_info_request(&self.writer, &self.db).await
    }

    pub async fn send_packet(&self, packet: TransportPacket) -> Result<(), String> {
        Self::write_packet(&self.writer, &packet).await
    }

    pub async fn get_response(&self) -> Result<TransportPacket, String> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Message::GetResponse { tx }).await.unwrap();
        match rx.await {
            Ok(response) => Ok(response),
            Err(_) => Err("Failed to receive response from server".to_string()),
        }
    }
}
