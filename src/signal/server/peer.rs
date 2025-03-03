use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, split}; // Добавляем split
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc}; // Добавляем mpsc

#[derive(Debug)]
pub struct InfoPeer {
    pub wait_connection: RwLock<bool>,
    pub public_addr: RwLock<String>,
    pub local_addr: String,
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
            });
        }

        let (reader, writer) = split(socket); // Разделяем поток
        let reader = Arc::new(RwLock::new(reader));
        let writer = Arc::new(RwLock::new(writer));
        let (tx, mut rx) = mpsc::channel(100); // Создаем канал

        let peer = Arc::new(Self {
            reader: reader.clone(),
            writer: writer.clone(),
            info: info.unwrap(),
            tx,
        });

        // Запускаем задачу для обработки отправки сообщений
        let peer_clone = peer.clone();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if let Err(e) = peer_clone.writer.write().await.write_all(message.as_bytes()).await {
                    println!("Failed to send message to peer {}: {}", peer_clone.info.local_addr, e);
                } else {
                    println!("[SendData] Message sent to peer {}: {}", peer_clone.info.local_addr, message);
                }
            }
        });

        peer
    }

    pub async fn send_data(&self, message: &str) {
        if let Err(e) = self.tx.send(message.to_string()).await {
            println!("Failed to send message to peer {}: {}", self.info.local_addr, e);
        }
    }

    pub async fn receive_message(&self) -> Result<String, String> {
        let mut buf = [0; 1024];
        let n = match self.reader.write().await.read(&mut buf).await {
            Ok(0) => {
                println!("Peer {} disconnected", self.info.local_addr);
                return Err("Peer disconnected".to_string());
            }
            Ok(n) => n,
            Err(e) => {
                println!("Error reading from peer {}: {}", self.info.local_addr, e);
                return Err(e.to_string());
            }
        };

        let message = String::from_utf8_lossy(&buf[..n]).to_string();

        return Ok(message);
    }

    pub async fn set_wait_connection(&self, wait_connection_new: bool) {
        let mut wait_connection = self.info.wait_connection.write().await;
        *wait_connection = wait_connection_new;
    }

    pub async fn set_public_addr(&self, public_addr: String) {
        let mut public_addr_now = self.info.public_addr.write().await;
        *public_addr_now = public_addr;
    }

    pub async fn send(&self, packet: String) -> Result<(), String> {
        self.send_data(&packet).await;
        Ok(())
    }

    pub async fn receive(&self) -> Result<String, String> {
        self.receive_message().await
    }
}
