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
            // println!("Received message: {}", message);

            let message: TransportPacket = serde_json::from_str(&message).unwrap();
            let peer_public_addr = &message.public_addr;

            let is_peer_wait_connection = message.act == "wait_connection";

            peer.set_wait_connection(is_peer_wait_connection).await;
            peer.set_public_addr(peer_public_addr.clone()).await;

            if message.act == "info" {
                println!("================");
                println!("CONNECTED PEER INFO:");
                println!("PUBLIC ADDRESS: {}", peer_public_addr);
                println!("LOCAL ADDRESS: {}", peer.info.local_addr);
                println!("================");
            }

            match message.protocol {
                Protocol::STUN => {
                    if (is_peer_wait_connection) {
                        println!("Peer is ready to connect: {}", peer_public_addr);
                        //send answer packet
                        // let answer_packet = TransportPacket {
                        //     public_addr: peer_public_addr.to_string(),
                        //     act: "answer".to_string(),
                        //     to: None,
                        //     data: None,
                        //     session_key: None,
                        //     status: None,
                        //     protocol: Protocol::STUN,
                        // };
                        // println!("Sending answer packet");
                        // let answer_packet = serde_json::to_string(&answer_packet).unwrap();
                        // let res = peer.send(answer_packet).await;
                        // match res {
                        //     Ok(_) => println!(
                        //         "Successfully sent answer packet to peer: {:?}",
                        //         peer.info.local_addr
                        //     ),
                        //     Err(e) => println!("Failed to send answer packet to peer: {}", e),
                        // }

                        if (self.peers.lock().await.len() >= 1) {
                            let server_clone = Arc::clone(&self);
                            server_clone.wait_for_peers().await;
                            println!("End wait peers");
                        }
                    }
                }
                Protocol::TURN => {
                    if message.to != None {
                        println!("Received turn packet: {:#?}", message);
                        let peers_guard = self.peers.lock().await;
                        for item in peers_guard.iter() {
                            if Some(item.info.public_addr.lock().await.to_string()) == message.to {
                                println!("Send turn packet: {} {:#?}", peer.info.local_addr, message);

                                let turn_packet = TransportPacket {
                                    public_addr: message.public_addr.to_string(),
                                    act: message.act.to_string(),
                                    to: message.to,
                                    data: message.data,
                                    session_key: message.session_key,
                                    status: message.status,
                                    protocol: Protocol::TURN,
                                };
                                let turn_packet = serde_json::to_string(&turn_packet).unwrap();
                                item.send(turn_packet).await.unwrap();
                                break;
                            }
                        }
                    }
                }
                Protocol::SIGNAL => {}
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

        let result = peer.send(wait_packet).await;

        match result {
            Ok(_) => println!(
                "[p2p] Successfully send packet to peer: {}. Peer connect to: {}",
                peer.info.public_addr.lock().await,
                public_addr
            ),
            Err(e) => println!("[p2p] Failed to send peer to peer info: {}", e),
        }
    }

    async fn wait_for_peers(self: Arc<Self>) {
        println!("START WAITNG PEERS ");
        loop {
            let peers_snapshot = {
                let peers_guard = self.peers.lock().await;
                peers_guard.clone()
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

                let first_peer_public_addr = first_peer.info.public_addr.lock().await.clone();
                let second_peer_public_addr = second_peer.info.public_addr.lock().await.clone();

                {
                    println!("Sending packet to: {}", second_peer.info.local_addr);
                    SignalServer::send_peer_info(second_peer.clone(), first_peer_public_addr).await;
                    println!("Sended packet to peer: {}", second_peer.info.local_addr);
                }
                second_peer.set_wait_connection(false).await;
                tokio::time::sleep(Duration::from_millis(500)).await;
                {
                    println!("Sending packet to: {}", first_peer.info.local_addr);
                    SignalServer::send_peer_info(first_peer.clone(), second_peer_public_addr).await;
                    println!("Sended packet to peer: {}", first_peer.info.local_addr);
                }
                first_peer.set_wait_connection(false).await;
                
                // println!(
                //     "Before sending: first_peer wait_connection = {}, second_peer wait_connection = {}",
                //     *first_peer.info.wait_connection.lock().await,
                //     *second_peer.info.wait_connection.lock().await,
                // );

                tokio::time::sleep(Duration::from_secs(1)).await;
                break;    
            }
        }
    }
}
