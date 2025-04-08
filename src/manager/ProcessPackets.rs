use super::ConnectionManager::ConnectionManager;
use crate::db::{Fragment, Storage};
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::{
    PeerFileSaved, PeerUploadFile, Protocol, Status, SyncPeerInfoData, TransportPacket,
};
use crate::peer::turn_tunnel;
use colored::Colorize;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
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
                            println!(
                                "{}",
                                format!("[Peer] Received packet: {:?}", packet).yellow()
                            );
                            let from_public_addr = packet.public_addr.clone();
                            let from_uuid = packet.uuid.clone();
                            let packet_clone = packet.clone();
                            let protocol_connection = packet.protocol.clone();
                            if packet.act == "peer_list" {
                                if let Some(data) = packet.data {
                                    match serde_json::from_value::<SyncPeerInfoData>(data) {
                                        Ok(peer_info_data) => {
                                            println!("{}", "[Peer] Received peer list:".yellow());
                                            for peer in peer_info_data.peers {
                                                println!(
                                                    "{}",
                                                    format!(
                                                    "[Peer] Peer - Public Address: {}, UUID: {}",
                                                    peer.public_addr, peer.uuid
                                                )
                                                    .cyan()
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            println!(
                                                "{}",
                                                format!("[Peer] Failed to parse peer list: {}", e)
                                                    .red()
                                            );
                                        }
                                    }
                                } else {
                                    println!("{}", "[Peer] Peer list data is missing.".red());
                                }
                            } else if protocol_connection == Protocol::STUN
                                && packet.act == "wait_connection"
                            {
                                println!(
                                    "{}",
                                    format!(
                                        "[Peer] [Stun] From public address: {}",
                                        from_public_addr
                                    )
                                    .yellow()
                                );
                                println!("{}", "[Peer] Start stun tunnel".yellow());
                                let result_tunnel = self.stun_tunnel(packet).await;
                                match result_tunnel {
                                    Ok(_) => {
                                        println!("{}", "[STUN] Connection established!".green());
                                    }
                                    Err(e) => {
                                        self.connections_turn.write().await.insert(
                                            from_uuid.clone(),
                                            ConnectionTurnStatus {
                                                connected: false,
                                                turn_connection: true,
                                            },
                                        );
                                        println!(
                                            "{}",
                                            format!("[Peer] Failed to establish connection: {}", e)
                                                .red()
                                        );
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
                            //println from_public_addr
                            println!(
                                "{}",
                                format!("[Peer] From public address: {}", from_public_addr)
                                    .yellow()
                            );
                            if let Some(status) =
                                self.connections_turn.write().await.get_mut(&from_uuid)
                            {
                                if status.turn_connection && !status.connected {
                                    let result_turn_tunnel = turn_tunnel(
                                        packet_clone,
                                        self.my_public_addr.clone(),
                                        &connection,
                                        &self.db,
                                    )
                                    .await;
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
                                                    public_addr: (*self.my_public_addr).clone(),
                                                    act: "test_turn".to_string(),
                                                    to: Some(from_public_addr.clone()),
                                                    data: None,
                                                    status: None,
                                                    protocol: Protocol::TURN,
                                                    uuid: self.db.get_or_create_peer_id().unwrap(),
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
                                    if packet_clone.act == "save_file" {
                                        let data = packet_file_clone.data.unwrap();
                                        let session_key = self
                                            .db
                                            .generate_and_store_secret_key(
                                                data["peer_id"].as_str().unwrap(),
                                            )
                                            .unwrap();

                                        let filename = data["filename"].as_str().unwrap();
                                        let contents =
                                            base64::decode(data["contents"].as_str().unwrap())
                                                .unwrap();
                                        let dir_path: String =
                                            format!("{}/files", self.db.path.as_str());
                                        if !std::path::Path::new(&dir_path).exists() {
                                            tokio::fs::create_dir_all(&dir_path).await.unwrap();
                                        }
                                        let path = format!("{}/{}", dir_path, filename);
                                        let mut file = File::create(path).await.unwrap();
                                        file.write_all(&contents).await.unwrap();

                                        let _ = self.db.add_storage_fragment(Storage {
                                            filename: filename.to_string(),
                                            session_key: session_key.clone(),
                                            session: session_key.clone().to_string().to_string(),
                                            owner_id: data["peer_id"].as_str().unwrap().to_string(),
                                            storage_peer_id: self
                                                .db
                                                .get_or_create_peer_id()
                                                .unwrap(),
                                        });

                                        println!("{}", "[Peer] File saved".green());

                                        let packet_feedback = TransportPacket {
                                            public_addr: (*self.my_public_addr).clone(),
                                            act: "file_saved".to_string(),
                                            to: Some(from_public_addr.clone()),
                                            data: Some(
                                                serde_json::to_value(PeerFileSaved {
                                                    filename: filename.to_string(),
                                                    session_key: session_key.clone(),
                                                    peer_id: self
                                                        .db
                                                        .get_or_create_peer_id()
                                                        .unwrap(),
                                                })
                                                .unwrap(),
                                            ),
                                            status: None,
                                            protocol: Protocol::TURN,
                                            uuid: self.db.get_or_create_peer_id().unwrap(),
                                        };

                                        if let Err(e) =
                                            connection.send_packet(packet_feedback).await
                                        {
                                            println!(
                                                "{}",
                                                format!("[Peer] Failed to send packet: {}", e)
                                                    .red()
                                            );
                                        }
                                    } else if packet_clone.act == "file_saved" {
                                        let data = packet_clone.data.unwrap();
                                        let filename = data["filename"].as_str().unwrap();
                                        let session_key = data["session_key"].as_str().unwrap();
                                        let peer_id = data["peer_id"].as_str().unwrap();

                                        let _ = self.db.add_myfile_fragment(Fragment {
                                            uuid_peer: peer_id.to_string(),
                                            session_key: session_key.to_string(),
                                            session: session_key.to_string(),
                                            filename: filename.to_string(),
                                        });

                                        println!(
                                            "{}",
                                            format!(
                                                "\x1b[32m[Peer] File saved. Session key: {}\x1b[0m",
                                                session_key
                                            )
                                            .green()
                                        );
                                    } else if packet_clone.act == "get_file" {
                                        println!("{}", "Get file packet".yellow());
                                        let data = packet_file_clone.data.unwrap();
                                        let session_key = data["session_key"].as_str().unwrap();
                                        let contents =
                                            self.db.get_storage_fragments_by_key(session_key);
                                        for fragment in contents.unwrap() {
                                            let dir_path =
                                                format!("{}/files", self.db.path.as_str());
                                            let path =
                                                format!("{}/{}", dir_path, fragment.filename);
                                            let mut file = File::open(path).await.unwrap();
                                            let mut contents = vec![];
                                            file.read_to_end(&mut contents).await.unwrap();

                                            let peer_upload_file =
                                                serde_json::to_value(PeerUploadFile {
                                                    filename: fragment.filename.clone(),
                                                    contents: base64::encode(contents),
                                                    peer_id: self
                                                        .db
                                                        .get_or_create_peer_id()
                                                        .unwrap(),
                                                })
                                                .unwrap();

                                            let packet_file = TransportPacket {
                                                public_addr: (*self.my_public_addr).clone(),
                                                act: "file".to_string(),
                                                to: Some(from_public_addr.clone()),
                                                data: Some(peer_upload_file),
                                                status: Some(Status::SUCCESS),
                                                protocol: Protocol::TURN,
                                                uuid: self.db.get_or_create_peer_id().unwrap(),
                                            };
                                            println!(
                                                "{}",
                                                format!(
                                                    "[Peer] Sending file: {}",
                                                    fragment.filename.clone()
                                                )
                                                .cyan()
                                            );
                                            if let Err(e) =
                                                connection.send_packet(packet_file).await
                                            {
                                                println!(
                                                    "{}",
                                                    format!("[Peer] Failed to send packet: {}", e)
                                                        .red()
                                                );
                                            } else {
                                                println!(
                                                    "{}",
                                                    "[Peer] Packet sent successfully".green()
                                                );
                                            }
                                        }
                                    } else if packet_clone.act == "file" {
                                        let data = packet_file_clone.data.unwrap();
                                        let filename = data["filename"].as_str().unwrap();
                                        let contents = data["contents"].as_str().unwrap();
                                        let dir_path: String =
                                            format!("{}/recive_files", self.db.path.as_str());
                                        if !std::path::Path::new(&dir_path).exists() {
                                            tokio::fs::create_dir_all(&dir_path).await.unwrap();
                                        }
                                        let path = format!("{}/{}", dir_path, filename);
                                        let mut file = File::create(path).await.unwrap();
                                        let contents = base64::decode(contents).unwrap();
                                        file.write_all(&contents).await.unwrap();
                                        println!(
                                            "{}",
                                            format!("[Peer] File saved: {}", filename).green()
                                        );
                                    } else if packet_clone.act == "message" {
                                        let data = packet_clone.data.unwrap();
                                        let message = data["text"].as_str().unwrap();
                                        println!(
                                            "{}",
                                            format!("[Peer] Message: {}", message).green()
                                        );

                                        let response_packet = TransportPacket {
                                            public_addr: (*self.my_public_addr).clone(),
                                            act: "message_response".to_string(),
                                            to: Some(from_public_addr.clone()),
                                            data: Some(
                                                serde_json::json!({ "text": "Message received" }),
                                            ),
                                            status: None,
                                            protocol: Protocol::TURN,
                                            uuid: self.db.get_or_create_peer_id().unwrap(),
                                        };
                                        if let Err(e) =
                                            connection.send_packet(response_packet).await
                                        {
                                            println!(
                                                "{}",
                                                format!("[Peer] Failed to send response: {}", e)
                                                    .red()
                                            );
                                        }
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
