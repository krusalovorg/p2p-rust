use crate::config::Config;
use crate::packets::PeerInfo;
use crate::signal::{Protocol, TransportPacket};
use crate::GLOBAL_DB;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncRead, AsyncWrite, split};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex, RwLock, RwLockWriteGuard};

#[derive(Debug)]
struct Peer {
    socket: Arc<RwLock<TcpStream>>,
    info: String,
}

type PeersSender = mpsc::Sender<Peer>;
type PeersReceiver = mpsc::Receiver<Peer>;

pub struct SignalClient {
    writer: Option<Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>>,
    reader: Option<Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>>,
}

impl SignalClient {
    pub fn new() -> Self {
        // let config: Config = Config::from_file("config.toml");
        // let signal_server_port = config.signal_server_port;

        SignalClient {
            writer: None,
            reader: None,
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
            "[SignalClient] Connecting to signal server {}:{}",
            signal_server_ip, signal_server_port
        );

        match TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port)).await {
            Ok(socket) => {
                let (reader, writer) = split(socket);
                self.writer = Some(Arc::new(RwLock::new(writer)));
                self.reader = Some(Arc::new(RwLock::new(reader)));

                let peer_info = serde_json::to_value(PeerInfo {
                    peer_id: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                }).unwrap();

                let connect_packet = TransportPacket {
                    public_addr: format!("{}:{}", public_ip, public_port),
                    act: "info".to_string(),
                    to: None,
                    data: Some(peer_info),
                    status: None,
                    protocol: Protocol::SIGNAL,
                };

                let connect_packet = serde_json::to_string(&connect_packet).unwrap();

                if let Some(writer) = &self.writer {
                    writer
                        .write()
                        .await
                        .write_all(connect_packet.as_bytes())
                        .await
                        .map_err(|e| {
                            println!("[SignalClient] Failed to send connect packet: {}", e);
                            e.to_string()
                        })?;
                }

                Ok(())
            }
            Err(e) => {
                println!("[SignalClient] Failed to connect to signal server: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn send_packet(&self, packet: TransportPacket) -> Result<(), String> {
        let string_packet = serde_json::to_string(&packet).unwrap();

        if self.writer.is_none() {
            return Err("[SignalClient] Writer is not connected".to_string());
        } else {
            println!("[SignalClient] Writer is connected");
        }

        if let Some(writer) = &self.writer {
            println!("[SignalClient] Sending turn data to signal server: {}", string_packet);

            let result = writer
                .clone()
                .write_owned()
                .await
                .write_all(string_packet.as_bytes())
                .await;

            println!("[SignalClient] Packet sent: {}", string_packet);

            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("[SignalClient] Writer error".to_string())
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
