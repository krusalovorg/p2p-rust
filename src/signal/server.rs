use crate::config::Config;
use crate::signal::{Protocol, TransportPacket};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
struct Peer {
    socket: Arc<Mutex<TcpStream>>,
    info: String,
    wait_connection: bool,
}

pub struct SignalServer {
    pub peers: Mutex<Vec<Arc<Mutex<Peer>>>>,
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
            let read_result = Arc::clone(peer_socket).lock().await.read(&mut buf).await;
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
                socket: peer_socket,
                info: peer_info.to_string(),
                wait_connection: is_peer_wait_connection,
            };

            let mut peers_guard = self.peers.lock().await; 
            peers_guard.push(Arc::new(Mutex::new(peer)));

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
                let peer_socket_clone = Arc::clone(&peer_socket);
                peer_socket_clone
                    .clone()
                    .lock()
                    .await
                    .write_all(answer_packet.as_bytes())
                    .await
                    .unwrap();
                println!("Sent answer packet: {}", answer_packet);
            }

            // if is_peer_wait_connection && peers.len() == 2 {
            //     println!("Both peers are ready to connect");
            //     let peer1_info = peers[0].info.clone();
            //     let peer1_socket = peers[0].socket.clone();
            //     let peer2_info = peers[1].info.clone();
            //     let peer2_socket = peers[1].socket.clone();
            //     println!("Peers: {:?}", peers);

            //     // Send peer2 info to peer1
            //     {
            //         let wait_packet = TransportPacket {
            //             public_addr: peer2_info.clone(),
            //             act: "wait_connection".to_string(),
            //             to: Some(peer1_info.clone()),
            //             data: None,
            //             session_key: None,
            //             status: None,
            //             protocol: Protocol::STUN,
            //         };
            //         let wait_packet = serde_json::to_string(&wait_packet).unwrap();
            //         println!("lock peer1_socket");
            //         let mut peer1_socket = peer1_socket.write().await;
            //         println!("Sending peer2 info to peer1: {}", wait_packet);
            //         if let Err(e) = peer1_socket.write_all(wait_packet.as_bytes()).await {
            //             println!("Failed to send peer2 info to peer1: {}", e);
            //         } else {
            //             println!("Successfully sent peer2 info to peer1");
            //         }
            //     }

            //     // Send peer1 info to peer2
            //     {
            //         let wait_packet = TransportPacket {
            //             public_addr: peer1_info.clone(),
            //             act: "wait_connection".to_string(),
            //             to: Some(peer2_info).clone(),
            //             data: None,
            //             session_key: None,
            //             status: None,
            //             protocol: Protocol::STUN,
            //         };
            //         let wait_packet = serde_json::to_string(&wait_packet).unwrap();
            //         let mut peer2_socket = peer2_socket.write().await;
            //         println!("Sending peer1 info to peer2: {}", wait_packet);
            //         if let Err(e) = peer2_socket.write_all(wait_packet.as_bytes()).await {
            //             println!("Failed to send peer1 info to peer2: {}", e);
            //         } else {
            //             println!("Successfully sent peer1 info to peer2");
            //         }
            //     }
            // }
        }
    }
}
