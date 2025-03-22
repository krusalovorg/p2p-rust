use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, split}; // Добавляем split
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc}; // Добавляем mpsc

#[derive(Debug)]
pub struct InfoPeer {
    pub wait_connection: RwLock<bool>,
    pub public_addr: RwLock<String>,
    pub local_addr: String,
    pub uuid: RwLock<Option<String>>,
}

#[derive(Debug)]
pub struct Peer {
    reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>, // Добавляем reader
    writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>, // Добавляем writer
    pub info: InfoPeer,              // Peer information (ip:port)
    tx: mpsc::Sender<String>,        // Добавляем Sender для отправки сообщений
}

impl Peer {
    pub fn new(socket: TcpStream, info: Option<InfoPeer>) -> Arc<Self> {
        let mut info = info;

        if info.is_none() {
            info = Some(InfoPeer {
                wait_connection: RwLock::new(false),
                public_addr: RwLock::new("".to_string()),
                local_addr: socket.peer_addr().unwrap().to_string(),
                uuid: RwLock::new(None),
            });
        }

        let (reader, writer) = split(socket);
        let reader = Arc::new(RwLock::new(reader));
        let writer = Arc::new(RwLock::new(writer));
        let (tx, _) = mpsc::channel(100); // Оставляем канал для совместимости с существующим кодом

        Arc::new(Self {
            reader,
            writer,
            info: info.unwrap(),
            tx,
        })
    }

    pub async fn send_data(&self, message: &str) {
        let message_len = message.len() as u32;
        let len_bytes = message_len.to_be_bytes();
        
        let mut writer = self.writer.write().await;
        
        // Отправляем длину сообщения (4 байта)
        if let Err(e) = writer.write_all(&len_bytes).await {
            println!("Failed to send message length to peer {}: {}", self.info.local_addr, e);
            return;
        }
        
        // Отправляем само сообщение
        if let Err(e) = writer.write_all(message.as_bytes()).await {
            println!("Failed to send message to peer {}: {}", self.info.local_addr, e);
        } else {
            println!("[SendData] Message sent to peer {}: {}", self.info.local_addr, message);
        }
    }

    pub async fn receive_message(&self) -> Result<String, String> {
        let mut reader = self.reader.write().await;
        
        // Читаем длину сообщения (4 байта)
        let mut len_bytes = [0u8; 4];
        match reader.read_exact(&mut len_bytes).await {
            Ok(_) => {},
            Err(e) => {
                if e.kind() == std::io::ErrorKind::ConnectionReset {
                    println!("Peer {} disconnected", self.info.local_addr);
                    return Err("Peer disconnected".to_string());
                }
                println!("Error reading message length from peer {}: {}", self.info.local_addr, e);
                return Err(e.to_string());
            }
        }
        
        let message_len = u32::from_be_bytes(len_bytes) as usize;
        
        // Читаем само сообщение
        let mut message_bytes = vec![0u8; message_len];
        match reader.read_exact(&mut message_bytes).await {
            Ok(_) => {},
            Err(e) => {
                println!("Error reading message from peer {}: {}", self.info.local_addr, e);
                return Err(e.to_string());
            }
        }

        match String::from_utf8(message_bytes) {
            Ok(message) => Ok(message),
            Err(e) => {
                println!("Error converting message to string from peer {}: {}", self.info.local_addr, e);
                Err(e.to_string())
            }
        }
    }

    pub async fn set_wait_connection(&self, wait_connection_new: bool) {
        let mut wait_connection = self.info.wait_connection.write().await;
        *wait_connection = wait_connection_new;
    }

    pub async fn set_public_addr(&self, public_addr: String) {
        let mut public_addr_now = self.info.public_addr.write().await;
        *public_addr_now = public_addr;
    }

    pub async fn set_uuid(&self, uuid: String) {
        let mut current_uuid = self.info.uuid.write().await;
        *current_uuid = Some(uuid);
    }

    pub async fn send(&self, packet: String) -> Result<(), String> {
        self.send_data(&packet).await;
        Ok(())
    }

    pub async fn receive(&self) -> Result<String, String> {
        self.receive_message().await
    }
}
