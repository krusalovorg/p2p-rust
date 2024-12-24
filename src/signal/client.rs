use crate::config::Config;
use crate::signal::{Protocol, TransportPacket};
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};

#[derive(Debug)]
struct Peer {
    socket: Arc<RwLock<TcpStream>>,
    info: String,
}

type PeersSender = mpsc::Sender<Peer>;
type PeersReceiver = mpsc::Receiver<Peer>;

pub struct SignalClient {
    peers_sender: PeersSender,
    peers_receiver: Arc<Mutex<PeersReceiver>>,
    socket: Option<Arc<RwLock<TcpStream>>>,
    port: i64,
}

impl SignalClient {
    pub fn new() -> Self {
        let (peers_sender, peers_receiver) = mpsc::channel(100);
        let config: Config = Config::from_file("config.toml");
        let signal_server_port = config.signal_server_port;

        SignalClient {
            peers_sender,
            peers_receiver: Arc::new(Mutex::new(peers_receiver)),
            socket: None,
            port: signal_server_port,
        }
    }

    pub async fn connect(
        &mut self,
        signal_server_ip: &str,
        signal_server_port: i64,
        public_ip: &str,
        public_port: u16,
    ) -> Result<(), String> {
        println!(
            "[signal] Connecting to signal server {}:{}",
            signal_server_ip, signal_server_port
        );
        match TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port)).await {
            Ok(socket) => {
                let socket = Arc::new(RwLock::new(socket));
                self.socket = Some(socket);
                let connect_packet = TransportPacket {
                    public_addr: format!("{}:{}", public_ip, public_port),
                    act: "info".to_string(),
                    to: None,
                    data: None,
                    session_key: None,
                    status: None,
                    protocol: Protocol::SIGNAL,
                };
                let connect_packet = serde_json::to_string(&connect_packet).unwrap();
                if let Some(socket) = &self.socket {
                    socket
                        .write()
                        .await
                        .write_all(connect_packet.as_bytes())
                        .await
                        .map_err(|e| {
                            println!("Failed to send connect packet: {}", e);
                            e.to_string()
                        })?;
                }
                Ok(())
            }
            Err(e) => {
                println!("Failed to connect to signal server: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn send_packet(&self, packet: TransportPacket) -> Result<(), String> {
        println!("[send_packet] Packet: {:?}", packet);
        let string_packet = serde_json::to_string(&packet).unwrap();

        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }

        if let Some(socket) = &self.socket {
            println!("Sending turn data to signal server: {}", string_packet);

            let result = socket
                .clone()
                .write_owned()
                .await
                .write_all(string_packet.as_bytes())
                .await;
            println!("Sended");

            match result {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        return Err("Socket error".to_string());
    }

    // для пира
    pub async fn send_peer_info_request(
        &self,
        public_ip: &str,
        public_port: u16,
    ) -> Result<(), String> {
        let connect_packet = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "wait_connection".to_string(),
            to: None,
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };
        let connect_packet = serde_json::to_string(&connect_packet).unwrap();

        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }

        if let Some(socket) = &self.socket {
            let result = socket
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
        } else {
            Err("Socket is not connected".to_string())
        }
    }

    //Со стороны юзера получение сообщений от сигнального сервера
    pub async fn receive_message(&self) -> Result<TransportPacket, String> {
        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }
        if let Some(socket) = &self.socket {
            let mut buf: [u8; 1024] = [0; 1024];
            let n = socket
                .write()
                .await
                .read(&mut buf)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("Received empty message".to_string());
            }
            let peer_info: String = String::from_utf8_lossy(&buf[..n]).to_string();
            let message: TransportPacket =
                serde_json::from_str(&peer_info).map_err(|e| e.to_string())?;

            Ok(message)
        } else {
            Err("Socket is not connected".to_string())
        }
    }

    pub async fn extract_addr(public_addr: String) -> Result<(String, u16), String> {
        let parts: Vec<&str> = public_addr.split(':').collect();

        if parts.len() != 2 {
            return Err(format!("Invalid peer info received: {}", public_addr));
        }

        let ip = parts[0].to_string();
        let port: u16 = parts[1]
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;

        Ok((ip, port))
    }
}
