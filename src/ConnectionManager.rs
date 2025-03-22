use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use crate::tunnel::Tunnel;
use crate::connection::Connection;

enum ConnectionType {
    Signal(String),
    Stun,
}

pub struct ConnectionManager {
    connections: Arc<Mutex<HashMap<String, Connection>>>, // signal + turn
    tunnels: Arc<Mutex<HashMap<String, Tunnel>>>,         // stun
    pub incoming_packet_rx: Arc<Mutex<mpsc::Receiver<(ConnectionType, Vec<u8>)>>>,
    incoming_packet_tx: mpsc::Sender<(ConnectionType, Vec<u8>)>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let (incoming_packet_tx, incoming_packet_rx) = mpsc::channel(100);
        ConnectionManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            incoming_packet_rx: Arc::new(Mutex::new(incoming_packet_rx)),
            incoming_packet_tx,
        }
    }
    pub async fn send_signaling_message(&self, server_address: &str, data: Vec<u8>) -> Result<(), String> {
        let connections = self.connections.lock().await;
        if let Some(conn) = connections.get(server_address) {
            if let Err(e) = conn.tx.send(data).await {
                return Err(format!("Failed to send message to {}: {}", server_address, e));
            }
            Ok(())
        } else {
            Err(format!("Signaling connection to {} not found", server_address))
        }
    }

    pub async fn add_connection(&self, id: String, connection: Connection) {
        let mut connections = self.connections.lock().await;
        connections.insert(id, connection);
    }

    pub async fn add_tunnel(&self, id: String, tunnel: Tunnel) {
        let mut tunnels = self.tunnels.lock().await;
        tunnels.insert(id, tunnel);
    }

    pub async fn handle_incoming_packets(&self) {
        let incoming_packet_rx = self.incoming_packet_rx.clone();
        let mut rx = incoming_packet_rx.lock().await;

        loop {
            tokio::select! {
                Some((connection_type, data)) = rx.recv() => {
                    match connection_type {
                        ConnectionType::Signal(addr) => {
                            println!("Received signaling packet from {}: {:?}", addr, data);
                            if let Err(e) = self.send_signaling_message(&addr, format!("Processed: {:?}", data).into_bytes()).await {
                                eprintln!("Error sending response to {}: {}", addr, e);
                            }
                        }
                        ConnectionType::Stun => {
                            println!("Received STUN related data: {:?}", data);
                            let server_address = format!("{}:{}", "your_signal_server_ip", "your_signal_server_port");
                            if let Err(e) = self.send_signaling_message(&server_address, data).await {
                                eprintln!("Error sending STUN data to signaling server: {}", e);
                            }
                        }
                    }
                },
                Some((id, message)) = self.receive_connection_message() => {
                    println!("Received message from Connection {}: {:?}", id, message);
                    // Обработка сообщения от Connection
                },
                Some((id, message)) = self.receive_tunnel_message() => {
                    println!("Received message from Tunnel {}: {:?}", id, message);
                    // Обработка сообщения от Tunnel
                }
            }
        }
    }

    pub async fn receive_connection_message(&self) -> Option<(String, Vec<u8>)> {
        let connections = self.connections.lock().await;
        for (id, connection) in connections.iter() {
            if let Ok(Some(message)) = connection.rx.try_recv() {
                return Some((id.clone(), message));
            }
        }
        None
    }

    pub async fn receive_tunnel_message(&self) -> Option<(String, Vec<u8>)> {
        let tunnels = self.tunnels.lock().await;
        for (id, tunnel) in tunnels.iter() {
            if let Ok(Some(message)) = tunnel.rx.try_recv() {
                return Some((id.clone(), message));
            }
        }
        None
    }
}