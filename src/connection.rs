use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, split};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task;
use tokio::time::sleep;

use crate::signal::{SignalClient, TransportPacket};

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
}

impl Connection {
    pub async fn new(
        signal_server_ip: String,
        signal_server_port: i64,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) -> Connection {
        let (tx, rx) = mpsc::channel(16);

        let stream = TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port)).await.unwrap();
        let (reader, writer) = split(stream);

        let reader = Arc::new(RwLock::new(reader));
        let writer = Arc::new(RwLock::new(writer));

        task::spawn(Self::process_messages(
            tx.clone(),
            rx,
            reader,
            writer.clone(),
            signal_server_ip,
            signal_server_port,
            tunnel_public_ip,
            tunnel_public_port,
        ));

        Connection { tx, writer }
    }

    async fn process_messages(
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
        reader: Arc<RwLock<tokio::io::ReadHalf<TcpStream>>>,
        writer: Arc<RwLock<tokio::io::WriteHalf<TcpStream>>>,
        signal_server_ip: String,
        signal_server_port: i64,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) {
        println!("[Connection] Connecting to signal server");

        let mut signal_server = SignalClient::new();
        if let Err(e) = signal_server
            .connect(
                &signal_server_ip,
                signal_server_port,
                &tunnel_public_ip,
                tunnel_public_port,
            )
            .await
        {
            println!("[Connection] Failed to connect to signal server: {}", e);
            return;
        }

        sleep(Duration::from_millis(100)).await;

        match signal_server
            .send_peer_info_request(&tunnel_public_ip, tunnel_public_port)
            .await
        {
            Ok(_) => (),
            Err(e) => {
                println!("[Connection] Failed to send peer info request: {}", e);
            }
        }

        let reader_clone = Arc::clone(&reader);
        task::spawn(async move {
            let mut reader = reader_clone.write().await;
            let mut buf = vec![0; 1024];
            loop {
                match reader.read(&mut buf).await {
                    Ok(n) if n == 0 => break,
                    Ok(n) => {
                        let response: TransportPacket = serde_json::from_slice(&buf[..n]).unwrap();
                        if let Err(e) = tx.send(Message::GetResponse { tx: oneshot::channel().0 }).await {
                            println!("[Connection] Failed to send response to channel: {:?}", e);
                        }
                    }
                    Err(e) => {
                        println!("[Connection] Failed to read from socket: {:?}", e);
                        break;
                    }
                }
            }
        });

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
                    let response = match signal_server.receive_message().await {
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
