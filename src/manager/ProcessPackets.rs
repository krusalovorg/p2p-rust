use super::ConnectionManager::ConnectionManager;
use crate::http::proxy::{handle_http_proxy_request_peer, handle_http_proxy_response};
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::{Protocol, StorageToken, TransportData, TransportPacket};
use crate::peer::turn_tunnel;
use colored::Colorize;
use hex;
use serde_json;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

impl ConnectionManager {
    pub async fn handle_incoming_packets(&self) {
        let incoming_packet_rx = self.incoming_packet_rx.clone();
        let mut rx = incoming_packet_rx.lock().await;
        println!("[Peer] Starting to handle incoming packets...");
        loop {
            if let Some((connection_type, packet, connection)) = rx.recv().await {
                match connection_type {
                    ConnectionType::Signal(id) => {
                        if let Some(connection) = connection {
                            println!("[DEBUG] Received signal packet: {:?}", packet);
                            let from_uuid = packet.uuid.clone();
                            let packet_clone = packet.clone();
                            let protocol_connection = packet.protocol.clone();

                            if let Some(data) = &packet.data {
                                match data {
                                    TransportData::StorageReservationRequest(request) => {
                                        if let Err(e) = self
                                            .handle_storage_reservation_request(
                                                request.clone(),
                                                &connection,
                                            )
                                            .await
                                        {
                                            println!("[Peer] Failed to handle storage reservation request: {}", e);
                                        }
                                    }
                                    TransportData::StorageValidTokenRequest(token) => {
                                        if let Err(e) = self
                                            .handle_storage_valid_token_request(
                                                token.token.clone(),
                                                &connection,
                                                from_uuid.clone(),
                                            )
                                            .await
                                        {
                                            println!("[Peer] Failed to handle storage valid token request: {}", e);
                                        }
                                    }
                                    TransportData::StorageValidTokenResponse(response) => {
                                        println!("\n╔════════════════════════════════════════════════════════════╗");
                                        println!("║                    ВАЛИДАЦИЯ ТОКЕНА ХРАНИЛИЩА                  ║");
                                        println!("╠════════════════════════════════════════════════════════════╣");
                                        println!(
                                            "║ Статус: {} ║",
                                            if response.status {
                                                "✅ ТОКЕН ВАЛИДЕН"
                                            } else {
                                                "❌ ТОКЕН НЕВАЛИДЕН"
                                            }
                                        );
                                        println!("╚════════════════════════════════════════════════════════════╝\n");
                                    }
                                    TransportData::PeerSearchResponse(response) => {
                                        println!("\n╔════════════════════════════════════════════════════════════╗");
                                        println!("║                      РЕЗУЛЬТАТЫ ПОИСКА ПИРА                    ║");
                                        println!("╠════════════════════════════════════════════════════════════╣");
                                        println!(
                                            "║ {} ║",
                                            format!("Статус: {}", "✅ ПИР НАЙДЕН").yellow()
                                        );
                                        println!(
                                            "║ {} ║",
                                            format!("UUID пира: {}", response.peer_id).cyan()
                                        );
                                        println!(
                                            "║ {} ║",
                                            format!(
                                                "Адрес ноды: {}:{}",
                                                response.public_ip, response.public_port
                                            )
                                            .cyan()
                                        );
                                        println!(
                                            "║ {} ║",
                                            format!("Прыжков: {}", response.hops).cyan()
                                        );
                                        println!("╚════════════════════════════════════════════════════════════╝\n");
                                    }
                                    TransportData::StorageReservationResponse(response) => {
                                        println!("\n{}", "=".repeat(80).yellow());
                                        println!("{}", "ВНИМАНИЕ! ВЫ ПОЛУЧИЛИ УНИКАЛЬНЫЙ ТОКЕН ДЛЯ ХРАНЕНИЯ И ПОЛУЧЕНИЯ ДАННЫХ С P2P ПИРА".red().bold());
                                        println!("{}", "ЕСЛИ ВЫ ПОТЕРЯЕТЕ КЛЮЧ ВЫ НЕ СМОЖЕТЕ ПОЛУЧИТЬ ДОСТУП К ДАННЫМ".red().bold());
                                        println!("{}", "=".repeat(80).yellow());

                                        if let Ok(token_bytes) = base64::decode(&response.token) {
                                            if let Ok(token_str) = String::from_utf8(token_bytes) {
                                                if let Ok(token) =
                                                    serde_json::from_str::<StorageToken>(&token_str)
                                                {
                                                    println!(
                                                        "\n{}",
                                                        "ДЕТАЛИ ТОКЕНА:".cyan().bold()
                                                    );
                                                    println!(
                                                        "{} {}",
                                                        "Размер файла:".yellow(),
                                                        format!("{} байт", token.file_size).white()
                                                    );
                                                    println!(
                                                        "{} {}",
                                                        "Провайдер хранилища:".yellow(),
                                                        token.storage_provider.white()
                                                    );
                                                    println!(
                                                        "{} {}",
                                                        "Временная метка:".yellow(),
                                                        format!("{}", token.timestamp).white()
                                                    );
                                                    println!(
                                                        "{} {}",
                                                        "Подпись:".yellow(),
                                                        hex::encode(&token.signature).white()
                                                    );
                                                }
                                                let path = format!(
                                                    "{}/tokens/{}.token",
                                                    self.db.path.as_str(),
                                                    response.peer_id.clone().to_string()
                                                );
                                                let tokens_dir =
                                                    format!("{}/tokens", self.db.path.as_str());
                                                if !std::path::Path::new(&tokens_dir).exists() {
                                                    tokio::fs::create_dir_all(&tokens_dir)
                                                        .await
                                                        .unwrap();
                                                }
                                                let mut file = File::create(path).await.unwrap();
                                                file.write_all(token_str.as_bytes()).await.unwrap();
                                            }
                                        }

                                        println!("\n{}", "=".repeat(80).yellow());
                                        println!("{}", "ТОКЕН В BASE64:".cyan().bold());
                                        println!("{}", response.token.white());
                                        println!("{}", "=".repeat(80).yellow());
                                    }
                                    _ => {}
                                }
                            }

                            if packet.act == "http_proxy_request" {
                                println!("[Peer] Received http proxy request");
                                handle_http_proxy_response(
                                    packet.clone(),
                                    &connection,
                                    Arc::new(self.clone()),
                                )
                                .await;
                            } else if packet.act == "peer_list" {
                                if let Some(TransportData::SyncPeerInfoData(peer_info_data)) =
                                    packet.data
                                {
                                    println!("{}", "[Peer] Received peer list:".yellow());
                                    for peer in peer_info_data.peers {
                                        println!(
                                            "{}",
                                            format!("[Peer] Peer - UUID: {}", peer.uuid).cyan()
                                        );
                                    }
                                } else {
                                    println!("{}", "[Peer] Peer list data is missing.".red());
                                }
                            } else if protocol_connection == Protocol::STUN {
                                println!("[DEBUG] Processing STUN packet");
                                match packet.act.as_str() {
                                    "wait_connection" => {
                                        println!(
                                            "[DEBUG] Received wait_connection from {}",
                                            from_uuid
                                        );
                                        let result = self
                                            .send_wait_connection(
                                                packet.uuid.clone(),
                                                &connection,
                                                self.db.get_or_create_peer_id().unwrap(),
                                            )
                                            .await;

                                        if let Err(e) = result {
                                            println!(
                                                "[DEBUG] Failed to send wait_connection: {}",
                                                e
                                            );
                                        } else {
                                            println!("[DEBUG] Successfully sent wait_connection");
                                        }
                                    }
                                    "accept_connection" => {
                                        println!(
                                            "[DEBUG] Received accept_connection from {}",
                                            from_uuid
                                        );
                                        let result = self
                                            .receive_accept_connection(
                                                packet,
                                                self.db.get_or_create_peer_id().unwrap(),
                                            )
                                            .await;

                                        match result {
                                            Ok(_) => {
                                                println!(
                                                    "[DEBUG] Connection established successfully"
                                                );
                                                self.connections_turn.write().await.insert(
                                                    from_uuid.clone(),
                                                    ConnectionTurnStatus {
                                                        connected: true,
                                                        turn_connection: false,
                                                    },
                                                );
                                            }
                                            Err(e) => {
                                                println!(
                                                    "[DEBUG] Failed to establish connection: {}",
                                                    e
                                                );
                                                self.connections_turn.write().await.insert(
                                                    from_uuid.clone(),
                                                    ConnectionTurnStatus {
                                                        connected: false,
                                                        turn_connection: true,
                                                    },
                                                );
                                            }
                                        }
                                    }
                                    _ => {
                                        println!("[DEBUG] Unknown STUN act: {}", packet.act);
                                    }
                                }
                            } else if protocol_connection == Protocol::TURN
                                && packet.act == "wait_connection"
                            {
                                self.connections_turn.write().await.insert(
                                    from_uuid.clone(),
                                    ConnectionTurnStatus {
                                        connected: false,
                                        turn_connection: true,
                                    },
                                );
                            }

                            println!(
                                "{}",
                                format!("[Peer] From UUID: {}", from_uuid.clone()).yellow()
                            );
                            if let Some(status) =
                                self.connections_turn.write().await.get_mut(&from_uuid)
                            {
                                if status.turn_connection && !status.connected {
                                    let result_turn_tunnel =
                                        turn_tunnel(packet_clone, &connection, &self.db).await;
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    println!(
                                        "{}",
                                        format!(
                                            "[Peer] Result turn tunnel {:?}",
                                            result_turn_tunnel
                                        )
                                        .yellow()
                                    );
                                    match result_turn_tunnel {
                                        Ok(r) => {
                                            if r == "successful_connection" {
                                                println!(
                                                    "{}",
                                                    "[TURN] Connection established!".green()
                                                );
                                                status.connected = true;
                                                status.turn_connection = false;

                                                let packet_hello = TransportPacket {
                                                    act: "test_turn".to_string(),
                                                    to: Some(from_uuid.clone()),
                                                    data: None,
                                                    protocol: Protocol::TURN,
                                                    uuid: self.db.get_or_create_peer_id().unwrap(),
                                                    nodes: vec![],
                                                };
                                                println!(
                                                    "{}",
                                                    "[Peer] Sending accept connection".yellow()
                                                );
                                                let _ = connection.send_packet(packet_hello).await;
                                            } else if r == "send_wait_connection" {
                                                println!(
                                                    "{}",
                                                    "[Peer] Wait answer acceptation connection..."
                                                        .yellow()
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            status.connected = false;
                                            status.turn_connection = true;
                                            println!("{}", format!("[Peer] Fail: {}", e).red());
                                        }
                                    }
                                    println!("{}", "[Peer] Wait new packets...".yellow());
                                } else {
                                    let packet_file_clone = packet_clone.clone();
                                    match packet_clone.act.as_str() {
                                        "save_file" => {
                                            if let Some(TransportData::PeerUploadFile(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self
                                                    .handle_file_upload(
                                                        data,
                                                        &connection,
                                                        from_uuid.clone(),
                                                    )
                                                    .await
                                                {
                                                    println!(
                                                        "[Peer] Failed to handle file upload: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        "file_saved" => {
                                            if let Some(TransportData::PeerFileSaved(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self.handle_file_saved(data).await {
                                                    println!(
                                                        "[Peer] Failed to handle file saved: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        "get_file" => {
                                            if let Some(TransportData::PeerFileGet(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self
                                                    .handle_file_get(
                                                        data.session_key,
                                                        &connection,
                                                        from_uuid.clone(),
                                                    )
                                                    .await
                                                {
                                                    println!(
                                                        "[Peer] Failed to handle file get: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        "file" => {
                                            if let Some(TransportData::FileData(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self.handle_file_data(data).await {
                                                    println!(
                                                        "[Peer] Failed to handle file data: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        "message_response" => {
                                            if let Err(e) = self.handle_message_response().await {
                                                println!(
                                                    "[Peer] Failed to handle message response: {}",
                                                    e
                                                );
                                            }
                                        }
                                        "message" => {
                                            if let Some(TransportData::Message(data)) =
                                                packet_clone.data
                                            {
                                                if let Err(e) = self
                                                    .handle_message(
                                                        data,
                                                        &connection,
                                                        from_uuid.clone(),
                                                    )
                                                    .await
                                                {
                                                    println!(
                                                        "[Peer] Failed to handle message: {}",
                                                        e
                                                    );
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            } else {
                                println!("{}", "[Peer] [Turn] Connection not found".red());
                            }
                        }
                    }
                    ConnectionType::Stun => {
                        println!("[Peer] Received message from Tunnel: {:?}", packet);
                    }
                }
            } else {
                println!("[Peer] No messages received, sleeping...");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}
