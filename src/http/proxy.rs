use std::sync::Arc;
use std::io::{self, Read};
use flate2::read::GzEncoder;
use flate2::Compression;

use crate::connection::Connection;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{Protocol, ProxyMessage, TransportData, TransportPacket};

pub async fn handle_http_proxy_response(
    packet: TransportPacket,
    connection: &Connection,
    manager: Arc<ConnectionManager>,
    path_blobs: String,
) -> Result<(), String> {
    if let Some(TransportData::ProxyMessage(msg)) = packet.data {
        // println!("[HTTP Proxy] Received encrypted request: {:?}", msg);

        let encrypted_request = match base64::decode(&msg.text) {
            Ok(data) => data,
            Err(e) => {
                return Err(format!("Failed to decode encrypted request: {}", e));
            }
        };

        let nonce = match base64::decode(&msg.nonce.clone()) {
            Ok(data) => data,
            Err(e) => {
                return Err(format!("Failed to decode nonce: {}", e));
            }
        };

        let nonce = nonce.clone();
        let nonce_array: [u8; 12] = match nonce.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                return Err("Invalid nonce length".to_string());
            }
        };

        println!("[HTTP Proxy] Public key: {}", &msg.from_peer_id);

        let request_bytes = match manager
            .db
            .decrypt_message(&encrypted_request, nonce_array, &msg.from_peer_id)
        {
            Ok(data) => data,
            Err(e) => {
                return Err(format!("Failed to decrypt message: {}", e));
            }
        };

        let request_str = String::from_utf8_lossy(&request_bytes);

        let mut lines = request_str.lines();
        let first_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        let url = if parts.len() > 1 { parts[1] } else { "/" };

        println!("PATH FILE: {}", url);
        let url_without_my_id = url.replace(&format!("/{}", manager.db.get_or_create_peer_id().unwrap()), "");
        let url_without_my_id = if url_without_my_id.starts_with('/') {
            url_without_my_id.trim_start_matches('/').to_string()
        } else {
            url_without_my_id
        };
        println!("URL WITHOUT MY ID: {}", url_without_my_id);
        let search_result = manager.db.search_fragment_in_virtual_storage(&url_without_my_id, Some(true));
        let fragments = search_result.unwrap();
        let first_fragment = match fragments.first() {
            Some(fragment) => fragment,
            None => {
                return Err("File not found".to_string());
            }
        };
        let file_hash = first_fragment.file_hash.clone();
        let file_path = format!("{}/{}", path_blobs, file_hash);
        println!("FILE PATH: {}", file_path);
        
        if !std::path::Path::new(&file_path).exists() {
            return Err("File not found".to_string());
        }

        let mut file_content = match std::fs::read(&file_path) {
            Ok(content) => content,
            Err(e) => {
                return Err(format!("Failed to read file: {}", e));
            }
        };

        let mime_type = if let Some(fragment) = fragments.first() {
            fragment.mime.clone()
        } else {
            "application/octet-stream".to_string()
        };

        let mut encoder = GzEncoder::new(&file_content[..], Compression::best());
        let mut compressed = Vec::new();
        encoder.read_to_end(&mut compressed).unwrap();

        let (encrypted_response, nonce) = manager
            .db
            .encrypt_message(&compressed, &msg.from_peer_id)
            .map_err(|e| format!("Failed to encrypt message: {}", e))?;

        let response_packet = TransportPacket {
            act: "http_proxy_response".to_string(),
            to: Some(msg.from_peer_id.clone()),
            data: Some(TransportData::ProxyMessage(ProxyMessage {
                text: base64::encode(encrypted_response),
                nonce: base64::encode(&nonce),
                mime: mime_type,
                from_peer_id: manager
                    .db
                    .get_or_create_peer_id()
                    .map_err(|e| format!("Failed to get peer ID: {}", e))?,
                end_peer_id: msg.from_peer_id.clone(),
                request_id: msg.request_id.clone(),
            })),
            protocol: Protocol::TURN,
            peer_key: manager
                .db
                .get_or_create_peer_id()
                .map_err(|e| format!("Failed to get peer ID: {}", e))?,
            uuid: manager
                .db
                .generate_uuid()
                .map_err(|e| format!("Failed to generate UUID: {}", e))?,
            nodes: vec![],
        };

        connection
            .send_packet(response_packet)
            .await
            .map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}