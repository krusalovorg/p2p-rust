use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use super::Peer;
use crate::config::Config;
use crate::signal::{Protocol, TransportPacket};

pub struct SignalServer {
    pub peers: Mutex<Vec<Arc<Peer>>>,
    port: i64,
}

impl SignalServer {
    pub fn new() -> Self {
        let config: Config = Config::from_file("config.toml");
        SignalServer {
            peers: Mutex::new(Vec::new()),
            port: config.signal_server_port,
        }
    }

    pub async fn run(self: Arc<Self>) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(addr.clone()).await.unwrap();
        println!("Signal server running on {}", addr);

        // let signal_server_clone = Arc::clone(&self);
        // tokio::spawn(async move {
        //     signal_server_clone.wait_for_peers().await;
        // });

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            println!("New connection: {}", socket.peer_addr().unwrap());

            let peer = Peer::new(socket, None);

            let server_clone = Arc::clone(&self);
            server_clone.add_peer(peer.clone(), false).await.unwrap();

            let peer_clone = peer.clone();
            
            tokio::spawn(async move {
                server_clone.handle_connection(peer_clone.clone()).await;
            });
        }
    }
    async fn handle_connection(self: Arc<Self>, peer: Arc<Peer>) {
        loop {
            let message = peer.receive().await.unwrap();
            println!("Received message: {}", message);

            let message: TransportPacket = serde_json::from_str(&message).unwrap();
            let peer_public_addr = &message.public_addr;

            let is_peer_wait_connection = message.act == "wait_connection";
            let is_peer_wait_stun_connection = message.protocol == Protocol::STUN;
            peer.set_wait_connection(is_peer_wait_connection).await;
            peer.set_public_addr(peer_public_addr.clone()).await;

            if is_peer_wait_connection && is_peer_wait_stun_connection {
                println!("Peer is ready to connect: {}", peer_public_addr);
                //send answer packet
                let answer_packet = TransportPacket {
                    public_addr: peer_public_addr.to_string(),
                    act: "answer".to_string(),
                    to: None,
                    data: None,
                    session_key: None,
                    status: None,
                    protocol: Protocol::STUN,
                };
                println!("Sending answer packet");
                let answer_packet = serde_json::to_string(&answer_packet).unwrap();
                let res = peer.send(answer_packet).await;
                match res {
                    Ok(_) => println!("Successfully sent answer packet to peer: {:?}", peer.info.local_addr),
                    Err(e) => println!("Failed to send answer packet to peer: {}", e),
                }

                let server_clone = Arc::clone(&self);
                server_clone.wait_for_peers().await;
            }
        }
    }

    async fn add_peer(
        &self,
        peer: Arc<Peer>,
        is_peer_wait_connection: bool,
    ) -> Result<Arc<Peer>, String> {
        let mut peers_guard = self.peers.lock().await;
        let mut peer_added = false;
        let mut peer_res: Option<Arc<Peer>> = None;
        for item in peers_guard.iter() {
            if *item.info.local_addr == peer.info.local_addr {
                println!("Peer already in the list: {}", peer.info.local_addr);
                peer_added = true;
                if is_peer_wait_connection {
                    let mut wait_connection = item.info.wait_connection.lock().await;
                    *wait_connection = true;
                }
                peer_res = Some(item.clone());
                break;
            }
        }
        if !peer_added {
            println!("Peer added to the list: {:?}", peer.info);
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
            public_addr: public_addr.clone(),
            act: "wait_connection".to_string(),
            to: Some(public_addr.clone()),
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };
        let wait_packet = serde_json::to_string(&wait_packet).unwrap();

        let result = peer.send(wait_packet).await;

        match result {
            Ok(_) => println!("[p2p] Successfully sent peer to peer info"),
            Err(e) => println!("[p2p] Failed to send peer to peer info: {}", e),
        }
    }

    async fn wait_for_peers(self: Arc<Self>) {
        // loop {
            let peers_snapshot = {
                let peers_guard = self.peers.lock().await;
                peers_guard.clone() // Извлекаем копию ссылок на Arc<Peer>
            };

            let mut waiting_peers: Vec<Arc<Peer>> = Vec::new();
            for peer in peers_snapshot.iter() {
                if *peer.info.wait_connection.lock().await {
                    waiting_peers.push(peer.clone());
                }
            }

            if waiting_peers.len() % 2 == 0 && waiting_peers.len() > 0 {
                println!("Found 2 peers, starting connection");
                let first_peer = &waiting_peers[0];
                let second_peer = &waiting_peers[1];

                {
                    SignalServer::send_peer_info(first_peer.clone(), second_peer.info.public_addr.lock().await.clone()).await;
                    SignalServer::send_peer_info(second_peer.clone(), first_peer.info.public_addr.lock().await.clone()).await;
                    first_peer.set_wait_connection(true).await;
                    second_peer.set_wait_connection(true).await;
                }
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        // }
    }
}
