use crate::connection::{Connection, Message};
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::TransportPacket;
use crate::peer::peer_api::PeerAPI;
use crate::tunnel::Tunnel;
use crate::ui::console_manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Connection>>>,
    pub tunnels: Arc<Mutex<HashMap<String, Arc<Mutex<Tunnel>>>>>,
    pub incoming_packet_rx:
        Arc<Mutex<mpsc::Receiver<(ConnectionType, TransportPacket, Option<Arc<Connection>>)>>>,
    pub incoming_packet_tx:
        mpsc::Sender<(ConnectionType, TransportPacket, Option<Arc<Connection>>)>,

    pub connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>>,
    pub base_tunnel: Arc<Mutex<Tunnel>>,
    pub my_public_addr: Arc<String>,
}

impl ConnectionManager {
    pub async fn new() -> Self {
        let (incoming_packet_tx, incoming_packet_rx) = mpsc::channel(100);

        let connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let tunnel = Arc::new(tokio::sync::Mutex::new(Tunnel::new().await));

        let my_ip = tunnel.lock().await.get_public_ip();
        let my_port = tunnel.lock().await.get_public_port();
        let my_public_addr = Arc::new(format!("{}:{}", my_ip, my_port));

        ConnectionManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            incoming_packet_rx: Arc::new(Mutex::new(incoming_packet_rx)),
            incoming_packet_tx,
            connections_turn,
            base_tunnel: tunnel,
            my_public_addr,
        }
    }
    pub async fn send_signaling_message(
        &self,
        server_address: &str,
        data: TransportPacket,
    ) -> Result<(), String> {
        let connections = self.connections.lock().await;
        if let Some(conn) = connections.get(server_address) {
            if let Err(e) = conn.tx.send(Message::SendData(data)).await {
                return Err(format!(
                    "Failed to send message to {}: {}",
                    server_address, e
                ));
            }
            Ok(())
        } else {
            Err(format!(
                "Signaling connection to {} not found",
                server_address
            ))
        }
    }

    pub async fn add_connection(&self, id: String, connection: Arc<Connection>) {
        let tx = self.incoming_packet_tx.clone();
        let mut connections = self.connections.lock().await;

        self.connections_turn.write().await.insert(
            id.clone(),
            ConnectionTurnStatus {
                connected: true,
                turn_connection: true,
            },
        );

        let id_clone = id.clone();
        let connections_turn_clone = self.connections_turn.clone();

        let connection_clone_for_console = connection.clone();
        let connection_clone_for_response = connection.clone();
        
        let my_public_addr_clone = self.my_public_addr.clone();
        let my_public_addr_clone_for_api = my_public_addr_clone.clone();

        let api = PeerAPI::new(connection.clone(), my_public_addr_clone_for_api);
        let api_clone = api.clone();

        tokio::spawn({
            async move {
                loop {
                    console_manager(
                        Arc::new(api_clone.clone()),
                        connections_turn_clone.clone(),
                    )
                    .await;
                }
            }
        });

        tokio::spawn({
            let tx_clone = tx.clone();
            let connection_clone = connection.clone();
            async move {
                while let Ok(response) = connection_clone.get_response().await {
                    let _ = tx_clone
                        .send((
                            ConnectionType::Signal(id_clone.clone()),
                            response,
                            Some(connection_clone.clone()),
                        ))
                        .await;
                }
            }
        });

        connections.insert(
            id,
            Arc::try_unwrap(connection).unwrap_or_else(|arc| (*arc).clone()),
        );
    }

    pub async fn add_tunnel(&self, id: String, tunnel: Tunnel) {
        let tx = self.incoming_packet_tx.clone();
        let mut tunnels = self.tunnels.lock().await;

        let id_clone = id.clone();
        let tunnel_clone = Arc::new(tokio::sync::Mutex::new(tunnel));
        let tunnel_clone_for_spawn = tunnel_clone.clone();
        tokio::spawn(async move {
            let (local_tx, mut local_rx) = mpsc::channel::<Vec<u8>>(16);

            // Запуск обработки входящих сообщений из туннеля
            tokio::spawn(async move {
                while let Some(data) = local_rx.recv().await {
                    // Преобразование Vec<u8> в TransportPacket
                    if let Ok(packet) = serde_json::from_slice(&data)
                        .map_err(|e| format!("Failed to parse TransportPacket: {}", e))
                    {
                        let _ = tx.send((ConnectionType::Stun, packet, None)).await;
                    } else {
                        println!("[ERROR] Failed to parse incoming data into TransportPacket");
                    }
                }
            });

            loop {
                let mut buf = vec![0u8; 1024];
                let mut tunnel = tunnel_clone_for_spawn.lock().await;
                if let Some(socket) = &tunnel.socket {
                    while let Ok((n, reply_addr)) = socket.recv_from(&mut buf).await {
                        let data = buf[..n].to_vec();
                        let _ = local_tx.send(data).await;
                    }
                }
            }
        });

        tunnels.insert(id, tunnel_clone);
    }
}
