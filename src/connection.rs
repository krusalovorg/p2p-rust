use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
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
}

impl Connection {
    pub async fn new(
        signal_server_ip: String,
        signal_server_port: i64,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) -> Connection {
        let (tx, mut rx) = mpsc::channel(16);

        let tx_clone = tx.clone();

        task::spawn(Self::process_messages(
            tx_clone,
            rx,
            signal_server_ip,
            signal_server_port,
            tunnel_public_ip,
            tunnel_public_port,
        ));

        Connection { tx }
    }

    async fn process_messages(
        tx: mpsc::Sender<Message>,
        mut rx: mpsc::Receiver<Message>,
        signal_server_ip: String,
        signal_server_port: i64,
        tunnel_public_ip: String,
        tunnel_public_port: u16,
    ) {
        println!("[Connection] Connecting to signal server {}:{}", signal_server_ip, signal_server_port);

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

        while let Some(message) = rx.recv().await {
            match message {
                Message::SendData(packet) => {
                    println!("[Connection] Sending packet: {:?}", packet);
                    if let Err(e) = signal_server.send_packet(packet).await {
                        println!("[Connection] Failed to send packet: {}", e);
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
        let result = self.tx.send(Message::SendData(packet)).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
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
