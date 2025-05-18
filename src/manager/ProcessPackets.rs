use super::ConnectionManager::ConnectionManager;
use crate::http::proxy::handle_http_proxy_response;
use crate::logger::LOGGER;
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::{Message, Protocol, StorageToken, TransportData, TransportPacket};
use crate::peer::turn_tunnel;
use colored::Colorize;
use hex;
use serde_json;
use std::sync::Arc;
use std::time::Duration;

impl ConnectionManager {
    pub async fn handle_incoming_packets(&self) {
        let incoming_packet_rx = self.incoming_packet_rx.clone();
        let mut rx = incoming_packet_rx.lock().await;
        LOGGER.debug("Starting to handle incoming packets...");
        loop {
            if let Some((connection_type, packet, connection)) = rx.recv().await {
                match connection_type {
                    ConnectionType::Signal(id) => {
                        if let Some(connection) = connection {
                            LOGGER.debug(&format!("Received signal packet: {:?}", packet));
                            let from_uuid = packet.uuid.clone();
                            let packet_clone = packet.clone();
                            let protocol_connection = packet.protocol.clone();

                            if let Some(data) = &packet.data {
                                match data {
                                    TransportData::ProxyMessage(data) => {
                                        self.proxy_http_tx_reciever.lock().await.send(packet.clone()).await;
                                    }
                                    TransportData::StorageReservationRequest(request) => {
                                        if let Err(e) = self
                                            .handle_storage_reservation_request(
                                                request.clone(),
                                                &connection,
                                            )
                                            .await
                                        {
                                            LOGGER.error(&format!(
                                                "Failed to handle storage reservation request: {}",
                                                e
                                            ));
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
                                            LOGGER.error(&format!(
                                                "Failed to handle storage valid token request: {}",
                                                e
                                            ));
                                        }
                                    }
                                    TransportData::PeerFileGet(data) => {
                                        if let Err(e) = self
                                            .handle_file_get(
                                                data.clone(),
                                                &connection,
                                                from_uuid.clone(),
                                            )
                                            .await
                                        {
                                            LOGGER.error(&format!(
                                                "Failed to handle file get: {}",
                                                e
                                            ));
                                        }
                                    }
                                    TransportData::FileData(data) => {
                                        let peer_id = data.peer_id.clone();
                                        if let Err(e) = self.handle_file_data(data.clone()).await {
                                            LOGGER.error(&format!(
                                                "Failed to handle file data: {}",
                                                e
                                            ));
                                        } else {
                                            if let Ok(free_space) =
                                                self.db.get_storage_free_space().await
                                            {
                                                if let Err(e) = self
                                                    .db
                                                    .update_token_free_space(&peer_id, free_space)
                                                {
                                                    LOGGER.error(&format!(
                                                        "Failed to update token free space: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                    }
                                    TransportData::StorageValidTokenResponse(response) => {
                                        LOGGER.storage("\n╔════════════════════════════════════════════════════════════╗");
                                        LOGGER.storage("║                    ВАЛИДАЦИЯ ТОКЕНА ХРАНИЛИЩА                  ║");
                                        LOGGER.storage("╠════════════════════════════════════════════════════════════╣");
                                        LOGGER.storage(&format!(
                                            "║ Статус: {} ║",
                                            if response.status {
                                                "✅ ТОКЕН ВАЛИДЕН"
                                            } else {
                                                "❌ ТОКЕН НЕВАЛИДЕН"
                                            }
                                        ));
                                        LOGGER.storage("╚════════════════════════════════════════════════════════════╝\n");
                                    }
                                    TransportData::PeerSearchResponse(response) => {
                                        LOGGER.peer("\n╔════════════════════════════════════════════════════════════╗");
                                        LOGGER.peer("║                      РЕЗУЛЬТАТЫ ПОИСКА ПИРА                    ║");
                                        LOGGER.peer("╠════════════════════════════════════════════════════════════╣");
                                        LOGGER.peer(&format!(
                                            "║ {} ║",
                                            format!("Статус: {}", "✅ ПИР НАЙДЕН").yellow()
                                        ));
                                        LOGGER.peer(&format!(
                                            "║ {} ║",
                                            format!("UUID пира: {}", response.peer_id).cyan()
                                        ));
                                        LOGGER.peer(&format!(
                                            "║ {} ║",
                                            format!(
                                                "Адрес ноды: {}:{}",
                                                response.public_ip, response.public_port
                                            )
                                            .cyan()
                                        ));
                                        LOGGER.peer(&format!(
                                            "║ {} ║",
                                            format!("Прыжков: {}", response.hops).cyan()
                                        ));
                                        LOGGER.peer("╚════════════════════════════════════════════════════════════╝\n");
                                    }
                                    TransportData::StorageReservationResponse(response) => {
                                        LOGGER.storage(&format!("\n{}", "=".repeat(80).yellow()));
                                        LOGGER.storage(&format!("{}", "ВНИМАНИЕ! ВЫ ПОЛУЧИЛИ УНИКАЛЬНЫЙ ТОКЕН ДЛЯ ХРАНЕНИЯ И ПОЛУЧЕНИЯ ДАННЫХ С P2P ПИРА".red().bold()));
                                        LOGGER.storage(&format!("{}", "ЕСЛИ ВЫ ПОТЕРЯЕТЕ КЛЮЧ ВЫ НЕ СМОЖЕТЕ ПОЛУЧИТЬ ДОСТУП К ДАННЫМ".red().bold()));
                                        LOGGER.storage(&format!("{}", "=".repeat(80).yellow()));

                                        if let Ok(token_bytes) = base64::decode(&response.token) {
                                            if let Ok(token_str) = String::from_utf8(token_bytes) {
                                                if let Ok(token) =
                                                    serde_json::from_str::<StorageToken>(&token_str)
                                                {
                                                    LOGGER.storage(&format!(
                                                        "\n{}",
                                                        "ДЕТАЛИ ТОКЕНА:".cyan().bold()
                                                    ));
                                                    LOGGER.storage(&format!(
                                                        "{} {}",
                                                        "Размер файла:".yellow(),
                                                        format!("{} байт", token.file_size).white()
                                                    ));
                                                    LOGGER.storage(&format!(
                                                        "{} {}",
                                                        "Провайдер хранилища:".yellow(),
                                                        token.storage_provider.white()
                                                    ));
                                                    LOGGER.storage(&format!(
                                                        "{} {}",
                                                        "Временная метка:".yellow(),
                                                        format!("{}", token.timestamp).white()
                                                    ));
                                                    LOGGER.storage(&format!(
                                                        "{} {}",
                                                        "Подпись:".yellow(),
                                                        hex::encode(&token.signature).white()
                                                    ));

                                                    if let Err(e) = self.db.add_token(
                                                        &response.peer_id,
                                                        &response.token,
                                                        token.file_size,
                                                    ) {
                                                        LOGGER.error(&format!(
                                                            "Failed to save token to database: {}",
                                                            e
                                                        ));
                                                    }
                                                }
                                            }
                                        }

                                        LOGGER.storage(&format!("\n{}", "=".repeat(80).yellow()));
                                        LOGGER.storage(&format!(
                                            "{}",
                                            "ТОКЕН В BASE64:".cyan().bold()
                                        ));
                                        LOGGER.storage(&format!("{}", response.token.white()));
                                        LOGGER.storage(&format!("{}", "=".repeat(80).yellow()));
                                    }
                                    _ => {}
                                }
                            }

                            if packet.act == "http_proxy_request" {
                                LOGGER.debug("Received http proxy request");
                                let connection = connection.clone();
                                let manager = Arc::new(self.clone());
                                let packet_clone = packet.clone();
                                let path_blobs = self.path_blobs.clone().to_string();
                                tokio::spawn(async move {
                                    let _ = handle_http_proxy_response(
                                        packet_clone,
                                        &connection,
                                        manager,
                                        path_blobs,
                                    )
                                    .await;
                                });
                            } else if packet.act == "peer_list" {
                                if let Some(TransportData::SyncPeerInfoData(peer_info_data)) =
                                    packet.data
                                {
                                    LOGGER.peer("Received peer list:");
                                    for peer in peer_info_data.peers {
                                        LOGGER.peer(&format!("Peer - UUID: {}", peer.uuid));
                                    }
                                } else {
                                    LOGGER.error("Peer list data is missing.");
                                }
                            } else if protocol_connection == Protocol::STUN {
                                LOGGER.debug("Processing STUN packet");
                                match packet.act.as_str() {
                                    "wait_connection" => {
                                        LOGGER.debug(&format!(
                                            "Received wait_connection from {}",
                                            from_uuid
                                        ));
                                        let result = async {
                                            self.send_wait_connection(
                                                packet.uuid.clone(),
                                                &connection,
                                                self.db.get_or_create_peer_id().unwrap(),
                                            )
                                            .await
                                        }
                                        .await;

                                        if let Err(e) = result {
                                            LOGGER.error(&format!(
                                                "Failed to send wait_connection: {}",
                                                e
                                            ));
                                        } else {
                                            LOGGER.debug("Successfully sent wait_connection");
                                        }
                                    }
                                    "accept_connection" => {
                                        LOGGER.debug(&format!(
                                            "Received accept_connection from {}",
                                            from_uuid
                                        ));
                                        let result = self
                                            .receive_accept_connection(
                                                packet,
                                                self.db.get_or_create_peer_id().unwrap(),
                                            )
                                            .await;

                                        match result {
                                            Ok(_) => {
                                                LOGGER.debug("Connection established successfully");
                                                self.connections_turn.write().await.insert(
                                                    from_uuid.clone(),
                                                    ConnectionTurnStatus {
                                                        connected: true,
                                                        turn_connection: false,
                                                    },
                                                );
                                            }
                                            Err(e) => {
                                                LOGGER.error(&format!(
                                                    "Failed to establish connection: {}",
                                                    e
                                                ));
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
                                        LOGGER.debug(&format!("Unknown STUN act: {}", packet.act));
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

                            LOGGER.debug(&format!("From UUID: {}", from_uuid.clone()));
                            if let Some(status) =
                                self.connections_turn.write().await.get_mut(&from_uuid)
                            {
                                if status.turn_connection && !status.connected {
                                    let result_turn_tunnel =
                                        turn_tunnel(packet_clone, &connection, &self.db).await;
                                    tokio::time::sleep(Duration::from_millis(100)).await;
                                    LOGGER.debug(&format!(
                                        "Result turn tunnel {:?}",
                                        result_turn_tunnel
                                    ));
                                    match result_turn_tunnel {
                                        Ok(r) => {
                                            if r == "successful_connection" {
                                                LOGGER.turn("Connection established!");
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
                                                LOGGER.debug("Sending accept connection");
                                                let _ = connection.send_packet(packet_hello).await;
                                            } else if r == "send_wait_connection" {
                                                LOGGER
                                                    .peer("Wait answer acceptation connection...");
                                            }
                                        }
                                        Err(e) => {
                                            status.connected = false;
                                            status.turn_connection = true;
                                            LOGGER.error(&format!("Fail: {}", e));
                                        }
                                    }
                                    LOGGER.debug("Wait new packets...");
                                } else {
                                    let packet_file_clone = packet_clone.clone();
                                    match packet_clone.act.as_str() {
                                        "save_file" => {
                                            if let Some(TransportData::PeerUploadFile(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self
                                                    .handle_file_upload(
                                                        &self.db,
                                                        data,
                                                        &connection,
                                                        from_uuid.clone(),
                                                    )
                                                    .await
                                                {
                                                    let formatted_error = format!(
                                                        "Failed to handle file upload: {}",
                                                        e
                                                    );
                                                    LOGGER.error(&formatted_error);

                                                    let packet_error = TransportPacket {
                                                        act: "message".to_string(),
                                                        to: Some(from_uuid.clone()),
                                                        data: Some(TransportData::Message(
                                                            Message {
                                                                text: formatted_error,
                                                                nonce: None,
                                                            },
                                                        )),
                                                        protocol: Protocol::TURN,
                                                        uuid: self
                                                            .db
                                                            .get_or_create_peer_id()
                                                            .unwrap(),
                                                        nodes: vec![],
                                                    };

                                                    let _ =
                                                        connection.send_packet(packet_error).await;
                                                }
                                            }
                                        }
                                        "file_saved" => {
                                            if let Some(TransportData::PeerFileSaved(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self.handle_file_saved(data).await {
                                                    println!("[Peer] Failed to handle file saved: {}", e);
                                                }
                                            }
                                        }
                                        "get_file" => {
                                            if let Some(TransportData::PeerFileGet(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self.handle_file_get(data, &connection, from_uuid.clone()).await {
                                                    println!("[Peer] Failed to handle file get: {}", e);
                                                }
                                            }
                                        }
                                        "move_file" => {
                                            if let Some(TransportData::PeerFileMove(data)) =
                                                packet_file_clone.data
                                            {
                                                if let Err(e) = self.handle_file_move(data, &connection, from_uuid.clone()).await {
                                                    println!("[Peer] Failed to handle file move: {}", e);
                                                }
                                            }
                                        }
                                        "message_response" => {
                                            if let Err(e) = self.handle_message_response().await {
                                                LOGGER.error(&format!(
                                                    "Failed to handle message response: {}",
                                                    e
                                                ));
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
                                                    LOGGER.error(&format!(
                                                        "Failed to handle message: {}",
                                                        e
                                                    ));
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            } else {
                                LOGGER.error("[Turn] Connection not found");
                            }
                        }
                    }
                    ConnectionType::Stun => {
                        LOGGER.debug(&format!("Received message from Tunnel: {:?}", packet));
                    }
                }
            } else {
                LOGGER.debug("No messages received, sleeping...");
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        }
    }
}
