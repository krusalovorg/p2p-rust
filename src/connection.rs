use std::sync::Arc;
use std::time::Duration;

use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task;
use tokio::time::sleep;

use crate::packets::{Protocol, TransportPacket};
use crate::GLOBAL_DB;

#[derive(Debug)]
pub enum Message {
    SendData(TransportPacket),
    GetResponse {
        tx: oneshot::Sender<TransportPacket>,
    },
}

pub struct Connection {
    pub tx: mpsc::Sender<Message>,
    pub rx: mpsc::Receiver<Vec<u8>>, // Новый канал для передачи сообщений
    writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
    reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
}

impl Connection {
    pub async fn new(
        signal_server_ip: String,
        signal_server_port: i64,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) -> Connection {
        let (tx, rx) = mpsc::channel(16);
        let (message_tx, message_rx) = mpsc::channel(16); // Новый канал

        let stream = TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port))
            .await
            .unwrap();
        let (reader, writer) = split(stream);

        let reader = Arc::new(RwLock::new(reader));
        let writer = Arc::new(RwLock::new(writer));

        // Отправляем пакет при создании соединения
        let connect_packet = TransportPacket {
            public_addr: format!("{}:{}", tunnel_public_ip, tunnel_public_port),
            act: "info".to_string(),
            to: None,
            data: Some(
                serde_json::json!({ "peer_id": &GLOBAL_DB.get_or_create_peer_id().unwrap() }),
            ),
            status: None,
            protocol: Protocol::SIGNAL,
        };

        if let Err(e) = Self::write_packet(&writer, &connect_packet).await {
            println!("[Connection] Failed to send connect packet: {}", e);
        } else {
            println!("[Connection] Connect packet sent successfully");
        }

        task::spawn(Self::process_messages(
            tx.clone(),
            rx,
            message_tx, // Передаем новый канал
            reader.clone(),
            writer.clone(),
            tunnel_public_ip,
            tunnel_public_port,
        ));

        Connection { tx, rx: message_rx, writer, reader }
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
        println!("[Connection] Writing packet to socket: {:?}", packet);
        match writer.write_all(packet_str.as_bytes()).await {
            Ok(_) => {
                println!("[Connection] Packet sent successfully");
                Ok(())
            }
            Err(e) => {
                println!("[Connection] Failed to send packet: {}", e);
                Err(e.to_string())
            }
        }
    }

    async fn process_messages(
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
        message_tx: mpsc::Sender<Vec<u8>>, // Новый канал
        reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
        writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) {
        println!("[Connection] Processing messages");

        sleep(Duration::from_millis(100)).await;

        match Self::send_peer_info_request(&writer, &tunnel_public_ip, tunnel_public_port).await {
            Ok(_) => (),
            Err(e) => {
                println!("[Connection] Failed to send peer info request: {}", e);
            }
        }

        while let Some(message) = rx.recv().await {
            match message {
                Message::SendData(packet) => {
                    println!("[Connection] Received SendData message: {:?}", packet);
                    if let Err(e) = Self::write_packet(&writer, &packet).await {
                        println!("[Connection] Failed to send packet: {}", e);
                    }
                }
                Message::GetResponse { tx } => {
                    let response = match Self::receive_message(&reader).await {
                        Ok(response) => response,
                        Err(e) => {
                            println!("[Connection] Failed to receive message: {}", e);
                            continue;
                        }
                    };
                    let _ = message_tx.send(response.data.unwrap_or_default().into_bytes()).await; // Отправляем сообщение
                    if let Err(e) = tx.send(response) {
                        println!("[Connection] Failed to send response to channel: {:?}", e);
                    }
                }
            }
        }
    }

    pub async fn send_peer_info_request(
        writer: &Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        public_ip: &str,
        public_port: u16,
    ) -> Result<(), String> {
        let connect_packet = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "info".to_string(),//wait_connection
            to: None,
            data: Some(
                serde_json::json!({ "peer_id": GLOBAL_DB.get_or_create_peer_id().unwrap() }),
            ),
            status: None,
            protocol: Protocol::STUN,
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
                println!("[Connection] Connection reset by peer: {}", e);
                return Err("Connection reset by peer".to_string());
            }
            return Err(format!("Failed to read message length: {}", e));
        }
        let packet_len = u32::from_be_bytes(len_bytes) as usize;
        
        // Читаем само сообщение
        let mut packet_bytes = vec![0u8; packet_len];
        if let Err(e) = reader.read_exact(&mut packet_bytes).await {
            if e.kind() == std::io::ErrorKind::ConnectionReset {
                println!("[Connection] Connection reset by peer: {}", e);
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
                println!("[Connection] Failed to parse JSON: {}", e);
                Err(format!("Failed to parse JSON: {}", e))
            }
        }
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
