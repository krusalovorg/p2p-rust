use reqwest;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;
use futures::future::join_all;

use crate::connection::Connection;
use crate::http::http_proxy::HttpProxy;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{Message, Protocol, ProxyMessage, TransportData, TransportPacket};
use crate::signal::SignalServer;

pub async fn handle_http_proxy_request_peer(
    request_data: Vec<u8>,
    target_peer_id: String,
    manager: Arc<ConnectionManager>,
) -> Vec<u8> {
    let base64_proxy_request = base64::encode(&request_data);
    let encrypted_proxy_request = manager
        .db
        .encrypt_message(base64_proxy_request.as_bytes(), &target_peer_id)
        .unwrap();

    let packet = TransportPacket {
        act: "http_proxy_request".to_string(),
        to: Some(target_peer_id.clone()),
        data: Some(TransportData::Message(Message {
            text: base64::encode(encrypted_proxy_request.0),
            nonce: Some(base64::encode(encrypted_proxy_request.1)),
        })),
        protocol: Protocol::TURN,
        uuid: manager.db.get_or_create_peer_id().unwrap(),
        nodes: vec![],
    };

    let conn = {
        let conn_map = manager.connections.lock().await;
        conn_map.get(&target_peer_id).cloned()
    };

    if let Some(conn) = conn {
        conn.send_packet(packet).await.unwrap();

        // Ждем ответа с таймаутом
        match tokio::time::timeout(tokio::time::Duration::from_secs(30), conn.get_response()).await {
            Ok(Ok(response_packet)) => {
                if let Some(TransportData::Message(msg)) = response_packet.data {
                    let encrypted_response = base64::decode(&msg.text).unwrap();
                    let nonce = base64::decode(&msg.nonce.unwrap()).unwrap();
                    let nonce_array: [u8; 12] = nonce.try_into().unwrap();
                    let decrypted_response = manager
                        .db
                        .decrypt_message(&encrypted_response, nonce_array, &target_peer_id)
                        .unwrap();
                    return base64::decode(&decrypted_response).unwrap();
                }
            }
            Ok(Err(_)) => println!("Failed to receive response"),
            Err(_) => println!("Timeout waiting for response"),
        }
    }
    vec![]
}

pub async fn start_http_proxy_peer(manager: Arc<ConnectionManager>) {
    let listener = TcpListener::bind("0.0.0.0:80").await.unwrap();
    println!("HTTP proxy listening on 0.0.0.0:80");

    let mut handles = Vec::new();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0; 8192];
        let n = socket.read_exact(&mut buffer).await.unwrap();
        let request_data = buffer[..n].to_vec();
        let request_str = String::from_utf8_lossy(&request_data);
        let target_peer_id = request_str
            .split("Host: ")
            .nth(1)
            .unwrap()
            .split("/")
            .nth(0)
            .unwrap()
            .to_string();

        let manager = manager.clone();
        let request_data = request_data.clone();

        let handle = tokio::spawn(async move {
            let response_bytes =
                handle_http_proxy_request_peer(request_data, target_peer_id, manager.clone()).await;
            let _ = socket.write_all(&response_bytes).await;
        });

        handles.push(handle);
        
        // Очищаем завершенные задачи
        handles.retain(|h| !h.is_finished());
    }
}

pub async fn handle_http_proxy_response(
    packet: TransportPacket,
    connection: &Connection,
    manager: Arc<ConnectionManager>,
) -> Result<(), String> {
    if let Some(TransportData::ProxyMessage(msg)) = packet.data {
        let encrypted_request = base64::decode(&msg.text).unwrap();
        let nonce = base64::decode(&msg.nonce).unwrap();
        let nonce_array: [u8; 12] = nonce.try_into().unwrap();
        let decrypted_request = manager
            .db
            .decrypt_message(&encrypted_request, nonce_array, &packet.uuid)
            .unwrap();
        let request_bytes = base64::decode(&decrypted_request).unwrap();
        let request_str = String::from_utf8_lossy(&request_bytes);

        let url_line = request_str.lines().next().unwrap_or("");
        let parts: Vec<&str> = url_line.split_whitespace().collect();
        let url = if parts.len() > 1 { parts[1] } else { "/" };

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();
            
        let resp = client.get(format!("http://localhost:4173{}", url))
            .send()
            .await
            .unwrap();
            
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.bytes().await.unwrap();

        // Формируем ответ с заголовками
        let mut response = format!("HTTP/1.1 {}\r\n", status);
        for (name, value) in headers.iter() {
            response.push_str(&format!("{}: {}\r\n", name, value.to_str().unwrap()));
        }
        response.push_str("\r\n");
        
        let mut full_response = response.into_bytes();
        full_response.extend_from_slice(&body);

        let base64_response = base64::encode(full_response);
        let (encrypted_response, nonce) = manager
            .db
            .encrypt_message(base64_response.as_bytes(), &packet.uuid)
            .unwrap();

        let response_packet = TransportPacket {
            act: "http_proxy_response".to_string(),
            to: Some(msg.from_peer_id.clone()),
            data: Some(TransportData::ProxyMessage(ProxyMessage {
                text: base64::encode(encrypted_response),
                nonce: base64::encode(nonce),
                from_peer_id: manager.db.get_or_create_peer_id().unwrap(),
                end_peer_id: msg.from_peer_id.clone(),
                request_id: msg.request_id.clone(),
            })),
            protocol: Protocol::TURN,
            uuid: manager.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        println!("Sending response to peer: {:?}", response_packet);

        connection
            .send_packet(response_packet)
            .await
            .map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
