use crate::config::Config;
use crate::signal::{Protocol, TransportPacket};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Debug)]
struct Peer {
    socket: Arc<Mutex<TcpStream>>,
    info: String,
    wait_connection: Mutex<bool>,
}

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

        let signal_server_clone = Arc::clone(&self);
        tokio::spawn(async move {
            signal_server_clone.wait_for_peers().await;
        });

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            println!("New connection: {}", socket.peer_addr().unwrap());
            let signal_server_clone = Arc::clone(&self);
            tokio::spawn(async move {
                let _ = signal_server_clone.handle_connection(socket).await;
            });
        }
    }
    async fn handle_connection(self: Arc<Self>, socket: TcpStream) {
        let peer_socket = Arc::new(Mutex::new(socket));
        let mut buf = [0; 1024];
        loop {
            let read_result = Arc::clone(&peer_socket).lock().await.read(&mut buf).await;
            let n = match read_result {
                Ok(0) => {
                    println!("Connection closed by peer");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    println!("Failed to read from socket: {}", e);
                    break;
                }
            };

            let message = String::from_utf8_lossy(&buf[..n]).to_string();
            println!("Received peer info: {:?}", message);
            let message: TransportPacket = serde_json::from_str(&message).unwrap();
            let peer_info = &message.public_addr;

            let is_peer_wait_connection = message.act == "wait_connection";
            let is_peer_wait_stun_connection = message.protocol == Protocol::STUN;
            let peer = Peer {
                socket: peer_socket.clone(),
                info: peer_info.to_string(),
                wait_connection: Mutex::new(is_peer_wait_connection),
            };

            //check if peer is already in the list
            let mut peers_guard = self.peers.lock().await;
            let mut peer_added = false;
            for item in peers_guard.iter() {
                if &item.info == peer_info {
                    println!("Peer already in the list: {}", peer_info);
                    peer_added = true;
                    if is_peer_wait_connection {
                        let mut wait_connection = item.wait_connection.lock().await;
                        *wait_connection = true;
                    }
                    break;
                }
            }
            if !peer_added {
                println!("Peer added to the list: {}", peer_info);
                peers_guard.push(Arc::new(peer));
            }

            if is_peer_wait_connection && is_peer_wait_stun_connection {
                println!("Peer is ready to connect: {}", peer_info);
                //send answer packet
                let answer_packet = TransportPacket {
                    public_addr: peer_info.to_string(),
                    act: "answer".to_string(),
                    to: None,
                    data: None,
                    session_key: None,
                    status: None,
                    protocol: Protocol::STUN,
                };
                println!("Sending answer packet");
                let answer_packet = serde_json::to_string(&answer_packet).unwrap();
                peer_socket
                    .lock()
                    .await
                    .write_all(answer_packet.as_bytes())
                    .await
                    .unwrap();
                println!("Sented answer packet: {}", answer_packet);
            }

            drop(peers_guard);
        }
    }

    async fn send_peer_info(peer: Arc<Peer>, peer_info: String) {
        let wait_packet = TransportPacket {
            public_addr: peer_info.clone(),
            act: "wait_connection".to_string(),
            to: Some(peer_info.clone()),
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };
        let wait_packet = serde_json::to_string(&wait_packet).unwrap();

        let socket = peer.socket.clone(); // Не блокируйте на этой строке
        let mut socket_locked = socket.lock().await; // теперь блокируем только сокет

        println!("Sending peer2 info to peer1: {}", wait_packet);
        if let Err(e) = socket_locked.write_all(wait_packet.as_bytes()).await {
            println!("Failed to send peer2 info to peer1: {}", e);
        } else {
            println!("Successfully sent peer2 info to peer1");
        }
    }

    async fn wait_for_peers(self: Arc<Self>) {
        loop {
            let peers_guard = self.peers.lock().await;

            let mut have_wait_connection_peers = 0;
            for peer in peers_guard.iter() {
                if *peer.wait_connection.lock().await {
                    have_wait_connection_peers += 1;
                }
            }

            if have_wait_connection_peers % 2 == 0 && have_wait_connection_peers > 0 {
                println!("Found 2 peers, starting connection");
                let first_peer = &peers_guard[0];
                let second_peer = &peers_guard[1];

                SignalServer::send_peer_info(
                    first_peer.clone(),
                    second_peer.info.clone(),
                )
                .await;
                SignalServer::send_peer_info(
                    second_peer.clone(),
                    first_peer.info.clone(),
                )
                .await;

                let mut first_peer = first_peer.wait_connection.lock().await;
                *first_peer = false;

                let mut second_peer = second_peer.wait_connection.lock().await;
                *second_peer = false;
            }
            drop(peers_guard);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
