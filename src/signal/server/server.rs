use std::sync::Arc;
use std::time::Duration;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::{RwLock, mpsc};

use super::Peer;
use crate::config::Config;
use crate::signal::{Protocol, TransportPacket};

#[derive(Debug)]
pub struct SignalServer {
    pub peers: RwLock<Vec<Arc<Peer>>>,
    port: i64,
    message_tx: mpsc::Sender<(Arc<Peer>, String)>,
}

impl SignalServer {
    pub fn new() -> Arc<Self> {
        let config: Config = Config::from_file("config.toml");
        let (message_tx, mut message_rx) = mpsc::channel(100);

        let server = SignalServer {
            peers: RwLock::new(Vec::new()),
            port: config.signal_server_port,
            message_tx,
        };

        let server_arc = Arc::new(server);

        let server_clone = Arc::clone(&server_arc);
        tokio::spawn(async move {
            while let Some((peer, message)) = message_rx.recv().await {
                server_clone.handle_message(&server_clone, peer, message).await;
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
            println!("[SignalServer] New connection: {}", socket.peer_addr().unwrap());

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
                    println!("[SignalServer] Failed to receive message from peer {}: {}", peer.info.local_addr, e);
                    if e == "Peer disconnected" {
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

    async fn handle_message(&self, server: &Arc<SignalServer>, peer: Arc<Peer>, message: String) {
        println!("[SignalServer] Handling message from peer {}: {}", peer.info.local_addr, message);
        let message: TransportPacket = match serde_json::from_str(&message) {
            Ok(msg) => msg,
            Err(e) => {
                println!("[SignalServer] Failed to parse message from peer {}: {}. Message: {}", peer.info.local_addr, e, message);
                return;
            }
        };
        let peer_public_addr = &message.public_addr;

        let is_peer_wait_connection = message.act == "wait_connection";

        peer.set_wait_connection(is_peer_wait_connection).await;
        peer.set_public_addr(peer_public_addr.clone()).await;

        if message.act == "info" {
            println!("[SignalServer] =================");
            println!("[SignalServer] CONNECTED PEER INFO:");
            println!("[SignalServer] PUBLIC ADDRESS: {}", peer_public_addr);
            println!("[SignalServer] LOCAL ADDRESS: {}", peer.info.local_addr);
            println!("[SignalServer] =================");
        }

        match message.protocol {
            Protocol::STUN => {
                if is_peer_wait_connection {
                    println!("[SignalServer] Peer is ready to connect: {}", peer_public_addr);
                    if server.peers.read().await.len() >= 1 {
                        let server_clone = Arc::clone(server);
                        server_clone.wait_for_peers().await;
                        println!("[SignalServer] End wait peers");
                    }
                }
            }
            Protocol::TURN => {
                if let Some(to) = &message.to {
                    println!("[SignalServer] Received turn packet: {:?}", message);
                    let peers_guard = server.peers.read().await;
                    for item in peers_guard.iter() {
                        if Some(item.info.public_addr.read().await.to_string()) == Some(to.clone()) {
                            println!("[SignalServer] Send turn packet: {} {:?}", peer.info.local_addr, message);

                            let turn_packet = TransportPacket {
                                public_addr: message.public_addr.to_string(),
                                act: message.act.to_string(),
                                to: message.to.clone(),
                                data: message.data.clone(),
                                session_key: message.session_key.clone(),
                                status: message.status.clone(),
                                protocol: Protocol::TURN,
                            };
                            let turn_packet = serde_json::to_string(&turn_packet).unwrap();
                            if let Err(e) = item.send(turn_packet).await {
                                println!("[SignalServer] Failed to send turn packet to peer {}: {}", item.info.local_addr, e);
                            } else {
                                println!("[SignalServer] Successfully send turn packet to peer {}", item.info.local_addr);
                            }
                            break;
                        }
                    }
                }
            }
            Protocol::SIGNAL => {}
        }
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

    async fn send_peer_info(peer: Arc<Peer>, public_addr: String) {
        let wait_packet = TransportPacket {
            public_addr: public_addr.clone(), //к кому будет пытаться подключиться пир
            act: "wait_connection".to_string(),
            to: None, //кому отправляем данный пакет
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };
        let wait_packet = serde_json::to_string(&wait_packet).unwrap();
        
        println!("[SignalServer] Sending wait packet to peer: {}", peer.info.local_addr);
        let result = peer.send(wait_packet).await;

        match result {
            Ok(_) => println!(
                "[SignalServer] Successfully sent packet to peer: {}. Peer connect to: {}",
                peer.info.public_addr.read().await,
                public_addr
            ),
            Err(e) => println!("[SignalServer] Failed to send peer to peer info: {}", e),
        }
    }

    async fn wait_for_peers(self: Arc<Self>) {
        println!("START WAITNG PEERS ");
        // loop {
        let peers_snapshot = {
            let peers_guard = self.peers.read().await;
            peers_guard.clone()
        };

        let mut waiting_peers: Vec<Arc<Peer>> = Vec::new();
        for peer in peers_snapshot.iter() {
            if *peer.info.wait_connection.read().await {
                waiting_peers.push(peer.clone());
            }
        }

        if waiting_peers.len() % 2 == 0 && waiting_peers.len() > 0 {
            println!("Found 2 peers, starting connection");
            let first_peer = &waiting_peers[0];
            let second_peer = &waiting_peers[1];

            let first_peer_public_addr = first_peer.info.public_addr.read().await.clone();
            let second_peer_public_addr = second_peer.info.public_addr.read().await.clone();

            {
                println!("[SignalServer] Sending packet to: {}", second_peer.info.local_addr);
                SignalServer::send_peer_info(second_peer.clone(), first_peer_public_addr).await;
                println!("[SignalServer] Sended packet to peer: {}", second_peer.info.local_addr);
            }
            second_peer.set_wait_connection(false).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            {
                println!("[SignalServer] Sending packet to: {}", first_peer.info.local_addr);
                SignalServer::send_peer_info(first_peer.clone(), second_peer_public_addr).await;
                println!("[SignalServer] Sended packet to peer: {}", first_peer.info.local_addr);
            }
            first_peer.set_wait_connection(false).await;
            
            // println!(
            //     "Before sending: first_peer wait_connection = {}, second_peer wait_connection = {}",
            //     *first_peer.info.wait_connection.read().await,
            //     *second_peer.info.wait_connection.read().await,
            // );

            // tokio::time::sleep(Duration::from_secs(1)).await;
            // break;    
        }
        // }
    }
}
