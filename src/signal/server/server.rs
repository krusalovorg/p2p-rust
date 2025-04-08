use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};

use super::Peer;
use crate::config::Config;
use crate::packets::{Protocol, SyncPeerInfo, SyncPeerInfoData, TransportPacket};
use crate::tunnel::Tunnel;
use crate::db::P2PDatabase;

#[derive(Debug)]
pub struct SignalServer {
    pub peers: RwLock<Vec<Arc<Peer>>>,
    port: i64,
    ip: String,
    message_tx: mpsc::Sender<(Arc<Peer>, String)>,
    my_public_addr: Arc<String>,
    db: Arc<P2PDatabase>,
}

impl SignalServer {
    pub async fn new(db: &P2PDatabase) -> Arc<Self> {
        let config: Config = Config::from_file("config.toml");
        let (message_tx, mut message_rx) = mpsc::channel(100);

        let tunnel = Tunnel::new().await;
        let public_ip = tunnel.get_public_ip();
        let my_public_addr = Arc::new(format!("{}:{}", public_ip, config.signal_server_port));

        let server = SignalServer {
            peers: RwLock::new(Vec::new()),
            port: config.signal_server_port,
            message_tx,
            ip: public_ip,
            my_public_addr,
            db: Arc::new(db.clone()),
        };

        let server_arc = Arc::new(server);

        let server_clone = Arc::clone(&server_arc);
        tokio::spawn(async move {
            while let Some((peer, message)) = message_rx.recv().await {
                server_clone
                    .handle_message(&server_clone, peer, message)
                    .await;
            }
        });

        server_arc
    }

    pub async fn run(self: Arc<Self>) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(addr.clone()).await.unwrap();
        println!("[SignalServer] Running on {}", addr);

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            println!(
                "[SignalServer] New connection: {}",
                socket.peer_addr().unwrap()
            );

            let peer = Peer::new(socket, None);

            self.add_peer(peer.clone(), false).await.unwrap();

            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                self_clone.handle_connection(peer.clone()).await;
            });
        }
    }

    async fn handle_connection(self: Arc<Self>, peer: Arc<Peer>) {
        loop {
            let message = match peer.receive().await {
                Ok(msg) => msg,
                Err(e) => {
                    println!(
                        "[SignalServer] Failed to receive message from peer {}: {}",
                        peer.info.local_addr, e
                    );
                    if e == "Peer disconnected" {
                        self.remove_peer(&peer).await;
                        break;
                    }
                    continue;
                }
            };
            println!("[SignalServer] Received message: {}", message);

            if let Err(e) = self.message_tx.send((peer.clone(), message)).await {
                println!("[SignalServer] Failed to send message to handler: {}", e);
            }
        }
    }

    async fn remove_peer(self: Arc<Self>, peer: &Arc<Peer>) -> bool {
        let mut peers = self.peers.write().await;
        let peer_index = peers
            .iter()
            .position(|p| p.info.local_addr == peer.info.local_addr);

        if let Some(index) = peer_index {
            peers.remove(index);
            true
        } else {
            false
        }
    }

    async fn sync_peers(self: Arc<Self>, peer: Arc<Peer>) {
        let peers_info: Vec<SyncPeerInfo> = {
            let peers_guard = self.peers.read().await;
            let mut peers_info = Vec::new();
            for p in peers_guard.iter() {
                let public_addr = p.info.public_addr.read().await.clone();
                let uuid = p
                    .info
                    .uuid
                    .read()
                    .await
                    .clone()
                    .unwrap_or_else(|| "Not set".to_string());
                peers_info.push(SyncPeerInfo {
                    public_addr: public_addr,
                    uuid: uuid,
                });
            }
            peers_info
        };

        let peer_public_addr = peer.info.public_addr.read().await.clone();

        let packet = TransportPacket {
            public_addr: self.my_public_addr.clone().to_string(),
            act: "peer_list".to_string(),
            to: Some(peer_public_addr.clone()),
            data: Some(json!(SyncPeerInfoData { peers: peers_info })),
            status: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        let packet = serde_json::to_string(&packet).unwrap();
        if let Err(e) = peer.send(packet).await {
            println!(
                "[SignalServer] Failed to send peer list to peer {}: {}",
                peer_public_addr, e
            );
        } else {
            println!(
                "[SignalServer] Successfully sent peer list to peer {}",
                peer_public_addr
            );
        }
    }

    async fn handle_message(&self, server: &Arc<SignalServer>, peer: Arc<Peer>, message: String) {
        println!(
            "[SignalServer] Handling message from peer {}: {}",
            peer.info.local_addr, message
        );
        let message: TransportPacket = match serde_json::from_str(&message) {
            Ok(msg) => msg,
            Err(e) => {
                println!(
                    "[SignalServer] Failed to parse message from peer {}: {}. Message: {}",
                    peer.info.local_addr, e, message
                );
                return;
            }
        };
        let peer_public_addr = &message.public_addr;

        let is_peer_wait_connection = message.act == "wait_connection";

        peer.set_wait_connection(is_peer_wait_connection).await;
        peer.set_public_addr(peer_public_addr.clone()).await;

        if let Some(data) = &message.data {
            if let Some(peer_id) = data.get("peer_id").and_then(|v| v.as_str()) {
                println!("[SignalServer] Setting peer UUID: {}", peer_id);
                peer.set_uuid(peer_id.to_string()).await;
            }
        }

        if message.act == "info" {
            println!("[SignalServer] =================");
            println!("[SignalServer] CONNECTED PEER INFO:");
            println!("[SignalServer] PUBLIC ADDRESS: {}", peer_public_addr);
            println!("[SignalServer] LOCAL ADDRESS: {}", peer.info.local_addr);
            if let Some(uuid) = peer.info.uuid.read().await.clone() {
                println!("[SignalServer] PEER UUID: {}", uuid);
            } else {
                println!("[SignalServer] PEER UUID: Not set");
            }
            println!("[SignalServer] =================");

            server.clone().sync_peers(peer.clone()).await;
        }

        match message.protocol {
            Protocol::STUN => {
                if is_peer_wait_connection {
                    println!(
                        "[SignalServer] Peer is ready to connect: {}",
                        peer_public_addr
                    );
                    if let Some(data) = message.data {
                        if let Some(target_peer_id) =
                            data.get("connect_peer_id").and_then(|v| v.as_str())
                        {
                            println!(
                                "[SignalServer] Looking for peer with UUID: {}",
                                target_peer_id
                            );
                            let peers_guard = server.peers.read().await;
                            for target_peer in peers_guard.iter() {
                                if let Some(uuid) = target_peer.info.uuid.read().await.clone() {
                                    if uuid == target_peer_id {
                                        println!(
                                            "[SignalServer] Found peer with matching UUID: {}",
                                            target_peer_id
                                        );
                                        let server_clone = Arc::clone(server);
                                        server_clone
                                            .connect_peers(peer.clone(), target_peer.clone())
                                            .await;
                                        return;
                                    }
                                }
                            }
                            println!("[SignalServer] Peer with UUID {} not found", target_peer_id);
                        }
                    }

                    // if server.peers.read().await.len() >= 1 {
                    //     let server_clone = Arc::clone(server);
                    //     server_clone.wait_for_peers().await;
                    //     println!("[SignalServer] End wait peers");
                    // }
                }
            }
            Protocol::TURN => {
                if let Some(to) = &message.to {
                    println!("[SignalServer] Received turn packet: {:?}", message);
                    let peers_guard = server.peers.read().await;
                    for item in peers_guard.iter() {
                        if Some(item.info.public_addr.read().await.to_string()) == Some(to.clone())
                            || *item.info.uuid.read().await == Some(to.clone())
                        {
                            println!(
                                "[SignalServer] Send turn packet: {} {:?}",
                                peer.info.local_addr, message
                            );

                            let turn_packet = TransportPacket {
                                public_addr: message.public_addr.to_string(),
                                act: message.act.to_string(),
                                to: message.to.clone(),
                                data: message.data.clone(),
                                status: message.status.clone(),
                                protocol: Protocol::TURN,
                                uuid: message.uuid.to_string(),
                            };
                            let turn_packet = serde_json::to_string(&turn_packet).unwrap();
                            if let Err(e) = item.send(turn_packet).await {
                                println!(
                                    "[SignalServer] Failed to send turn packet to peer {}: {}",
                                    item.info.local_addr, e
                                );
                            } else {
                                println!(
                                    "[SignalServer] Successfully send turn packet to peer {}",
                                    item.info.local_addr
                                );
                            }
                            break;
                        }
                    }
                }
            }
            Protocol::SIGNAL => {
                if message.act == "peer_list" {
                    server.clone().sync_peers(peer.clone()).await;
                } else if message.protocol == Protocol::STUN && message.act == "wait_connection" {
                    // Добавляем обработку массива peers
                    if let Some(data) = &message.data {
                        if let Some(peers) = data.get("peers").and_then(|v| v.as_array()) {
                            println!("[SignalServer] Processing peers for wait_connection:");
                            for peer in peers {
                                if let Some(public_addr) =
                                    peer.get("public_addr").and_then(|v| v.as_str())
                                {
                                    println!("  - Public Address: {}", public_addr);
                                }
                                if let Some(uuid) = peer.get("uuid").and_then(|v| v.as_str()) {
                                    println!("    UUID: {}", uuid);
                                }
                            }
                        } else {
                            println!(
                                "[SignalServer] No peers found in the data for wait_connection."
                            );
                        }
                    } else {
                        println!("[SignalServer] No data found in the packet for wait_connection.");
                    }
                }
            }
        }
    }

    async fn connect_peers(&self, first_peer: Arc<Peer>, second_peer: Arc<Peer>) {
        println!("[SignalServer] Connecting peers");

        let first_peer_public_addr = first_peer.info.public_addr.read().await.clone();
        let second_peer_public_addr = second_peer.info.public_addr.read().await.clone();

        {
            println!(
                "[SignalServer] Sending packet to: {}",
                second_peer.info.local_addr
            );
            SignalServer::send_peer_info(second_peer.clone(), first_peer.clone(), first_peer_public_addr).await;
            println!(
                "[SignalServer] Sent packet to peer: {}",
                second_peer.info.local_addr
            );
        }
        second_peer.set_wait_connection(false).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            println!(
                "[SignalServer] Sending packet to: {}",
                first_peer.info.local_addr
            );
            SignalServer::send_peer_info(first_peer.clone(), second_peer.clone(), second_peer_public_addr).await;
            println!(
                "[SignalServer] Sent packet to peer: {}",
                first_peer.info.local_addr
            );
        }
        first_peer.set_wait_connection(false).await;
    }

    async fn add_peer(
        &self,
        peer: Arc<Peer>,
        is_peer_wait_connection: bool,
    ) -> Result<Arc<Peer>, String> {
        let mut peers_guard = self.peers.write().await;
        let mut peer_added = false;
        let mut peer_res: Option<Arc<Peer>> = None;
        for item in peers_guard.iter() {
            if *item.info.local_addr == peer.info.local_addr {
                println!("Peer already in the list: {}", peer.info.local_addr);
                peer_added = true;
                if is_peer_wait_connection {
                    let mut wait_connection = item.info.wait_connection.write().await;
                    *wait_connection = true;
                }
                peer_res = Some(item.clone());
                break;
            }
        }
        if !peer_added {
            peer_res = Some(peer.clone());
            peers_guard.push(peer.clone());
        }
        if peer_res.is_none() {
            return Err("Failed to add peer to the list".to_string());
        }
        return Ok(peer_res.clone().unwrap());
    }

    async fn send_peer_info(to_peer: Arc<Peer>, about_peer: Arc<Peer>, public_addr: String) {
        let wait_packet = TransportPacket {
            public_addr: public_addr.clone(), //к кому будет пытаться подключиться пир
            act: "wait_connection".to_string(), //TODO:было wait_connection
            to: None,                         //кому отправляем данный пакет
            data: None,
            status: None,
            protocol: Protocol::STUN,
            uuid: about_peer.info.uuid.read().await.clone().unwrap(),
        };
        let wait_packet = serde_json::to_string(&wait_packet).unwrap();

        println!(
            "[SignalServer] Sending wait packet to peer: {}",
            to_peer.info.local_addr
        );
        let result = to_peer.send(wait_packet).await;

        match result {
            Ok(_) => println!(
                "[SignalServer] Successfully sent packet to peer: {}. Peer connect to: {}",
                to_peer.info.public_addr.read().await,
                public_addr
            ),
            Err(e) => println!("[SignalServer] Failed to send peer to peer info: {}", e),
        }
    }

    // async fn wait_for_peers(self: Arc<Self>) {
    //     println!("START WAITNG PEERS ");
    //     // loop {
    //     let peers_snapshot = {
    //         let peers_guard = self.peers.read().await;
    //         peers_guard.clone()
    //     };

    //     let mut waiting_peers: Vec<Arc<Peer>> = Vec::new();
    //     for peer in peers_snapshot.iter() {
    //         if *peer.info.wait_connection.read().await {
    //             waiting_peers.push(peer.clone());
    //         }
    //     }

    //     if waiting_peers.len() % 2 == 0 && waiting_peers.len() > 0 {
    //         println!("Found 2 peers, starting connection");
    //         let first_peer = &waiting_peers[0];
    //         let second_peer = &waiting_peers[1];

    //         let first_peer_public_addr = first_peer.info.public_addr.read().await.clone();
    //         let second_peer_public_addr = second_peer.info.public_addr.read().await.clone();

    //         {
    //             println!(
    //                 "[SignalServer] Sending packet to: {}",
    //                 second_peer.info.local_addr
    //             );
    //             SignalServer::send_peer_info(second_peer.clone(), first_peer_public_addr).await;
    //             println!(
    //                 "[SignalServer] Sended packet to peer: {}",
    //                 second_peer.info.local_addr
    //             );
    //         }
    //         second_peer.set_wait_connection(false).await;
    //         tokio::time::sleep(Duration::from_millis(500)).await;
    //         {
    //             println!(
    //                 "[SignalServer] Sending packet to: {}",
    //                 first_peer.info.local_addr
    //             );
    //             SignalServer::send_peer_info(first_peer.clone(), second_peer_public_addr).await;
    //             println!(
    //                 "[SignalServer] Sended packet to peer: {}",
    //                 first_peer.info.local_addr
    //             );
    //         }
    //         first_peer.set_wait_connection(false).await;
    //     }
    // }
}
