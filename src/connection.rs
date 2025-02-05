use std::sync::Arc;
use std::time::Duration;

use tokio::io::{split, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task;
use tokio::time::{sleep, timeout};

use crate::signal::{Protocol, SignalClient, TransportPacket};
use crate::GLOBAL_DB;

#[derive(Debug)]
pub enum Message {
    SendData(TransportPacket),
    GetResponse {
        tx: oneshot::Sender<TransportPacket>,
    },
}

pub struct Connection {
    tx: mpsc::Sender<Message>,
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
                serde_json::json!({ "peer_uuid": &GLOBAL_DB.get_or_create_peer_id().unwrap() }),
            ), // Добавляем UUID в пакет
            session_key: None,
            status: None,
            protocol: Protocol::SIGNAL,
        };

        let connect_packet = serde_json::to_string(&connect_packet).unwrap();

        if let Err(e) = writer
            .write()
            .await
            .write_all(connect_packet.as_bytes())
            .await
        {
            println!("[Connection] Failed to send connect packet: {}", e);
        } else {
            println!("[Connection] Connect packet sent successfully");
        }

        task::spawn(Self::process_messages(
            tx.clone(),
            rx,
            reader.clone(),
            writer.clone(),
            tunnel_public_ip,
            tunnel_public_port,
        ));

        Connection { tx, writer, reader }
    }

    async fn process_messages(
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
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
                    let mut writer = writer.write().await;
                    let packet_bytes = serde_json::to_vec(&packet).unwrap();
                    println!("[Connection] Writing packet to socket: {:?}", packet_bytes);
                    if let Err(e) = writer.write_all(&packet_bytes).await {
                        println!("[Connection] Failed to send packet: {}", e);
                    } else {
                        println!("[Connection] Packet sent successfully");
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
            act: "wait_connection".to_string(),
            to: None,
            data: Some(
                serde_json::json!({ "peer_uuid": GLOBAL_DB.get_or_create_peer_id().unwrap() }),
            ), // Добавляем UUID в пакет
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };

        let connect_packet = serde_json::to_string(&connect_packet).unwrap();

        let result = writer
            .write()
            .await
            .write_all(connect_packet.as_bytes())
            .await;

        match result {
            Ok(_) => {
                println!("Sent peer info request: {}", connect_packet);
                Ok(())
            }
            Err(e) => {
                println!("Failed to send peer info request: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn receive_message(
        reader: &Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
    ) -> Result<TransportPacket, String> {
        let mut buf: [u8; 1024] = [0; 1024];
        println!("Reading from socket");
        let n = match reader.write().await.read(&mut buf).await {
            Ok(n) => {
                println!("[Connection] Received {} bytes", n);
                n
            }
            Err(e) => {
                println!("[Connection] Failed to read from socket: {}", e);
                return Err(e.to_string());
            }
        };

        if n == 0 {
            println!("[Connection] Received empty message");
            return Err("Received empty message".to_string());
        }

        let peer_info: String = String::from_utf8_lossy(&buf[..n]).to_string();
        let message: TransportPacket = match serde_json::from_str(&peer_info) {
            Ok(msg) => msg,
            Err(e) => {
                println!("[Connection] Failed to parse message: {}", e);
                return Err(e.to_string());
            }
        };
        println!("[Connection] Received message: {:?}", message);
        Ok(message)
    }

    pub async fn send_packet(&self, packet: TransportPacket) -> Result<(), String> {
        let packet_bytes = serde_json::to_vec(&packet).unwrap();
        let mut writer = self.writer.write().await;
        println!("[Connection] Writing packet to socket: {:?}", packet);
        match writer.write_all(&packet_bytes).await {
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

    pub async fn get_response(&self) -> Result<TransportPacket, String> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Message::GetResponse { tx }).await.unwrap();
        match rx.await {
            Ok(response) => Ok(response),
            Err(_) => Err("Failed to receive response from server".to_string()),
        }
    }
}
