use crate::config::Config;
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, Mutex, RwLock};

#[derive(Debug)]
struct Peer {
    socket: Arc<RwLock<TcpStream>>,
    info: String,
}

type PeersSender = mpsc::Sender<Peer>;
type PeersReceiver = mpsc::Receiver<Peer>;

pub struct SignalServer {
    peers_sender: PeersSender,
    peers_receiver: Arc<Mutex<PeersReceiver>>,
    socket: Option<Arc<RwLock<TcpStream>>>,
    port: i64,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub enum Protocol {
    TURN,
    STUN,
    SIGNAL,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum Status {
    ERROR,
    SUCCESS,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct TransportPacket {
    pub public_addr: String,
    pub act: String, //info, answer, wait_connection,
    pub to: Option<String>,
    pub data: Option<serde_json::Value>,
    pub session_key: Option<String>,
    pub status: Option<Status>, // success, falied
    pub protocol: Protocol,     // TURN, STUN, SIGNAL
}

impl SignalServer {
    pub fn new() -> Self {
        let (peers_sender, peers_receiver) = mpsc::channel(100);
        let config: Config = Config::from_file("config.toml");
        let signal_server_port = config.signal_server_port;

        SignalServer {
            peers_sender,
            peers_receiver: Arc::new(Mutex::new(peers_receiver)),
            socket: None,
            port: signal_server_port,
        }
    }

    pub async fn run(self: Arc<Self>) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(addr.clone()).await.unwrap();
        println!("Signal server running on {}", addr);

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            println!("New connection: {}", socket.peer_addr().unwrap());
            let peers_sender = self.peers_sender.clone();
            let server = self.clone();
            tokio::spawn(async move {
                server.handle_connection(socket, peers_sender).await;
            });
        }
    }

    pub async fn connect(
        &mut self,
        signal_server_ip: &str,
        signal_server_port: i64,
        public_ip: &str,
        public_port: u16,
    ) -> Result<(), String> {
        println!(
            "[signal] Connecting to signal server {}:{}",
            signal_server_ip, signal_server_port
        );
        match TcpStream::connect(format!("{}:{}", signal_server_ip, signal_server_port)).await {
            Ok(socket) => {
                let socket = Arc::new(RwLock::new(socket));
                self.socket = Some(socket);
                let connect_packet = TransportPacket {
                    public_addr: format!("{}:{}", public_ip, public_port),
                    act: "info".to_string(),
                    to: None,
                    data: None,
                    session_key: None,
                    status: None,
                    protocol: Protocol::SIGNAL,
                };
                let connect_packet = serde_json::to_string(&connect_packet).unwrap();
                if let Some(socket) = &self.socket {
                    socket
                        .write()
                        .await
                        .write_all(connect_packet.as_bytes())
                        .await
                        .map_err(|e| {
                            println!("Failed to send connect packet: {}", e);
                            e.to_string()
                        })?;
                    println!("Sent public address to signal server: {}", connect_packet);
                }
                Ok(())
            }
            Err(e) => {
                println!("Failed to connect to signal server: {}", e);
                Err(e.to_string())
            }
        }
    }

    async fn handle_connection(&self, socket: TcpStream, peers_sender: PeersSender) {
        let peer_socket = Arc::new(RwLock::new(socket));
        let mut buf = [0; 1024];
        loop {
            let mut peer_socket_read = peer_socket.write().await;
            let read_result = peer_socket_read.read(&mut buf).await;
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
            println!("Peer message info: {:?}", message);

            let is_peer_wait_connection = message.act == "wait_connection";
            let is_peer_wait_stun_connection = message.protocol == Protocol::STUN;
            let mut peers: Vec<Peer> = Vec::new();

            if is_peer_wait_connection && is_peer_wait_stun_connection {
                let peer = Peer {
                    socket: peer_socket.clone(),
                    info: peer_info.to_string(),
                };
                peers_sender.send(peer).await.unwrap();
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
                    .write()
                    .await
                    .write_all(answer_packet.as_bytes())
                    .await
                    .unwrap();
                println!("Sent answer packet: {}", answer_packet);

                while let Some(peer) = self.peers_receiver.lock().await.recv().await {
                    peers.push(peer);
                    if peers.len() == 2 {
                        break;
                    }
                }
            }

            if is_peer_wait_connection && peers.len() == 2 {
                println!("Both peers are ready to connect");
                let peer1_info = peers[0].info.clone();
                let peer1_socket = peers[0].socket.clone();
                let peer2_info = peers[1].info.clone();
                let peer2_socket = peers[1].socket.clone();
                println!("Peers: {:?}", peers);

                // Send peer2 info to peer1
                {
                    let wait_packet = TransportPacket {
                        public_addr: peer2_info.clone(),
                        act: "wait_connection".to_string(),
                        to: Some(peer1_info.clone()),
                        data: None,
                        session_key: None,
                        status: None,
                        protocol: Protocol::STUN,
                    };
                    let wait_packet = serde_json::to_string(&wait_packet).unwrap();
                    println!("lock peer1_socket");
                    let mut peer1_socket = peer1_socket.write().await;
                    println!("Sending peer2 info to peer1: {}", wait_packet);
                    if let Err(e) = peer1_socket.write_all(wait_packet.as_bytes()).await {
                        println!("Failed to send peer2 info to peer1: {}", e);
                    } else {
                        println!("Successfully sent peer2 info to peer1");
                    }
                }

                // Send peer1 info to peer2
                {
                    let wait_packet = TransportPacket {
                        public_addr: peer1_info.clone(),
                        act: "wait_connection".to_string(),
                        to: Some(peer2_info).clone(),
                        data: None,
                        session_key: None,
                        status: None,
                        protocol: Protocol::STUN,
                    };
                    let wait_packet = serde_json::to_string(&wait_packet).unwrap();
                    let mut peer2_socket = peer2_socket.write().await;
                    println!("Sending peer1 info to peer2: {}", wait_packet);
                    if let Err(e) = peer2_socket.write_all(wait_packet.as_bytes()).await {
                        println!("Failed to send peer1 info to peer2: {}", e);
                    } else {
                        println!("Successfully sent peer1 info to peer2");
                    }
                }
            }
        }
    }

    pub async fn send_packet(&self, packet: TransportPacket) -> Result<(), String> {
        println!("[send_packet] Packet: {:?}", packet);
        let string_packet = serde_json::to_string(&packet).unwrap();

        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }

        if let Some(socket) = &self.socket {
            println!("Sending turn data to signal server: {}", string_packet);

            let result = socket
                .clone()
                .write_owned()
                .await
                .write_all(string_packet.as_bytes())
                .await;
            println!("Sended");

            match result {
                Ok(_) => {
                    return Ok(());
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        return Err("Socket error".to_string());
    }

    // для пира
    pub async fn send_peer_info_request(
        &self,
        public_ip: &str,
        public_port: u16,
    ) -> Result<(), String> {
        let connect_packet = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "wait_connection".to_string(),
            to: None,
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::STUN,
        };
        let connect_packet = serde_json::to_string(&connect_packet).unwrap();

        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }

        if let Some(socket) = &self.socket {
            println!(
                "Sending public address to signal server: {}",
                connect_packet
            );

            let result = socket
                .write()
                .await
                .write_all(connect_packet.as_bytes())
                .await;

            match result {
                Ok(_) => {
                    println!("Sent peer info request: {}", connect_packet);
                    Ok(())
                }
                Err(e) => {
                    println!("Failed to send peer info request: {}", e);
                    Err(e.to_string())
                }
            }
        } else {
            Err("Socket is not connected".to_string())
        }
    }

    //Со стороны юзера получение сообщений от сигнального сервера
    pub async fn receive_message(&self) -> Result<TransportPacket, String> {
        if self.socket.is_none() {
            return Err("Socket is not connected".to_string());
        }
        if let Some(socket) = &self.socket {
            let mut buf: [u8; 1024] = [0; 1024];
            let n = socket
                .write()
                .await
                .read(&mut buf)
                .await
                .map_err(|e| e.to_string())?;
            if n == 0 {
                return Err("Received empty message".to_string());
            }
            let peer_info: String = String::from_utf8_lossy(&buf[..n]).to_string();
            let message: TransportPacket =
                serde_json::from_str(&peer_info).map_err(|e| e.to_string())?;

            Ok(message)
        } else {
            Err("Socket is not connected".to_string())
        }
    }

    pub async fn extract_addr(public_addr: String) -> Result<(String, u16), String> {
        let parts: Vec<&str> = public_addr.split(':').collect();

        if parts.len() != 2 {
            return Err(format!("Invalid peer info received: {}", public_addr));
        }

        let ip = parts[0].to_string();
        let port: u16 = parts[1]
            .parse()
            .map_err(|e: std::num::ParseIntError| e.to_string())?;

        Ok((ip, port))
    }
}
