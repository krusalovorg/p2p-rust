use reqwest;
use std::sync::Arc;

use crate::commands::create_base_commands;
use crate::connection::Connection;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{Protocol, ProxyMessage, TransportData, TransportPacket};

fn create_error_response(status: u16, message: &str) -> Vec<u8> {
    let status_text = match status {
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown Error",
    };

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Error {}</title>
    <style>
        body {{
            font-family: Arial, sans-serif;
            background-color: #f5f5f5;
            margin: 0;
            padding: 0;
            display: flex;
            justify-content: center;
            align-items: center;
            min-height: 100vh;
        }}
        .error-container {{
            background-color: white;
            padding: 2rem;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            text-align: center;
            max-width: 500px;
            width: 90%;
        }}
        .error-code {{
            color: #e74c3c;
            font-size: 2.5rem;
            margin: 0;
            margin-bottom: 1rem;
        }}
        .error-title {{
            color: #2c3e50;
            font-size: 1.5rem;
            margin: 0;
            margin-bottom: 1rem;
        }}
        .error-message {{
            color: #7f8c8d;
            font-size: 1rem;
            line-height: 1.5;
        }}
    </style>
</head>
<body>
    <div class="error-container">
        <h1 class="error-code">{}</h1>
        <h2 class="error-title">{}</h2>
        <p class="error-message">{}</p>
    </div>
</body>
</html>"#,
        status, status, status_text, message
    );

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Content-Type: text/html; charset=UTF-8\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         Pragma: no-cache\r\n\
         Expires: 0\r\n\
         X-Content-Type-Options: nosniff\r\n\
         X-Frame-Options: DENY\r\n\
         X-XSS-Protection: 1; mode=block\r\n\
         \r\n\
         {}",
        status,
        status_text,
        html.len(),
        html
    );
    response.into_bytes()
}

pub async fn handle_http_proxy_response(
    packet: TransportPacket,
    connection: &Connection,
    manager: Arc<ConnectionManager>,
    path_blobs: String,
) -> Result<(), String> {
    if let Some(TransportData::ProxyMessage(msg)) = packet.data {
        println!("[HTTP Proxy] Received encrypted request: {:?}", msg);

        let encrypted_request = match base64::decode(&msg.text) {
            Ok(data) => data,
            Err(e) => {
                let error_response = create_error_response(
                    400,
                    &format!("Failed to decode encrypted request: {}", e),
                );
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };

        let nonce = match base64::decode(&msg.nonce) {
            Ok(data) => data,
            Err(e) => {
                let error_response =
                    create_error_response(400, &format!("Failed to decode nonce: {}", e));
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };

        let nonce_array: [u8; 12] = match nonce.try_into() {
            Ok(arr) => arr,
            Err(_) => {
                let error_response = create_error_response(400, "Invalid nonce length");
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };

        println!("[HTTP Proxy] Public key: {}", &msg.from_peer_id);

        let request_bytes =
            match manager
                .db
                .decrypt_message(&encrypted_request, nonce_array, &msg.from_peer_id)
            {
                Ok(data) => data,
                Err(e) => {
                    let error_response =
                        create_error_response(500, &format!("Failed to decrypt message: {}", e));
                    return send_error_response(error_response, &msg, connection, manager).await;
                }
            };

        let request_str = String::from_utf8_lossy(&request_bytes);

        let mut lines = request_str.lines();
        let first_line = lines.next().unwrap_or("");
        let parts: Vec<&str> = first_line.split_whitespace().collect();
        // let method = if parts.len() > 0 { parts[0] } else { "GET" };
        let url = if parts.len() > 1 { parts[1] } else { "/" };

        // let client = match reqwest::Client::builder()
        //     .danger_accept_invalid_certs(true)
        //     .build()
        // {
        //     Ok(client) => client,
        //     Err(e) => {
        //         let error_response =
        //             create_error_response(500, &format!("Failed to create HTTP client: {}", e));
        //         return send_error_response(error_response, &msg, connection, manager).await;
        //     }
        // };

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
                println!("File not found");
                let error_response = create_error_response(404, "File not found");
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };
        let file_hash = first_fragment.file_hash.clone();
        let file_path = format!("{}/{}", path_blobs, file_hash);
        println!("FILE PATH: {}", file_path);
        
        if !std::path::Path::new(&file_path).exists() {
            let error_response = create_error_response(404, "File not found");
            return send_error_response(error_response, &msg, connection, manager).await;
        }

        let file_content = match std::fs::read(&file_path) {
            Ok(content) => content,
            Err(e) => {
                let error_response = create_error_response(500, &format!("Failed to read file: {}", e));
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };

        let mime_type = if let Some(fragment) = fragments.first() {
            fragment.mime.clone()
        } else {
            "application/octet-stream".to_string()
        };

        // Формируем HTTP-ответ
        let mut response = format!(
            "HTTP/1.1 200 OK\r\n\
             Content-Type: {}\r\n\
             Content-Length: {}\r\n\
             Cache-Control: no-cache\r\n\
             X-Content-Type-Options: nosniff\r\n\
             \r\n",
            mime_type,
            file_content.len()
        );

        let mut full_response = response.into_bytes();
        full_response.extend_from_slice(&file_content);

        // Шифруем ответ
        let (encrypted_response, nonce) = match manager
            .db
            .encrypt_message(&full_response, &msg.from_peer_id)
        {
            Ok(result) => result,
            Err(e) => {
                let error_response =
                    create_error_response(500, &format!("Failed to encrypt response: {}", e));
                return send_error_response(error_response, &msg, connection, manager).await;
            }
        };

        let response_packet = TransportPacket {
            act: "http_proxy_response".to_string(),
            to: Some(msg.from_peer_id.clone()),
            data: Some(TransportData::ProxyMessage(ProxyMessage {
                text: base64::encode(encrypted_response),
                nonce: base64::encode(nonce),
                from_peer_id: manager
                    .db
                    .get_or_create_peer_id()
                    .map_err(|e| format!("Failed to get peer ID: {}", e))?,
                end_peer_id: msg.from_peer_id.clone(),
                request_id: msg.request_id.clone(),
            })),
            protocol: Protocol::TURN,
            uuid: manager
                .db
                .get_or_create_peer_id()
                .map_err(|e| format!("Failed to get peer ID: {}", e))?,
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

async fn send_error_response(
    error_response: Vec<u8>,
    original_msg: &ProxyMessage,
    connection: &Connection,
    manager: Arc<ConnectionManager>,
) -> Result<(), String> {
    let base64_response = base64::encode(error_response);
    let (encrypted_response, nonce) = manager
        .db
        .encrypt_message(base64_response.as_bytes(), &original_msg.from_peer_id)
        .map_err(|e| format!("Failed to encrypt error response: {}", e))?;

    let response_packet = TransportPacket {
        act: "http_proxy_response".to_string(),
        to: Some(original_msg.from_peer_id.clone()),
        data: Some(TransportData::ProxyMessage(ProxyMessage {
            text: base64::encode(encrypted_response),
            nonce: base64::encode(nonce),
            from_peer_id: manager
                .db
                .get_or_create_peer_id()
                .map_err(|e| format!("Failed to get peer ID: {}", e))?,
            end_peer_id: original_msg.from_peer_id.clone(),
            request_id: original_msg.request_id.clone(),
        })),
        protocol: Protocol::TURN,
        uuid: manager
            .db
            .get_or_create_peer_id()
            .map_err(|e| format!("Failed to get peer ID: {}", e))?,
        nodes: vec![],
    };

    connection
        .send_packet(response_packet)
        .await
        .map_err(|e| e.to_string())
}
