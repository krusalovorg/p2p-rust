use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};

#[derive(Debug)]
pub enum Message {
    SendData(String),
    GetResponse { tx: oneshot::Sender<String> },
    SetWaitConnection(bool),
}

#[derive(Debug)]
pub struct InfoPeer {
    pub wait_connection: Mutex<bool>,
    pub public_addr: Mutex<String>,
    pub local_addr: String,
}

#[derive(Debug)]
pub struct Peer {
    socket: Arc<RwLock<TcpStream>>, // TcpStream wrapped in Arc and Mutex for shared access
    pub info: InfoPeer,              // Peer information (ip:port)
    tx: mpsc::Sender<Message>,     // Sender to the actor's message channel
}

impl Peer {
    pub fn new(socket: TcpStream, info: Option<InfoPeer>) -> Arc<Self> {
        let (tx, mut rx) = mpsc::channel(100); // Message channel for actor
        let mut info = info;

        if info.is_none() {
            info = Some(InfoPeer {
                wait_connection: Mutex::new(false),
                public_addr: Mutex::new("".to_string()),
                local_addr: socket.peer_addr().unwrap().to_string(),
            });
        }

        let socket = Arc::new(RwLock::new(socket));

        let peer = Arc::new(Self {
            socket,
            info: info.unwrap(),
            tx,
        });

        let peer_clone = Arc::clone(&peer);
        tokio::spawn(async move {
            peer_clone.process_message(&mut rx).await;
        });

        peer
    }


    async fn process_message(&self, rx: &mut mpsc::Receiver<Message>) {
        while let Some(message) = rx.recv().await {
            match message {
                Message::SendData(msg) => {
                    println!("[Peer] Sending message to peer {}: {}", self.info.local_addr, msg);
                    self.send_data(&msg).await;
                }
                Message::GetResponse { tx } => {
                    let response = self.receive_message().await;
                    println!("[Peer] Received response from peer {}: {}", self.info.local_addr, response.clone().unwrap());
                    match response {
                        Ok(response) => {
                            tx.send(response).unwrap();
                        }
                        Err(e) => {
                            println!("[Peer] Failed to receive response from peer {}: {}", self.info.local_addr, e);
                        }
                    }
                }
                Message::SetWaitConnection(wait_connection_new) => {
                    self.set_wait_connection(wait_connection_new).await;
                }
            }
        }
    }

    pub async fn send_data(&self, message: &str) {
        if let Err(e) = self.socket.clone().write().await.write_all(message.as_bytes()).await {
            println!("Failed to send message to peer {}: {}", self.info.local_addr, e);
        }
         else {
            //происходит deadlock, не отправляется пиру потому что пир используется 
            println!("[SendData] Message sent to peer {}: {}", self.info.local_addr, message);
        }
    }

    pub async fn receive_message(&self) -> Result<String, String> {
        let mut buf = [0; 1024];
        let n = match self.socket.write().await.read(&mut buf).await {
            Ok(0) => {
                println!("Peer {} disconnected", self.info.local_addr);
                return Err("Peer disconnected".to_string());
            }
            Ok(n) => n,
            Err(e) => {
                println!("Error reading from peer {}: {}", self.info.local_addr, e);
                return Err(e.to_string());
            }
        };

        let message = String::from_utf8_lossy(&buf[..n]).to_string();

        return Ok(message);
    }

    pub async fn set_wait_connection(&self, wait_connection_new: bool) {
        let mut wait_connection = self.info.wait_connection.lock().await;
        *wait_connection = wait_connection_new;
    }

    pub async fn set_public_addr(&self, public_addr: String) {
        let mut public_addr_now = self.info.public_addr.lock().await;
        *public_addr_now = public_addr;
    }

    pub async fn send(&self, packet: String) -> Result<(), String> {
        let result = self.tx.send(Message::SendData(packet)).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn receive(&self) -> Result<String, String> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Message::GetResponse { tx }).await.unwrap();
        match rx.await {
            Ok(response) => Ok(response),
            Err(_) => Err("Failed to receive response from server".to_string()),
        }
    }
}
