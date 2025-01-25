use async_std::net::{SocketAddr, UdpSocket};
use async_std::{fs, task};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use std::{str, thread};
use stun_client::*;
use tokio::time::timeout;

#[derive(Serialize, Deserialize)]
struct Message {
    text: String,
}

#[derive(Serialize, Deserialize)]
struct FileMessage {
    filename: String,
    data: Vec<u8>,
}

pub struct Tunnel {
    pub local_port: u16,
    pub public_ip: String,
    pub public_port: u16,
    socket: Option<Arc<UdpSocket>>,
    client: Option<SocketAddr>,
}

impl Tunnel {
    pub async fn new() -> Self {
        let local_port = rand::thread_rng().gen_range(16000..65535);
        let (public_ip, public_port) = Self::stun(local_port).await;
        Tunnel {
            local_port,
            public_ip,
            public_port,
            socket: None,
            client: None,
        }
    }

    async fn stun(port: u16) -> (String, u16) {
        let client = Client::new(format!("0.0.0.0:{}", port), None).await;
        if let Err(e) = client {
            panic!("Failed to create STUN client: {:?}", e);
        }
        let mut client = client.unwrap();

        let res = client.binding_request("stun.l.google.com:19302", None).await;
        if let Err(e) = res {
            panic!("Failed to send binding request: {:?}", e);
        }
        let res = res.unwrap();

        let xor_mapped_addr = Attribute::get_xor_mapped_address(&res);
        if let Some(addr) = xor_mapped_addr {
            (addr.ip().to_string(), addr.port())
        } else {
            let mapped_addr = Attribute::get_mapped_address(&res);
            if let Some(addr) = mapped_addr {
                (addr.ip().to_string(), addr.port())
            } else {
                panic!(
                    "Failed to get XOR mapped address or Mapped address from STUN response: {:?}",
                    res
                );
            }
        }
    }

    pub async fn make_connection(&mut self, ip: &str, port: u16, timeout_default: u64) -> Result<(), String> {
        let addr = format!("{}:{}", ip, port)
            .parse::<SocketAddr>()
            .expect("Invalid address");
        let local_port = self.local_port;
        let mut timeout_count = timeout_default;
        println!("[STUN] Trying to connect to {}:{}", ip, port);

        while timeout_count > 0 {
            let sock = match UdpSocket::bind(format!("0.0.0.0:{}", local_port)).await {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    return Err(format!("[STUN] Failed to bind UDP socket: {:?}", e));
                }
            };

            println!("[STUN] Sending connection request to {}:{}... {}/{}", ip, port, timeout_count, timeout_default);

            if let Err(e) = sock.send_to(b"Con. Request!", addr).await {
                println!("[STUN] Failed to send connection request: {:?}", e);
                timeout_count -= 1;
                continue;
            }

            let mut buf = vec![0u8; 1024];
            match timeout(Duration::from_secs(2), sock.recv_from(&mut buf)).await {
                Ok(res) => match res {
                    Ok((_n, peer)) => {
                        println!("[STUN] Reply received from {}:{}...", peer.ip(), peer.port());
                        if let Err(e) = sock.send_to(b"Con. Request!", addr).await {
                            println!("[STUN] Failed to resend connection request: {:?}", e);
                            timeout_count -= 1;
                            continue;
                        }
                        self.client = Some(addr);
                        self.socket = Some(sock.clone());
                        println!("[STUN] Hole with {} successfully broken!", addr);
                        return Ok(());
                    }
                    Err(e) => {
                        timeout_count -= 1;
                        println!("[STUN] Error while receiving data: {:?}", e);
                    }
                },
                Err(_) => {
                    timeout_count -= 1;
                    println!("[STUN] No handshake with {}:{} yet...", ip, port);
                }
            }
        }

        if self.client.is_none() {
            return Err(format!("[STUN] Failed to establish connection with {}:{}", ip, port));
        }
        Err(format!("[STUN] Failed to establish connection with {}:{}", ip, port))
    }

    pub fn backlife_cycle(&self, freq: u64) {
        if let Some(client) = self.client {
            if let Some(sock) = &self.socket {
                let sock = sock.clone();
                thread::spawn(move || {
                    Self::life_cycle(sock, client, freq);
                });
                println!("[STUN] Session with {} stabilized!", client);
            }
        } else {
            println!("[STUN] No client to stabilize session with.");
        }
    }

    fn life_cycle(sock: Arc<UdpSocket>, client: SocketAddr, freq: u64) {
        loop {
            task::block_on(sock.send_to(b"KPL", client)).unwrap();
            thread::sleep(Duration::from_secs_f64(1.0 / freq as f64));

            let mut buf = vec![0u8; 9999];
            while let Ok((n, reply_addr)) = task::block_on(sock.recv_from(&mut buf)) {
                Self::handle_received_data(&buf[..n], reply_addr, client, sock.clone());
            }
        }
    }

    fn handle_received_data(data: &[u8], reply_addr: SocketAddr, client: SocketAddr, sock: Arc<UdpSocket>) {
        let protocol = &data[..3];
        println!("[STUN] {}: Received {} from {}: {:?}", client.ip(), str::from_utf8(protocol).unwrap(), reply_addr, data);

        if protocol == b"KPL" {
            return;
        } else if protocol == b"MSG" {
            let message: Message = serde_json::from_slice(&data[3..]).unwrap();
            println!("[STUN] Message from {}: {}", client.ip(), message.text);
        } else if protocol == b"FIL" {
            let file_message: FileMessage = serde_json::from_slice(&data[3..]).unwrap();
            println!("[STUN] Received file {} from {}", file_message.filename, client.ip());
            task::block_on(Self::save_file(&file_message.filename, &file_message.data));
        }
    }

    pub async fn send_message(&self, message: &str) {
        let msg = Message {
            text: message.to_string(),
        };
        let msg_bytes = serde_json::to_vec(&msg).unwrap();
        let client = self.client.unwrap();
        self.socket
            .as_ref()
            .unwrap()
            .send_to(&[b"MSG", &msg_bytes[..]].concat(), client)
            .await
            .unwrap();
    }
    
    async fn save_file(filename: &str, data: &[u8]) {
        let path = format!("./received_files/{}", filename);
        if let Err(e) = fs::create_dir_all("./received_files").await {
            println!("Failed to create directory: {:?}", e);
            return;
        }
        if let Err(e) = fs::write(&path, data).await {
            println!("Failed to save file: {:?}", e);
        } else {
            println!("File saved to {}", path);
        }
    }

    pub async fn send_file_path(&self, file_path: &str) {
        let filename = file_path.split('/').last().unwrap().to_string();
        let data = fs::read(file_path).await;
        if let Err(e) = data {
            println!("Failed to read file: {:?}", e);
            return;
        }
        self.send_file(&filename, data.unwrap()).await;
    }

    pub async fn send_file(&self, filename: &str, data: Vec<u8>) {
        let file_message = FileMessage {
            filename: filename.to_string(),
            data,
        };
        let file_message_bytes = serde_json::to_vec(&file_message).unwrap();
        let client = self.client.unwrap();
        self.socket
            .as_ref()
            .unwrap()
            .send_to(&[b"FIL", &file_message_bytes[..]].concat(), client)
            .await
            .unwrap();
    }
}
