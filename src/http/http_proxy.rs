use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{RwLock, oneshot, Mutex};
use colored::Colorize;

use crate::signal::SignalServer;
use crate::packets::{Message, Protocol, ProxyMessage, TransportData, TransportPacket};

#[derive(Debug, Clone)]
pub struct HttpProxy {
    server: Arc<SignalServer>,
    pub active_connections: Arc<Mutex<HashMap<String, tokio::net::TcpStream>>>,
    client_to_peer_mapping: Arc<RwLock<HashMap<String, String>>>, // client_ip -> peer_id
    pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Vec<u8>>>>>, // client_ip -> sender
}

impl HttpProxy {
    pub fn new(server: Arc<SignalServer>) -> Self {
        Self {
            server,
            active_connections: Arc::new(Mutex::new(HashMap::new())),
            client_to_peer_mapping: Arc::new(RwLock::new(HashMap::new())),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn set_peer_for_client(&self, client_ip: String, peer_id: String) {
        let mut mapping = self.client_to_peer_mapping.write().await;
        mapping.insert(client_ip, peer_id);
    }

    pub async fn get_peer_for_client(&self, client_ip: &str) -> Option<String> {
        let mapping = self.client_to_peer_mapping.read().await;
        mapping.get(client_ip).cloned()
    }

    pub async fn set_response(&self, client_ip: String, response: Vec<u8>) {
        println!("Setting response for client {}", client_ip);
        if let Some(sender) = self.pending_responses.lock().await.remove(&client_ip) {
            if let Err(e) = sender.send(response) {
                println!("Failed to send response to client {}: {:?}", client_ip, e);
            } else {
                println!("Successfully sent response to client {}", client_ip);
            }
        } else {
            println!("No pending response found for client {}", client_ip);
        }
    }

    pub async fn start(&self) {
        let mut port = 80;
        let mut listener = loop {
            match TcpListener::bind(format!("0.0.0.0:{}", port)).await {
                Ok(l) => break l,
                Err(_) => {
                    println!("Порт {} занят, пробуем следующий...", port);
                    port += 1;
                }
            }
        };
        println!("HTTP proxy слушает на 0.0.0.0:{}", port);

        let mut handles = Vec::new();
        
        loop {
            let (socket, addr) = listener.accept().await.unwrap();
            let client_ip = socket.peer_addr().unwrap().to_string();
            println!("HTTP proxy connected to {}", client_ip);
            
            let server = self.server.clone();
            let active_connections = self.active_connections.clone();
            let client_ip = client_ip.clone();
            let http_proxy = self.clone();

            // Сохраняем соединение в HashMap
            {
                let mut connections = self.active_connections.lock().await;
                connections.insert(client_ip.clone(), socket);
            }
            println!("Inserted connection to {}", client_ip);

            // Запускаем обработку в отдельной задаче
            let handle = tokio::spawn(async move {
                Self::handle_client_connection(
                    Arc::new(http_proxy),
                    server,
                    active_connections,
                    client_ip,
                ).await;
            });
            
            handles.push(handle);
            
            // Очищаем завершенные задачи
            handles.retain(|h| !h.is_finished());
        }
    }

    fn extract_peer_id(request_str: &str) -> Option<String> {
        // Сначала проверяем Referer заголовок
        if let Some(referer) = request_str.lines()
            .find(|line| line.starts_with("Referer:"))
            .and_then(|line| line.split("Referer:").nth(1))
            .and_then(|url| url.trim().split("/").last())
        {
            // Проверяем, что это похоже на peer_id (hex строка)
            if referer.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(referer.to_string());
            }
        }

        // Если не нашли в Referer, проверяем путь GET запроса
        if let Some(first_line) = request_str.lines().next() {
            if first_line.starts_with("GET") {
                if let Some(path) = first_line.split_whitespace().nth(1) {
                    // Убираем начальный слеш и проверяем peer_id
                    if let Some(peer_id) = path.strip_prefix("/") {
                        // Проверяем, что это похоже на peer_id (hex строка)
                        if peer_id.chars().all(|c| c.is_ascii_hexdigit()) {
                            return Some(peer_id.to_string());
                        }
                    }
                }
            }
        }

        None
    }

    async fn handle_client_connection(
        http_proxy: Arc<HttpProxy>, 
        server: Arc<SignalServer>,
        active_connections: Arc<Mutex<HashMap<String, tokio::net::TcpStream>>>,
        client_ip: String,
    ) {
        let mut buffer = vec![0; 8192];
        
        let mut connections = active_connections.lock().await;
        let socket = match connections.get_mut(&client_ip) {
            Some(socket) => socket,
            None => {
                println!("Connection not found for {}", client_ip);
                return;
            }
        };
        
        match socket.read(&mut buffer).await {
            Ok(0) => {
                println!("Connection closed by client {}", client_ip);
            }
            Ok(n) => {
                let request_data = buffer[..n].to_vec();
                let request_str = String::from_utf8_lossy(&request_data);
                
                println!("{}", "=".repeat(80).yellow());
                println!("{}", "NEW HTTP REQUEST:".cyan().bold());
                println!("Client IP: {}", client_ip);
                println!("Request size: {} bytes", n);

                // Сначала пробуем получить peer_id из URL
                let target_peer_id = match Self::extract_peer_id(&request_str) {
                    Some(peer_id) => {
                        println!("Found peer_id in URL: {}", peer_id);
                        // Если нашли peer_id в URL, сохраняем его для этого клиента
                        let proxy = http_proxy.clone();
                        proxy.set_peer_for_client(client_ip.clone(), peer_id.clone()).await;
                        peer_id
                    },
                    None => {
                        // Если не нашли в URL, пробуем получить из сохраненного маппинга
                        let proxy = http_proxy.clone();
                        match proxy.get_peer_for_client(&client_ip).await {
                            Some(peer_id) => {
                                println!("Found peer_id in mapping: {}", peer_id);
                                peer_id
                            },
                            None => {
                                println!("Failed to determine target peer ID for client {}", client_ip);
                                return;
                            }
                        }
                    }
                };

                println!("{}{}", "TARGET PEER ID: ".cyan().bold(), target_peer_id.white());
                println!("{}", "=".repeat(80).yellow());

                let response = Self::handle_http_proxy_request(
                    &http_proxy,
                    request_data,
                    target_peer_id,
                    server.clone(),
                    client_ip.clone(),
                ).await;

                println!("{}", "=".repeat(80).yellow());
                println!("{}", "SENDING RESPONSE:".cyan().bold());
                println!("Client IP: {}", client_ip);
                println!("Response size: {} bytes", response.len());
                println!("{}", "=".repeat(80).yellow());

                if let Err(e) = socket.write_all(&response).await {
                    println!("Failed to send response to {}: {}", client_ip, e);
                } else {
                    println!("Successfully sent response to {}", client_ip);
                }
            }
            Err(e) => {
                println!("Failed to read from socket for client {}: {}", client_ip, e);
            }
        }
    }

    async fn handle_http_proxy_request(
        &self,
        request_data: Vec<u8>,
        target_peer_id: String,
        server: Arc<SignalServer>,
        client_ip: String,
    ) -> Vec<u8> {
        let base64_proxy_request = base64::encode(&request_data);
        
        println!("{}", "=".repeat(80).yellow());
        println!("Target peer ID: {}", target_peer_id);
        println!("{}", "=".repeat(80).yellow());

        let encrypted_proxy_request = match server
            .db
            .encrypt_message(base64_proxy_request.as_bytes(), &target_peer_id)
        {
            Ok(result) => result,
            Err(e) => {
                println!("Encryption failed: {:?}", e);
                return vec![];
            }
        };

        // Создаем канал для получения ответа
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending_responses.lock().await;
            pending.insert(client_ip.clone(), tx);
        }

        let packet = TransportPacket {
            act: "http_proxy_request".to_string(),
            to: Some(target_peer_id.clone()),
            data: Some(TransportData::ProxyMessage(ProxyMessage {
                text: base64::encode(encrypted_proxy_request.0),
                nonce: base64::encode(encrypted_proxy_request.1),
                from_peer_id: server.db.get_or_create_peer_id().unwrap(),
                end_peer_id: target_peer_id.clone(),
                request_id: client_ip.clone(), 
            })),
            protocol: Protocol::TURN,
            uuid: server.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        println!("Sending proxy request to peer {}", target_peer_id);
        server.auto_send_packet(packet).await;
        
        // Ждем ответа с таймаутом
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                println!("Received response for client {}", client_ip);
                response
            },
            Ok(Err(_)) => {
                println!("Failed to receive response for client {}", client_ip);
                vec![]
            }
            Err(_) => {
                println!("Timeout waiting for response for client {}", client_ip);
                vec![]
            }
        }
    }
} 