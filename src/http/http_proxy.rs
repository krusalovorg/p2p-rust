use crate::db::P2PDatabase;
use crate::packets::{
    FragmentSearchRequest, Protocol, ProxyMessage, TransportData, TransportPacket,
};
use bytes::Bytes;
use colored::Colorize;
use dashmap::DashMap;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::tokio::TokioIo;
use std::time::{Duration, Instant};
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use std::io::{self, Read};
use flate2::read::{GzEncoder, GzDecoder};
use flate2::Compression;

#[derive(Clone, Debug)]
struct CachedFile {
    content: Vec<u8>,
    mime_type: String,
    expires_at: Instant,
}

#[derive(Clone, Debug)]
pub struct HttpProxy {
    db: Arc<P2PDatabase>,
    proxy_tx: mpsc::Sender<TransportPacket>,
    client_to_peer_mapping: Arc<DashMap<String, String>>,
    pending_responses: Arc<DashMap<String, oneshot::Sender<TransportPacket>>>,
    fragment_cache: Arc<DashMap<String, String>>,
    file_cache: Arc<DashMap<String, CachedFile>>,
    path_blobs: String,
}

impl HttpProxy {
    pub fn new(
        db: Arc<P2PDatabase>,
        proxy_tx: mpsc::Sender<TransportPacket>,
        path_blobs: String,
    ) -> Self {
        Self {
            db,
            proxy_tx,
            client_to_peer_mapping: Arc::new(DashMap::new()),
            pending_responses: Arc::new(DashMap::new()),
            fragment_cache: Arc::new(DashMap::new()),
            file_cache: Arc::new(DashMap::new()),
            path_blobs: path_blobs,
        }
    }

    pub async fn set_peer_for_client(&self, client_ip: String, peer_id: String) {
        self.client_to_peer_mapping.insert(client_ip, peer_id);
    }

    pub async fn get_peer_for_client(&self, client_ip: &str) -> Option<String> {
        self.client_to_peer_mapping
            .get(client_ip)
            .map(|v| v.clone())
    }

    pub async fn set_response(&self, request_id: String, response: TransportPacket) {
        println!("[HTTP Proxy] Set response for request: {}", request_id);
        if let Some((_, sender)) = self.pending_responses.remove(&request_id) {
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
        println!("[HTTP Proxy] [DEBUG] Starting peer ID extraction");

        if let Some(host) = req.headers().get("host") {
            if let Ok(host_str) = host.to_str() {
                println!("[HTTP Proxy] [DEBUG] Checking host header: {}", host_str);
                if host_str.chars().all(|c| c.is_ascii_hexdigit()) {
                    println!(
                        "[HTTP Proxy] [DEBUG] Found valid peer ID in host: {}",
                        host_str
                    );
                    return Some(host_str.to_string());
                }
            }
        }

        let path = req.uri().path();
        println!("[HTTP Proxy] [DEBUG] Checking URI path: {}", path);

        for segment in path.split('/') {
            if !segment.is_empty() && segment.chars().all(|c| c.is_ascii_hexdigit()) {
                println!(
                    "[HTTP Proxy] [DEBUG] Found valid peer ID in URI path: {}",
                    segment
                );
                return Some(segment.to_string());
            }
        }

        if let Some(query) = req.uri().query() {
            println!("[HTTP Proxy] [DEBUG] Checking query parameters: {}", query);
            for param in query.split('&') {
                let parts: Vec<&str> = param.split('=').collect();
                if parts.len() == 2 {
                    let value = parts[1];
                    if value.chars().all(|c| c.is_ascii_hexdigit()) {
                        println!(
                            "[HTTP Proxy] [DEBUG] Found valid peer ID in query: {}",
                            value
                        );
                        return Some(value.to_string());
                    }
                }
            }
        }

        println!("[HTTP Proxy] [DEBUG] No valid peer ID found in request");
        None
    }

    async fn get_peer_id_from_request(&self, req: &Request<Incoming>, client_ip: &str) -> String {
        println!(
            "[HTTP Proxy] [DEBUG] Starting peer ID extraction for client IP: {}",
            client_ip
        );
        let mut peer_id = None;

        if peer_id.is_none() {
            if let Some(extracted) = Self::extract_peer_id(req) {
                println!(
                    "[HTTP Proxy] [DEBUG] Successfully extracted peer ID from request: {}",
                    extracted
                );
                peer_id = Some(extracted);
            } else {
                println!(
                    "[HTTP Proxy] [DEBUG] No peer ID found in request, checking client mapping"
                );
                peer_id = self.get_peer_for_client(client_ip).await;
                if let Some(id) = &peer_id {
                    println!(
                        "[HTTP Proxy] [DEBUG] Found peer ID in client mapping: {}",
                        id
                    );
                }
            }
        }

        let peer_id = peer_id.unwrap_or_else(|| {
            println!("[HTTP Proxy] [DEBUG] No peer ID found, using empty string");
            "".to_string()
        });

        if !peer_id.is_empty() && peer_id.len() == 66 {
            println!(
                "[HTTP Proxy] [DEBUG] Setting peer ID {} for client {}",
                peer_id, client_ip
            );
            self.set_peer_for_client(client_ip.to_string(), peer_id.clone())
                .await;
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

        let mut peer_id = self.get_peer_id_from_request(&req, &client_ip).await;
        println!("[HTTP Proxy] Peer ID: {}", peer_id);

        let uri_path = req.uri().path().trim_matches('/').to_string();
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
        let mut this_peer_storage_file = false;
        let mut file_hash = None;
        let mut mime = None;

        let is_hash = (peer_id.len() == 64 || peer_id.len() == 66) && peer_id.chars().all(|c| c.is_ascii_hexdigit());

        if !is_hash {
            println!("[HTTP Proxy] [ERROR] Invalid hash format or length. Expected 64 or 66 characters hex string");
            return Ok(Response::builder()
                .status(hyper::StatusCode::BAD_REQUEST)
                .body(full("Invalid hash format. Expected 64 or 66 characters hex string"))
                .unwrap());
        }

        println!("[HTTP Proxy] [DEBUG] Searching for fragments by hash");

        if let Some(cached_owner) = self.fragment_cache.get(&peer_id) {
            println!(
                "[HTTP Proxy] [DEBUG] Found cached owner for hash: {}",
                cached_owner.clone()
            );
            peer_id = cached_owner.clone();
            this_peer_storage_file = *cached_owner == *self.db.get_or_create_peer_id().unwrap();
            if this_peer_storage_file {
                if let Ok(fragments) = self.db.search_fragment_in_virtual_storage(&peer_id, None) {
                    if let Some(fragment) = fragments.first() {
                        file_hash = Some(fragment.file_hash.clone());
                        mime = Some(fragment.mime.clone());
                    }
                }
            }
        } else {
            if let Ok(fragments) = self.db.search_fragment_in_virtual_storage(&peer_id, None) {
                if let Some(fragment) = fragments.first() {
                    println!("[HTTP Proxy] [DEBUG] Found fragment in local storage");
                    peer_id = fragment.storage_peer_key.clone();
                    this_peer_storage_file =
                        fragment.storage_peer_key == self.db.get_or_create_peer_id().unwrap();
                    file_hash = Some(fragment.file_hash.clone());
                    mime = Some(fragment.mime.clone());
                } else {
                    let search_packet = TransportPacket {
                        act: "search_fragments".to_string(),
                        to: None,
                        data: Some(TransportData::FragmentSearchRequest(
                            FragmentSearchRequest {
                                query: peer_id.clone(),
                                request_id: request_id.clone(),
                            },
                        )),
                        protocol: Protocol::TURN,
                        peer_key: self.db.get_or_create_peer_id().unwrap(),
                        uuid: request_id.clone(),
                        nodes: vec![],
                    };

                    let (search_tx, search_rx) = oneshot::channel();
                    self.pending_responses.insert(request_id.clone(), search_tx);

                    self.proxy_tx.send(search_packet).await.unwrap();

                    if let Ok(search_response) =
                        tokio::time::timeout(tokio::time::Duration::from_secs(5), search_rx)
                            .await
                    {
                        if let Ok(response_data) = search_response {
                            println!("[HTTP Proxy] Search response: {:?}", response_data);
                            if let Some(TransportData::FragmentSearchResponse(response)) =
                                response_data.data
                            {
                                for fragment in response.fragments {
                                    if fragment.file_hash == peer_id {
                                        self.fragment_cache.insert(
                                            peer_id.clone(),
                                            fragment.storage_peer_key.clone(),
                                        );
                                        peer_id = fragment.storage_peer_key.clone();
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let file_hash_clone = file_hash.clone();
        if file_hash.is_some() {
            let file_hash_str = file_hash.unwrap();
            println!("[HTTP PROXY] Processing file request for hash: {}", file_hash_str);
            
            if let Some((cached_content, cached_mime)) = self.get_cached_file(&file_hash_str) {
                println!("[HTTP PROXY] Serving file from cache: {}", file_hash_str);

                let mut decoder = GzDecoder::new(&cached_content[..]);
                let mut decompressed = Vec::new();
                decoder.read_to_end(&mut decompressed).unwrap();

                return Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", cached_mime)
                    .header("Cache-Control", "no-cache")
                    .body(Full::new(Bytes::from(decompressed)).boxed())
                    .unwrap());
            }

            if this_peer_storage_file {
                println!("[HTTP PROXY] File is stored locally, serving from disk");
                let file_hash_str = file_hash_clone.unwrap();

                let file_path = format!("{}/{}", self.path_blobs, file_hash_str);
                println!("[HTTP PROXY] File path: {}", file_path);

                if !std::path::Path::new(&file_path).exists() {
                    println!("[HTTP PROXY] File not found at path: {}", file_path);
                    return Ok(Response::builder()
                        .status(hyper::StatusCode::NOT_FOUND)
                        .body(full("File not found"))
                        .unwrap());
                }

                let file_content = std::fs::read(&file_path).unwrap();
                let mime_type = mime.unwrap_or("application/octet-stream".to_string());

                let mut encoder = GzEncoder::new(&file_content[..], Compression::best());
                let mut compressed = Vec::new();
                encoder.read_to_end(&mut compressed).unwrap();

                println!("[HTTP PROXY] Caching file with hash: {}", file_hash_str);
                self.cache_file(
                    file_hash_str.clone(),
                    compressed.clone(),
                    mime_type.clone(),
                );

                return Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Encoding", "gzip")
                    .header("Cache-Control", "no-cache")
                    .body(Full::new(Bytes::from(compressed)).boxed())
                    .unwrap());
            }
        }

        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let encrypted = match self.db.encrypt_message(request_str.as_bytes(), &peer_id) {
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
                mime: "text/html".to_string(),
                from_peer_id: self.db.get_or_create_peer_id().unwrap(),
                end_peer_id: peer_id.clone(),
                request_id: request_id.clone(),
            })),
            protocol: Protocol::TURN,
            peer_key: self.db.get_or_create_peer_id().unwrap(),
            uuid: request_id.clone(),
            nodes: vec![],
        };

        self.proxy_tx.send(packet).await.unwrap();

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(packet)) => {
                if let Some(TransportData::ProxyMessage(msg)) = packet.data {
                    let file_content = match base64::decode(&msg.text) {
                        Ok(content) => content,
                        Err(_) => {
                            return Ok(Response::builder()
                                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                .body(full("Failed to decode file content"))
                                .unwrap());
                        }
                    };

                    let mime_type = msg.mime.clone();
                    let nonce_decoded = match base64::decode(&msg.nonce) {
                        Ok(n) => {
                            let mut nonce_array = [0u8; 12];
                            nonce_array.copy_from_slice(&n[..12]);
                            nonce_array
                        }
                        Err(_) => {
                            return Ok(Response::builder()
                                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                .body(full("Failed to decode nonce"))
                                .unwrap());
                        }
                    };

                    let decrypted =
                        match self
                            .db
                            .decrypt_message(&file_content, nonce_decoded, &peer_id)
                        {
                            Ok(d) => d,
                            Err(_) => {
                                return Ok(Response::builder()
                                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(full("Failed to decrypt file content"))
                                    .unwrap());
                            }
                        };

                    let mut decoder = GzDecoder::new(&decrypted[..]);
                    let mut decompressed = Vec::new();
                    if decoder.read_to_end(&mut decompressed).is_err() {
                        decompressed = decrypted.clone();
                    }

                    let mut response_builder = Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header("Content-Type", &mime_type)
                        .header("Content-Length", decompressed.len().to_string())
                        .header("Cache-Control", "no-cache")
                        .header("X-Content-Type-Options", "nosniff");

                    // let cookie = format!(
                    //     "peer_id={}; Path=/; HttpOnly; SameSite=Strict; Max-Age=300",
                    //     peer_id
                    // );
                    // response_builder = response_builder.header("Set-Cookie", cookie);

                    if file_hash_clone.is_some() {
                        self.cache_file(
                            file_hash_clone.unwrap(),
                            decrypted.clone(),
                            mime_type.clone(),
                        );
                    }

                    Ok(response_builder
                        .body(Full::new(Bytes::from(decompressed)).boxed())
                        .unwrap())
                } else {
                    println!("[HTTP Proxy] Received invalid packet data");
                    return Ok(Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .body(full("Invalid packet data"))
                        .unwrap());
                }
            }
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .body(full("Proxy timeout"))
                .unwrap()),
        }
    }

    fn cache_file(&self, file_hash: String, content: Vec<u8>, mime_type: String) {
        println!("[HTTP PROXY] Caching file with hash: {}", file_hash);
        let cached_file = CachedFile {
            content,
            mime_type,
            expires_at: Instant::now() + Duration::from_secs(300),
        };
        self.file_cache.insert(file_hash, cached_file);
    }

    fn get_cached_file(&self, file_hash: &str) -> Option<(Vec<u8>, String)> {
        println!("[HTTP PROXY] Checking cache for file: {}", file_hash);
        if let Some(cached) = self.file_cache.get(file_hash) {
            if cached.expires_at > Instant::now() {
                println!("[HTTP PROXY] Cache hit for file: {}", file_hash);
                return Some((cached.content.clone(), cached.mime_type.clone()));
            } else {
                println!("[HTTP PROXY] Cache expired for file: {}", file_hash);
                self.file_cache.remove(file_hash);
            }
        } else {
            println!("[HTTP PROXY] Cache miss for file: {}", file_hash);
        }
        None
    }
}

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, Infallible> {
    Full::new(chunk.into()).map_err(|_| unreachable!()).boxed()
}
