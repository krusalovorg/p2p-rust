use crate::connection::{Connection, Message};
use crate::db::P2PDatabase;
use crate::http::http_proxy::HttpProxy;
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::{TransportData, TransportPacket};
use crate::peer::peer_api::PeerAPI;
use crate::tunnel::Tunnel;
use crate::ui::console_manager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

use super::types::PeerOpenNetInfo;

#[derive(Clone)]
pub struct ConnectionManager {
    pub connections: Arc<Mutex<HashMap<String, Connection>>>,
    pub tunnels: Arc<Mutex<HashMap<String, Arc<Mutex<Tunnel>>>>>,
    pub connections_stun: Arc<Mutex<HashMap<String, PeerOpenNetInfo>>>,

    pub incoming_packet_rx:
        Arc<Mutex<mpsc::Receiver<(ConnectionType, TransportPacket, Option<Arc<Connection>>)>>>,
    pub incoming_packet_tx:
        mpsc::Sender<(ConnectionType, TransportPacket, Option<Arc<Connection>>)>,

    pub connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>>,
    pub db: Arc<P2PDatabase>,
    pub http_proxy: Arc<HttpProxy>,
    pub proxy_http_tx: mpsc::Sender<TransportPacket>,

    pub proxy_http_tx_reciever: Arc<Mutex<mpsc::Sender<TransportPacket>>>,
}

impl ConnectionManager {
    pub async fn new(db: &P2PDatabase) -> Self {
        let (incoming_packet_tx, incoming_packet_rx) = mpsc::channel(100);
        let (proxy_http_tx, mut proxy_http_rx) = mpsc::channel(1000);
        let (proxy_http_tx_reciever, mut proxy_http_rx_reciever) = mpsc::channel(1000);

        let connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let connections_stun = Arc::new(Mutex::new(HashMap::<String, PeerOpenNetInfo>::new()));

        let db_arc = Arc::new(db.clone());
        let proxy = Arc::new(HttpProxy::new(db_arc.clone(), proxy_http_tx.clone()));

        let proxy_clone = Arc::clone(&proxy);
        let proxy_clone_for_spawn = proxy_clone.clone();
        
        let mut proxy_http_rx_reciever = proxy_http_rx_reciever;

        tokio::spawn(async move {
            proxy_clone_for_spawn.start().await;
        });

        let manager = ConnectionManager {
            connections: Arc::new(Mutex::new(HashMap::new())),
            tunnels: Arc::new(Mutex::new(HashMap::new())),
            connections_stun,

            incoming_packet_rx: Arc::new(Mutex::new(incoming_packet_rx)),
            incoming_packet_tx,

            connections_turn,

            db: db_arc,
            http_proxy: proxy,
            proxy_http_tx,

            proxy_http_tx_reciever: Arc::new(Mutex::new(proxy_http_tx_reciever)),
        };

        let manager_clone = manager.clone();
        let manager_clone_for_http = manager_clone.clone();
        tokio::spawn(async move {
            while let Some(packet) = proxy_http_rx.recv().await {
                println!(
                    "[HTTP Proxy] Getted request from http proxy: {:?}",
                    packet.to
                );
                manager_clone_for_http.auto_send_packet(packet).await;
            }
        });

        let db_clone = manager_clone.db.clone();
        let proxy_clone_rx = proxy_clone.clone();
        tokio::spawn(async move {
            while let Some(packet) = proxy_http_rx_reciever.recv().await {
                println!("[HTTP Proxy] Getted request from http proxy: {:?}", packet.to);
                let db = db_clone.clone();
                let proxy = proxy_clone_rx.clone();
                tokio::spawn(async move {
                    if let Some(TransportData::ProxyMessage(msg)) = packet.data {
                        println!("[HTTP Proxy] Getted request from http proxy: {:?}", msg.from_peer_id);
                        let encrypted_response = base64::decode(&msg.text).unwrap();
                        let nonce = base64::decode(&msg.nonce).unwrap();
                        let nonce_array: [u8; 12] = nonce.try_into().unwrap();
                        let response_bytes = db
                            .decrypt_message(&encrypted_response, nonce_array, &msg.from_peer_id)
                            .unwrap();
                        proxy.set_response(msg.request_id.clone(), response_bytes).await;
                    }
                });
            }
        });

        manager
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

    pub async fn auto_send_packet(&self, packet: TransportPacket) {
        let connections = self.connections.lock().await;
        let mut sended_by_uuid = false;
        println!("Auto send packet: {:?}", packet);
        for (id, connection) in connections.iter() {
            if let Some(to) = &packet.to {
                if id == to {
                    if let Err(e) = connection.send_packet(packet.clone()).await {
                        println!("[ERROR] Failed to send packet to connection {}: {}", id, e);
                    } else {
                        println!(
                            "[HTTP Proxy] [AUTO SEND] Sended packet to connection {}: {:?}",
                            id, packet.to
                        );
                        sended_by_uuid = true;
                    }
                }
            }
        }
        if !sended_by_uuid {
            for (id, connection) in connections.iter() {
                if let Err(e) = connection.send_packet(packet.clone()).await {
                    println!("[ERROR] Failed to send packet to connection {}: {}", id, e);
                } else {
                    println!(
                        "[HTTP Proxy] [BROADCAST] Sended packet to connection {}: {:?}",
                        id, packet.to
                    );
                    sended_by_uuid = true;
                }
            }
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

        let api = PeerAPI::new(connection.clone(), &self.db, &self);
        let api_clone = api.clone();
        let db_clone = self.db.clone();

        tokio::spawn({
            async move {
                loop {
                    console_manager(
                        Arc::new(api_clone.clone()),
                        connections_turn_clone.clone(),
                        &db_clone,
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

    pub async fn get_tunnel(&self, id: String) -> Option<Arc<Mutex<Tunnel>>> {
        let tunnels = self.tunnels.lock().await;
        tunnels.get(&id).cloned()
    }

    pub async fn have_connection_with_peer(&self, id: String) -> bool {
        let connections_turn = self.connections_turn.read().await;
        connections_turn.get(&id).is_some() && connections_turn.get(&id).unwrap().connected
    }

    pub async fn add_tunnel(&self, id: String, tunnel: Tunnel) {
        let tx = self.incoming_packet_tx.clone();
        let mut tunnels = self.tunnels.lock().await;

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
