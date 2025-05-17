use bytes::Bytes;
use colored::Colorize;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::tokio::TokioIo;
use uuid::Uuid;
use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::sync::mpsc;
use crate::db::P2PDatabase;
use crate::packets::{Protocol, ProxyMessage, TransportData, TransportPacket};

#[derive(Clone, Debug)]
pub struct HttpProxy {
    db: Arc<P2PDatabase>,
    proxy_tx: mpsc::Sender<TransportPacket>,
    client_to_peer_mapping: Arc<RwLock<HashMap<String, String>>>,
    pending_responses: Arc<Mutex<HashMap<String, oneshot::Sender<Vec<u8>>>>>,
}

impl HttpProxy {
    pub fn new(db: Arc<P2PDatabase>, proxy_tx: mpsc::Sender<TransportPacket>) -> Self {
        Self {
            db,
            proxy_tx,
            client_to_peer_mapping: Arc::new(RwLock::new(HashMap::new())),
            pending_responses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn set_peer_for_client(&self, client_ip: String, peer_id: String) {
        self.client_to_peer_mapping
            .write()
            .await
            .insert(client_ip, peer_id);
    }

    pub async fn get_peer_for_client(&self, client_ip: &str) -> Option<String> {
        self.client_to_peer_mapping
            .read()
            .await
            .get(client_ip)
            .cloned()
    }

    pub async fn set_response(&self, request_id: String, response: Vec<u8>) {
        println!("[HTTP Proxy] Set response for request: {}", request_id);
        if let Some(sender) = self.pending_responses.lock().await.remove(&request_id) {
            let _ = sender.send(response);
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut port = 8080;
        let mut listener = None;
        
        while listener.is_none() {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            match TcpListener::bind(addr).await {
                Ok(l) => {
                    listener = Some(l);
                    println!(
                        "{}",
                        format!("[HTTP Proxy] Listening on http://{}", addr).green()
                    );
                }
                Err(_) => {
                    println!("[HTTP Proxy] Port {} is busy, trying {}", port, port + 1);
                    port += 1;
                }
            }
        }

        let listener = listener.unwrap();

        loop {
            let (stream, socket) = listener.accept().await.unwrap();
            let peer_ip = format!(
                "{}:{}",
                stream.peer_addr().unwrap().ip().to_string(),
                stream.peer_addr().unwrap().port()
            );
            println!("[HTTP Proxy] Peer IP: {}", peer_ip);
            let proxy = self.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req: Request<Incoming>| {
                    let proxy = proxy.clone();
                    let peer_ip = peer_ip.clone();
                    println!("[HTTP Proxy] [thread] Peer IP: {}", peer_ip);
                    async move { proxy.handle(req, peer_ip).await }
                });

                if let Err(err) = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    eprintln!("[HTTP Proxy] Connection error: {:?}", err);
                }
            });
        }
    }

    fn extract_peer_id(req: &Request<Incoming>) -> Option<String> {
        // Проверяем заголовок host
        if let Some(host) = req.headers().get("host") {
            if let Ok(host_str) = host.to_str() {
                if host_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Some(host_str.to_string());
                }
            }
        }

        // Проверяем заголовок referer
        if let Some(referer) = req.headers().get("referer") {
            if let Ok(referer_str) = referer.to_str() {
                if let Some(last_segment) = referer_str.split('/').last() {
                    if last_segment.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Some(last_segment.to_string());
                    }
                }
            }
        }

        // Проверяем путь URI
        let path = req.uri().path();
        if let Some(last_segment) = path.split('/').last() {
            if last_segment.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(last_segment.to_string());
            }
        }

        // Проверяем query параметры
        if let Some(query) = req.uri().query() {
            for param in query.split('&') {
                let parts: Vec<&str> = param.split('=').collect();
                if parts.len() == 2 {
                    let value = parts[1];
                    if value.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Some(value.to_string());
                    }
                }
            }
        }

        None
    }

    async fn get_peer_id_from_request(
        &self,
        req: &Request<Incoming>,
        client_ip: &str,
    ) -> String {
        let mut peer_id = None;

        if peer_id.is_none() {
            if let Some(extracted) = Self::extract_peer_id(req) {
                peer_id = Some(extracted);
            } else {
                peer_id = self.get_peer_for_client(client_ip).await;
            }
        }

        if peer_id.is_none() {
            if let Some(cookie_header) = req.headers().get("cookie") {
                if let Ok(cookie_str) = cookie_header.to_str() {
                    for cookie in cookie_str.split(';') {
                        let parts: Vec<&str> = cookie.trim().split('=').collect();
                        if parts.len() == 2 && parts[0] == "peer_id" {
                            let value = parts[1];
                            if value.chars().all(|c| c.is_ascii_hexdigit()) {
                                peer_id = Some(value.to_string());
                                break;
                            }
                        }
                    }
                }
            }    
        }

        let peer_id = peer_id.unwrap_or_else(|| "".to_string());
        
        if !peer_id.is_empty() {
            self.set_peer_for_client(client_ip.to_string(), peer_id.clone()).await;
        }

        peer_id
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
        client_ip: String,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        println!("[HTTP Proxy] Request Method: {}", req.method());
        println!("[HTTP Proxy] Request URI: {}", req.uri());
        println!("[HTTP Proxy] Request Headers:");
        for (name, value) in req.headers() {
            println!("  {}: {}", name, value.to_str().unwrap_or("(invalid)"));
        }

        let peer_id = self.get_peer_id_from_request(&req, &client_ip).await;
        println!("[HTTP Proxy] Peer ID: {}", peer_id);

        let mut request_str = String::new();
        request_str.push_str(&format!("{} {} HTTP/1.1\r\n", req.method(), req.uri()));

        for (name, value) in req.headers() {
            request_str.push_str(&format!(
                "{}: {}\r\n",
                name,
                value.to_str().unwrap_or("(invalid)")
            ));
        }

        request_str.push_str("\r\n");

        let whole_body = req.collect().await?.to_bytes();
        if !whole_body.is_empty() {
            request_str.push_str(&String::from_utf8_lossy(&whole_body));
        }

        let request_id = Uuid::new_v4().to_string();
        
        let (tx, rx) = oneshot::channel();
        self.pending_responses
            .lock()
            .await
            .insert(request_id.clone(), tx);
        
        let encrypted = match self
            .db
            .encrypt_message(request_str.as_bytes(), &peer_id)
        {
            Ok(e) => e,
            Err(_) => {
                return Ok(Response::builder()
                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                    .body(full("Encryption failed"))
                    .unwrap());
            }
        };

        let packet = TransportPacket {
            act: "http_proxy_request".to_string(),
            to: Some(peer_id.clone()),
            data: Some(TransportData::ProxyMessage(ProxyMessage {
                text: base64::encode(encrypted.0),
                nonce: base64::encode(encrypted.1),
                from_peer_id: self.db.get_or_create_peer_id().unwrap(),
                end_peer_id: peer_id.clone(),
                request_id: request_id.clone(),
            })),
            protocol: Protocol::TURN,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        self.proxy_tx.send(packet).await;

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                let mut header_end = 0;
                for i in 0..response.len().saturating_sub(3) {
                    if &response[i..i + 4] == b"\r\n\r\n" {
                        header_end = i + 4;
                        break;
                    }
                }

                if header_end == 0 {
                    return Ok(Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(full("Failed to parse response"))
                        .unwrap());
                }

                let (header_bytes, body_bytes) = response.split_at(header_end);
                let header_str = String::from_utf8_lossy(header_bytes);

                let mut lines = header_str.lines();
                let status_line = lines.next().unwrap_or("HTTP/1.1 500 Internal Server Error");
                let status_code = status_line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u16>().ok())
                    .unwrap_or(500);

                let mut response_builder = Response::builder().status(status_code);
                let mut content_type_set = false;

                for line in lines {
                    if let Some((name, value)) = line.split_once(':') {
                        let name_trimmed = name.trim();
                        let value_trimmed = value.trim();

                        if name_trimmed.eq_ignore_ascii_case("transfer-encoding") {
                            continue;
                        }

                        if name_trimmed.eq_ignore_ascii_case("content-type") {
                            content_type_set = true;
                        }

                        response_builder = response_builder.header(name_trimmed, value_trimmed);
                    }
                }

                if !content_type_set {
                    response_builder =
                        response_builder.header("Content-Type", "text/html; charset=utf-8");
                }

                let cookie = format!(
                    "peer_id={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=31536000",
                    peer_id
                );
                response_builder = response_builder.header("Set-Cookie", cookie);

                Ok(response_builder
                    .body(Full::new(Bytes::from(body_bytes.to_vec())).boxed())
                    .unwrap())
            }
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .body(full("Proxy timeout"))
                .unwrap()),
        }
    }
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).map_err(|_| unreachable!()).boxed()
}
