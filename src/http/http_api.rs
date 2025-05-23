use crate::db::P2PDatabase;
use crate::packets::{
    FragmentSearchRequest, PeerFileGet, PeerFileUpdate, PeerUploadFile, Protocol, TransportData,
    TransportPacket,
};
use bytes::Bytes;
use chrono;
use colored::Colorize;
use dashmap::DashMap;
use hex;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{body::Incoming, Request, Response};
use hyper_util::rt::tokio::TokioIo;
use mime_guess;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Mutex;
use std::{convert::Infallible, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;
use super::api::{full, log, FileAccessRequest, FileCache, UpdateRequest, UploadRequest};


#[derive(Clone, Debug)]
pub struct HttpApi {
    db: Arc<P2PDatabase>,
    api_tx: mpsc::Sender<TransportPacket>,
    pending_responses: Arc<DashMap<String, oneshot::Sender<TransportPacket>>>,
    fragment_cache: Arc<DashMap<String, String>>,
    file_cache: Arc<FileCache>,
    port: Arc<Mutex<u16>>,
    public_ip: String,
    path_blobs: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PacketRequest {
    to: Option<String>,
    data: Option<TransportData>,
    protocol: Protocol,
}

impl HttpApi {
    pub async fn new(
        db: Arc<P2PDatabase>,
        public_ip: String,
        api_tx: mpsc::Sender<TransportPacket>,
        path_blobs: String,
    ) -> Self {
        Self {
            db,
            api_tx,
            pending_responses: Arc::new(DashMap::new()),
            fragment_cache: Arc::new(DashMap::new()),
            file_cache: Arc::new(FileCache::new()),
            port: Arc::new(Mutex::new(8080)),
            public_ip,
            path_blobs,
        }
    }

    pub fn get_file_url(&self, file_hash: &str) -> String {
        format!(
            "http://{}:{}/{}",
            self.get_public_ip(),
            *self.port.lock().unwrap(),
            file_hash
        )
    }

    fn get_public_ip(&self) -> String {
        self.public_ip.clone()
    }

    pub async fn set_response(&self, request_id: String, response: TransportPacket) {
        log(&format!(
            "[HTTP API] Set response for request: {}",
            request_id
        ));
        if let Some((_, sender)) = self.pending_responses.remove(&request_id) {
            let _ = sender.send(response);
        }
    }

    pub async fn start(self: Arc<Self>) {
        let mut port = 8081;
        let mut listener = None;

        while listener.is_none() {
            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            match TcpListener::bind(addr).await {
                Ok(l) => {
                    listener = Some(l);
                    log(&format!("[HTTP API] Listening on http://{}", addr).green());
                }
                Err(_) => {
                    log(&format!(
                        "[HTTP API] Port {} is busy, trying {}",
                        port,
                        port + 1
                    ));
                    port += 1;
                }
            }
        }

        let listener = listener.unwrap();
        let port = listener.local_addr().unwrap().port();
        *self.port.lock().unwrap() = port;

        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let api = self.clone();

            tokio::spawn(async move {
                let service = service_fn(move |req: Request<Incoming>| {
                    let api = api.clone();
                    async move { api.handle(req).await }
                });

                if let Err(err) = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    eprintln!("[HTTP API] Connection error: {:?}", err);
                }
            });
        }
    }

    async fn handle(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let path = req.uri().path();
        let method = req.method();

        // Обработка CORS preflight запросов
        if method == &hyper::Method::OPTIONS {
            return Ok(Response::builder()
                .status(hyper::StatusCode::OK)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(""))
                .unwrap());
        }

        match (method, path) {
            (&hyper::Method::GET, "/api/info") => self.handle_node_info(req).await,
            (&hyper::Method::POST, "/api/packet") => self.handle_packet(req).await,
            (&hyper::Method::GET, path) if path.starts_with("/api/file/") => {
                self.handle_get_file(req).await
            }
            (&hyper::Method::GET, "/api/files") => self.handle_list_files(req).await,
            (&hyper::Method::POST, "/api/upload") => self.handle_upload_file(req).await,
            (&hyper::Method::POST, "/api/update") => self.handle_update_file(req).await,
            (&hyper::Method::DELETE, path) if path.starts_with("/api/file/") => {
                self.handle_delete_file(req).await
            }
            (&hyper::Method::PUT, path)
                if path.starts_with("/api/file/") && path.ends_with("/access") =>
            {
                self.handle_change_file_access(req).await
            }
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::NOT_FOUND)
                .body(full("Not Found"))
                .unwrap()),
        }
    }

    async fn handle_node_info(
        &self,
        _req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let peer_id = self.db.get_or_create_peer_id().unwrap();

        #[derive(Serialize)]
        struct NodeInfo {
            node_id: String,
            host_type: String,
            status: String,
            connection_type: String,
            last_switch: String,
        }

        let info = NodeInfo {
            node_id: peer_id,
            host_type: "Peer Node".to_string(),
            status: "ONLINE".to_string(),
            connection_type: "P2P".to_string(),
            last_switch: chrono::Local::now().format("%H:%M:%S").to_string(),
        };

        let response_json = serde_json::to_string(&info).unwrap();
        Ok(Response::builder()
            .status(hyper::StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type")
            .body(full(response_json))
            .unwrap())
    }

    async fn handle_packet(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let whole_body = req.collect().await?.to_bytes();
        let packet_request: PacketRequest = match serde_json::from_slice(&whole_body) {
            Ok(p) => p,
            Err(e) => {
                return Ok(Response::builder()
                    .status(hyper::StatusCode::BAD_REQUEST)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(format!("Invalid request format: {}", e)))
                    .unwrap());
            }
        };

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let packet = TransportPacket {
            act: "api_request".to_string(),
            to: packet_request.to,
            data: packet_request.data,
            protocol: packet_request.protocol,
            peer_key: self.db.get_or_create_peer_id().unwrap(),
            uuid: request_id.clone(),
            nodes: vec![],
        };

        if let Err(e) = self.api_tx.send(packet).await {
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                let response_json = serde_json::to_string(&response).unwrap();
                Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(response_json))
                    .unwrap())
            }
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("Request timeout"))
                .unwrap()),
        }
    }

    async fn handle_get_file(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let path = req.uri().path();
        let file_hash = path.strip_prefix("/api/file/").unwrap_or("");
        log(&format!(
            "[HTTP API] [DEBUG] Handling get file request for hash: {}",
            file_hash
        ));

        if file_hash.is_empty() {
            log(&format!("[HTTP API] [DEBUG] Empty file hash received"));
            return Ok(Response::builder()
                .status(hyper::StatusCode::BAD_REQUEST)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("File hash is required"))
                .unwrap());
        }

        let request_id = Uuid::new_v4().to_string();
        let mut indentificator = file_hash.to_string(); // file hash
        let mut storage_peer_id = None;
        let mut this_peer_storage_file = false;
        let mut file_hash = None;
        let mut mime = None;

        if indentificator.len() == 64 {
            log(&format!(
                "[HTTP API] [DEBUG] Searching for fragments with hash length 64"
            ));

            if let Some(cached_owner) = self.fragment_cache.get(&indentificator) {
                log(&format!(
                    "[HTTP API] [DEBUG] Found cached owner for hash: {}",
                    cached_owner.clone()
                ));
                storage_peer_id = Some(cached_owner.clone());
            } else {
                log(&format!(
                    "[HTTP API] [DEBUG] No cached owner found, searching in local storage"
                ));
                if let Ok(fragments) = self
                    .db
                    .search_fragment_in_virtual_storage(&indentificator, None)
                {
                    if let Some(fragment) = fragments.first() {
                        log(&format!(
                            "[HTTP API] [DEBUG] Found fragment in local storage: {:?}",
                            fragment
                        ));
                        self.fragment_cache
                            .insert(indentificator.clone(), fragment.storage_peer_key.clone());
                        storage_peer_id = Some(fragment.storage_peer_key.clone());
                        this_peer_storage_file =
                            fragment.storage_peer_key == self.db.get_or_create_peer_id().unwrap();
                        file_hash = Some(fragment.file_hash.clone());
                        mime = Some(fragment.mime.clone());
                        log(&format!(
                            "[HTTP API] [DEBUG] This peer storage file: {}",
                            this_peer_storage_file
                        ));
                    } else {
                        log(&format!("[HTTP API] [DEBUG] No fragment found in local storage, initiating network search"));
                        let search_packet = TransportPacket {
                            act: "search_fragments".to_string(),
                            to: None,
                            data: Some(TransportData::FragmentSearchRequest(
                                FragmentSearchRequest {
                                    query: indentificator.clone(),
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

                        if let Err(e) = self.api_tx.send(search_packet).await {
                            log(&format!(
                                "[HTTP API] [DEBUG] Failed to send search packet: {}",
                                e
                            ));
                            return Ok(Response::builder()
                                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                                .body(full(format!("Failed to send search packet: {}", e)))
                                .unwrap());
                        }

                        log(&format!("[HTTP API] [DEBUG] Waiting for search response"));
                        if let Ok(search_response) =
                            tokio::time::timeout(tokio::time::Duration::from_secs(5), search_rx)
                                .await
                        {
                            if let Ok(response_data) = search_response {
                                log(&format!(
                                    "[HTTP API] [DEBUG] Search response received: {:?}",
                                    response_data
                                ));
                                if let Some(TransportData::FragmentSearchResponse(response)) =
                                    response_data.data
                                {
                                    for fragment in response.fragments {
                                        if fragment.file_hash == indentificator {
                                            log(&format!("[HTTP API] [DEBUG] Found matching fragment in network search"));
                                            self.fragment_cache.insert(
                                                indentificator.clone(),
                                                fragment.storage_peer_key.clone(),
                                            );
                                            storage_peer_id =
                                                Some(fragment.storage_peer_key.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        } else {
                            log(&format!("[HTTP API] [DEBUG] Search response timeout"));
                        }
                    }
                }
            }
        }
        let file_hash_clone = file_hash.clone();
        if file_hash.is_some() {
            let file_hash_str = file_hash.unwrap();
            log(&format!(
                "[HTTP API] [DEBUG] Checking file cache for hash: {}",
                file_hash_str
            ));
            if let Some((cached_content, cached_mime)) = self.file_cache.get_cached_file(&file_hash_str) {
                log(&format!(
                    "[HTTP API] [DEBUG] Serving file from cache: {}",
                    file_hash_str
                ));
                return Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", cached_mime)
                    .header("Content-Length", cached_content.len().to_string())
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(Full::new(Bytes::from(cached_content)).boxed())
                    .unwrap());
            }

            if this_peer_storage_file {
                let file_hash_str = file_hash_clone.unwrap();
                log(&format!(
                    "[HTTP API] [DEBUG] File is stored on this peer, reading from local storage"
                ));

                let file_path = format!("{}/{}", self.path_blobs, file_hash_str);
                log(&format!("[HTTP API] [DEBUG] File path: {}", file_path));

                if !std::path::Path::new(&file_path).exists() {
                    log(&format!(
                        "[HTTP API] [DEBUG] File not found at path: {}",
                        file_path
                    ));
                    return Ok(Response::builder()
                        .status(hyper::StatusCode::NOT_FOUND)
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                        .header("Access-Control-Allow-Headers", "Content-Type")
                        .body(full("File not found"))
                        .unwrap());
                }

                let file_content = std::fs::read(&file_path).unwrap();
                let mime_type = mime.unwrap_or("application/octet-stream".to_string());
                log(&format!(
                    "[HTTP API] [DEBUG] File read successfully, size: {} bytes, mime: {}",
                    file_content.len(),
                    mime_type
                ));

                return Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", mime_type)
                    .header("Content-Length", file_content.len().to_string())
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(Full::new(Bytes::from(file_content)).boxed())
                    .unwrap());
            }
        }

        log(&format!(
            "[HTTP API] [DEBUG] Initiating file request from network"
        ));
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let packet = TransportPacket {
            act: "get_file".to_string(),
            to: storage_peer_id.clone(),
            data: Some(TransportData::PeerFileGet(PeerFileGet {
                token: None,
                peer_id: self.db.get_or_create_peer_id().unwrap(),
                file_hash: file_hash_clone.clone().unwrap_or(indentificator.clone()),
            })),
            protocol: Protocol::TURN,
            peer_key: self.db.get_or_create_peer_id().unwrap(),
            uuid: request_id.clone(),
            nodes: vec![],
        };
        log(&format!(
            "[HTTP API] [DEBUG] Sending get_file packet: {:?}",
            packet
        ));

        if self.api_tx.is_closed() {
            log(&format!(
                "[HTTP API] [DEBUG] Channel is closed, attempting to reconnect..."
            ));
            return Ok(Response::builder()
                .status(hyper::StatusCode::SERVICE_UNAVAILABLE)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("Service temporarily unavailable"))
                .unwrap());
        }

        if let Err(e) = self.api_tx.send(packet).await {
            log(&format!(
                "[HTTP API] [DEBUG] Failed to send get_file packet: {}",
                e
            ));
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        log(&format!("[HTTP API] [DEBUG] Waiting for file response"));
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(response)) => {
                log(&format!(
                    "[HTTP API] [DEBUG] File response received: {:?}",
                    response
                ));
                if let Some(TransportData::FileData(file_data)) = response.data {
                    let file_content = base64::decode(&file_data.contents).unwrap();
                    self.file_cache.cache_file(
                        file_hash_clone.clone().unwrap_or(indentificator.clone()),
                        file_content.clone(),
                        file_data.mime.clone(),
                    );
                    Ok(Response::builder()
                        .status(hyper::StatusCode::OK)
                        .header("Content-Type", &file_data.mime)
                        .header("Content-Length", file_content.len().to_string())
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                        .header("Access-Control-Allow-Headers", "Content-Type")
                        .body(full(file_content))
                        .unwrap())
                } else {
                    log(&format!("[HTTP API] [DEBUG] Invalid response format"));
                    Ok(Response::builder()
                        .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                        .header("Access-Control-Allow-Origin", "*")
                        .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                        .header("Access-Control-Allow-Headers", "Content-Type")
                        .body(full("Invalid response format"))
                        .unwrap())
                }
            }
            _ => {
                log(&format!("[HTTP API] [DEBUG] File request timeout"));
                Ok(Response::builder()
                    .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full("Request timeout"))
                    .unwrap())
            }
        }
    }

    async fn handle_list_files(
        &self,
        _req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let files = match self.db.get_storage_fragments() {
            Ok(f) => f,
            Err(e) => {
                return Ok(Response::builder()
                    .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(format!("Failed to get files: {}", e)))
                    .unwrap());
            }
        };

        let response_json = serde_json::to_string(&files).unwrap();
        Ok(Response::builder()
            .status(hyper::StatusCode::OK)
            .header("Content-Type", "application/json")
            .header("Access-Control-Allow-Origin", "*")
            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
            .header("Access-Control-Allow-Headers", "Content-Type")
            .body(full(response_json))
            .unwrap())
    }

    async fn handle_upload_file(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let (filename, contents, public, encrypted, compressed, auto_decompress, token) =
            if content_type.starts_with("multipart/form-data") {
                let boundary = match content_type.split("boundary=").nth(1) {
                    Some(b) => b,
                    None => {
                        return Ok(Response::builder()
                            .status(hyper::StatusCode::BAD_REQUEST)
                            .body(full("Missing boundary in multipart request"))
                            .unwrap());
                    }
                };

                let boundary_bytes = format!("--{}", boundary).into_bytes();
                let whole_body = req.collect().await?.to_bytes();
                let mut filename = String::new();
                let mut contents = String::new();
                let mut public = true;
                let mut encrypted = false;
                let mut compressed = false;
                let mut auto_decompress = false;
                let mut token = String::new();

                let parts = whole_body.split_inclusive(|b| boundary_bytes.contains(b));
                for part in parts {
                    if part.is_empty() || part == b"--\r\n" {
                        continue;
                    }

                    let mut headers = Vec::new();
                    let mut content = Vec::new();
                    let mut is_content = false;

                    for line in part.split(|&b| b == b'\n') {
                        if line == b"\r" {
                            is_content = true;
                            continue;
                        }
                        if !is_content {
                            headers.push(line);
                        } else {
                            content.extend_from_slice(line);
                        }
                    }

                    let content_disposition = headers
                        .iter()
                        .find(|h| h.starts_with(b"Content-Disposition:"))
                        .and_then(|h| std::str::from_utf8(h).ok())
                        .unwrap_or("");

                    if content_disposition.contains("name=\"file\"") {
                        let filename_start =
                            content_disposition.find("filename=\"").unwrap_or(0) + 10;
                        let filename_end = content_disposition[filename_start..]
                            .find("\"")
                            .unwrap_or(0)
                            + filename_start;
                        filename = content_disposition[filename_start..filename_end].to_string();
                        contents = base64::encode(&content);
                    } else if content_disposition.contains("name=\"public\"") {
                        public = std::str::from_utf8(&content).unwrap_or("true").trim() == "true";
                    } else if content_disposition.contains("name=\"encrypted\"") {
                        encrypted =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"compressed\"") {
                        compressed =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"auto_decompress\"") {
                        auto_decompress =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"token\"") {
                        token = std::str::from_utf8(&content)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                    }
                }

                (
                    filename,
                    contents,
                    public,
                    encrypted,
                    compressed,
                    auto_decompress,
                    token,
                )
            } else {
                let whole_body = req.collect().await?.to_bytes();
                let upload_request: UploadRequest = match serde_json::from_slice(&whole_body) {
                    Ok(r) => r,
                    Err(e) => {
                        return Ok(Response::builder()
                            .status(hyper::StatusCode::BAD_REQUEST)
                            .header("Access-Control-Allow-Origin", "*")
                            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                            .header("Access-Control-Allow-Headers", "Content-Type")
                            .body(full(format!("Invalid request format: {}", e)))
                            .unwrap());
                    }
                };
                (
                    upload_request.filename,
                    upload_request.contents,
                    upload_request.public,
                    upload_request.encrypted,
                    upload_request.compressed,
                    upload_request.auto_decompress,
                    upload_request.token,
                )
            };

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let my_peer_id = self.db.get_or_create_peer_id().unwrap();
        let file_hash = hex::encode(Sha256::digest(contents.as_bytes()));
        let mime = mime_guess::from_path(&filename)
            .first_or_text_plain()
            .to_string();

        log(&format!(
            "[HTTP API] Processing file upload: filename={}, size={} bytes, mime={}, hash={}",
            filename,
            contents.len(),
            mime,
            file_hash
        ));

        let packet = TransportPacket {
            act: "save_file".to_string(),
            to: None,
            data: Some(TransportData::PeerUploadFile(PeerUploadFile {
                filename,
                contents: base64::decode(contents).unwrap(),
                peer_id: my_peer_id.clone(),
                token,
                file_hash: file_hash.clone(),
                mime,
                public,
                encrypted,
                compressed,
                auto_decompress,
            })),
            protocol: Protocol::TURN,
            peer_key: my_peer_id,
            uuid: request_id.clone(),
            nodes: vec![],
        };

        log(&format!(
            "[HTTP API] Sending save_file packet with request_id: {}",
            request_id
        ));

        if let Err(e) = self.api_tx.send(packet).await {
            log(&format!(
                "[HTTP API] Failed to send save_file packet: {}",
                e
            ));
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        log(&format!("[HTTP API] Waiting for save_file response..."));
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(_)) => {
                log(&format!(
                    "[HTTP API] File successfully saved with hash: {}",
                    file_hash
                ));
                Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(format!(
                        "{{\"status\":\"success\",\"file_hash\":\"{}\"}}",
                        file_hash
                    )))
                    .unwrap())
            }
            _ => {
                log(&format!("[HTTP API] Save file request timed out"));
                Ok(Response::builder()
                    .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full("Request timeout"))
                    .unwrap())
            }
        }
    }

    async fn handle_update_file(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let content_type = req
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        let (file_hash, contents, public, encrypted, compressed, auto_decompress, token) =
            if content_type.starts_with("multipart/form-data") {
                let boundary = match content_type.split("boundary=").nth(1) {
                    Some(b) => b,
                    None => {
                        return Ok(Response::builder()
                            .status(hyper::StatusCode::BAD_REQUEST)
                            .body(full("Missing boundary in multipart request"))
                            .unwrap());
                    }
                };

                let boundary_bytes = format!("--{}", boundary).into_bytes();
                let whole_body = req.collect().await?.to_bytes();
                let mut file_hash = String::new();
                let mut contents = Vec::new();
                let mut public = true;
                let mut encrypted = false;
                let mut compressed = false;
                let mut auto_decompress = false;
                let mut token = String::new();

                let parts = whole_body.split_inclusive(|b| boundary_bytes.contains(b));
                for part in parts {
                    if part.is_empty() || part == b"--\r\n" {
                        continue;
                    }

                    let mut headers = Vec::new();
                    let mut content = Vec::new();
                    let mut is_content = false;

                    for line in part.split(|&b| b == b'\n') {
                        if line == b"\r" {
                            is_content = true;
                            continue;
                        }
                        if !is_content {
                            headers.push(line);
                        } else {
                            content.extend_from_slice(line);
                        }
                    }

                    let content_disposition = headers
                        .iter()
                        .find(|h| h.starts_with(b"Content-Disposition:"))
                        .and_then(|h| std::str::from_utf8(h).ok())
                        .unwrap_or("");

                    if content_disposition.contains("name=\"file\"") {
                        contents = content;
                    } else if content_disposition.contains("name=\"file_hash\"") {
                        file_hash = std::str::from_utf8(&content)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                    } else if content_disposition.contains("name=\"public\"") {
                        public = std::str::from_utf8(&content).unwrap_or("true").trim() == "true";
                    } else if content_disposition.contains("name=\"encrypted\"") {
                        encrypted =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"compressed\"") {
                        compressed =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"auto_decompress\"") {
                        auto_decompress =
                            std::str::from_utf8(&content).unwrap_or("false").trim() == "true";
                    } else if content_disposition.contains("name=\"token\"") {
                        token = std::str::from_utf8(&content)
                            .unwrap_or("")
                            .trim()
                            .to_string();
                    }
                }

                (
                    file_hash,
                    contents,
                    public,
                    encrypted,
                    compressed,
                    auto_decompress,
                    token,
                )
            } else {
                let whole_body = req.collect().await?.to_bytes();
                let update_request: UpdateRequest = match serde_json::from_slice(&whole_body) {
                    Ok(r) => r,
                    Err(e) => {
                        return Ok(Response::builder()
                            .status(hyper::StatusCode::BAD_REQUEST)
                            .header("Access-Control-Allow-Origin", "*")
                            .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                            .header("Access-Control-Allow-Headers", "Content-Type")
                            .body(full(format!("Invalid request format: {}", e)))
                            .unwrap());
                    }
                };
                (
                    update_request.file_hash,
                    update_request.contents,
                    update_request.public,
                    update_request.encrypted,
                    update_request.compressed,
                    update_request.auto_decompress,
                    update_request.token,
                )
            };

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let my_peer_id = self.db.get_or_create_peer_id().unwrap();
        let new_file_hash = hex::encode(Sha256::digest(&contents));

        log(&format!(
            "[HTTP API] Processing file update: old_hash={}, new_hash={}, size={} bytes",
            file_hash,
            new_file_hash,
            contents.len()
        ));

        let packet = TransportPacket {
            act: "update_file".to_string(),
            to: None,
            data: Some(TransportData::PeerFileUpdate(PeerFileUpdate {
                peer_id: my_peer_id.clone(),
                file_hash: file_hash.clone(),
                filename: "".to_string(),
                contents,
                token,
                mime: "".to_string(),
                public,
                encrypted,
                compressed,
                auto_decompress,
            })),
            protocol: Protocol::TURN,
            peer_key: my_peer_id,
            uuid: request_id.clone(),
            nodes: vec![],
        };

        log(&format!(
            "[HTTP API] Sending update_file packet with request_id: {}",
            request_id
        ));

        if let Err(e) = self.api_tx.send(packet).await {
            log(&format!(
                "[HTTP API] Failed to send update_file packet: {}",
                e
            ));
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        log(&format!("[HTTP API] Waiting for update_file response..."));
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(_)) => {
                log(&format!(
                    "[HTTP API] File successfully updated with hash: {}",
                    file_hash
                ));
                Ok(Response::builder()
                    .status(hyper::StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(format!(
                        "{{\"status\":\"success\",\"file_hash\":\"{}\"}}",
                        file_hash
                    )))
                    .unwrap())
            }
            _ => {
                log(&format!("[HTTP API] Update file request timed out"));
                Ok(Response::builder()
                    .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full("Request timeout"))
                    .unwrap())
            }
        }
    }

    async fn handle_delete_file(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let path = req.uri().path();
        let file_hash = path.strip_prefix("/api/file/").unwrap_or("");

        if file_hash.is_empty() {
            return Ok(Response::builder()
                .status(hyper::StatusCode::BAD_REQUEST)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("File hash is required"))
                .unwrap());
        }

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let packet = TransportPacket {
            act: "delete_file".to_string(),
            to: None,
            data: Some(TransportData::FragmentSearchRequest(
                FragmentSearchRequest {
                    query: file_hash.to_string(),
                    request_id: request_id.clone(),
                },
            )),
            protocol: Protocol::TURN,
            peer_key: self.db.get_or_create_peer_id().unwrap(),
            uuid: request_id.clone(),
            nodes: vec![],
        };

        if let Err(e) = self.api_tx.send(packet).await {
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(_)) => Ok(Response::builder()
                .status(hyper::StatusCode::OK)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("File deleted successfully"))
                .unwrap()),
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("Request timeout"))
                .unwrap()),
        }
    }

    async fn handle_change_file_access(
        &self,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, Infallible>>, hyper::Error> {
        let path = req.uri().path().to_string();
        let whole_body = req.collect().await?.to_bytes();
        let file_hash = path
            .strip_prefix("/api/file/")
            .and_then(|p| p.strip_suffix("/access"))
            .unwrap_or("");

        if file_hash.is_empty() {
            return Ok(Response::builder()
                .status(hyper::StatusCode::BAD_REQUEST)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("File hash is required"))
                .unwrap());
        }

        let access_request: FileAccessRequest = match serde_json::from_slice(&whole_body) {
            Ok(r) => r,
            Err(e) => {
                return Ok(Response::builder()
                    .status(hyper::StatusCode::BAD_REQUEST)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                    .header("Access-Control-Allow-Headers", "Content-Type")
                    .body(full(format!("Invalid request format: {}", e)))
                    .unwrap());
            }
        };

        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_responses.insert(request_id.clone(), tx);

        let packet = TransportPacket {
            act: "change_file_access".to_string(),
            to: None,
            data: Some(TransportData::FragmentSearchRequest(
                FragmentSearchRequest {
                    query: file_hash.to_string(),
                    request_id: request_id.clone(),
                },
            )),
            protocol: Protocol::TURN,
            peer_key: self.db.get_or_create_peer_id().unwrap(),
            uuid: request_id.clone(),
            nodes: vec![],
        };

        if let Err(e) = self.api_tx.send(packet).await {
            return Ok(Response::builder()
                .status(hyper::StatusCode::INTERNAL_SERVER_ERROR)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full(format!("Failed to send packet: {}", e)))
                .unwrap());
        }

        match tokio::time::timeout(tokio::time::Duration::from_secs(30), rx).await {
            Ok(Ok(_)) => Ok(Response::builder()
                .status(hyper::StatusCode::OK)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("File access changed successfully"))
                .unwrap()),
            _ => Ok(Response::builder()
                .status(hyper::StatusCode::GATEWAY_TIMEOUT)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type")
                .body(full("Request timeout"))
                .unwrap()),
        }
    }
}