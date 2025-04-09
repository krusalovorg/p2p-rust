use super::ConnectionManager::ConnectionManager;
use crate::db::{Fragment, Storage};
use crate::manager::types::{ConnectionTurnStatus, ConnectionType};
use crate::packets::{
    FileData, Message, PeerFileSaved, PeerUploadFile, Protocol, SaveFile, Status, SyncPeerInfoData,
    TransportData, TransportPacket,
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
                            let from_uuid = packet.uuid.clone();
                            let packet_clone = packet.clone();
                            let protocol_connection = packet.protocol.clone();
                            if packet.act == "peer_list" {
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
                            } else if protocol_connection == Protocol::STUN
                                && packet.act == "wait_connection"
                            {
                                println!(
                                    "{}",
                                    format!("[Peer] [Stun] From UUID: {}", from_uuid).yellow()
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

                            println!(
                                "{}",
                                format!("[Peer] From UUID: {}", from_uuid.clone()).yellow()
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
                                                    to: Some(from_uuid.clone()),
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
                                        println!(
                                            "{}",
                                            "[Peer] Peer want upload file, getted save_file packet"
                                                .yellow()
                                        );
                                        if let Some(TransportData::PeerUploadFile(data)) =
                                            packet_file_clone.data
                                        {
                                            println!(
                                                "{}",
                                                "[Peer] File uploaded successfully".green()
                                            );
                                            let session_key = self
                                                .db
                                                .generate_and_store_secret_key(&data.peer_id)
                                                .unwrap();

                                            let contents = base64::decode(&data.contents).unwrap();
                                            let dir_path: String =
                                                format!("{}/files", self.db.path.as_str());
                                            if !std::path::Path::new(&dir_path).exists() {
                                                tokio::fs::create_dir_all(&dir_path).await.unwrap();
                                            }
                                            let path = format!("{}/{}", dir_path, data.filename);
                                            let mut file = File::create(path).await.unwrap();
                                            file.write_all(&contents).await.unwrap();

                                            let _ = self.db.add_storage_fragment(Storage {
                                                filename: data.filename.clone(),
                                                session_key: session_key.clone(),
                                                session: session_key.clone(),
                                                owner_id: data.peer_id,
                                                storage_peer_id: self
                                                    .db
                                                    .get_or_create_peer_id()
                                                    .unwrap(),
                                            });

                                            println!("{}", "[Peer] File saved".green());

                                            let packet_feedback = TransportPacket {
                                                public_addr: (*self.my_public_addr).clone(),
                                                act: "file_saved".to_string(),
                                                to: Some(from_uuid.clone()),
                                                data: Some(TransportData::PeerFileSaved(
                                                    PeerFileSaved {
                                                        filename: data.filename,
                                                        session_key: session_key,
                                                        peer_id: self
                                                            .db
                                                            .get_or_create_peer_id()
                                                            .unwrap(),
                                                    },
                                                )),
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
                                        }
                                    } else if packet_clone.act == "file_saved" {
                                        if let Some(TransportData::PeerFileSaved(data)) =
                                            packet_clone.data
                                        {
                                            let session_key_clone =
                                                data.session_key.clone().to_string();
                                            let _ = self.db.add_myfile_fragment(Fragment {
                                                uuid_peer: data.peer_id,
                                                session_key: session_key_clone.clone(),
                                                session: session_key_clone.clone(),
                                                filename: data.filename.clone(),
                                            });

                                            println!("{}", format!("\x1b[32m[Peer] File saved. Session key: {}\x1b[0m", session_key_clone).green());
                                        }
                                    } else if packet_clone.act == "get_file" {
                                        println!("{}", "Get file packet".yellow());
                                        if let Some(TransportData::PeerFileGet(data)) =
                                            packet_file_clone.data
                                        {
                                            let contents = self
                                                .db
                                                .get_storage_fragments_by_key(&data.session_key);
                                            for fragment in contents.unwrap() {
                                                let dir_path =
                                                    format!("{}/files", self.db.path.as_str());
                                                let path =
                                                    format!("{}/{}", dir_path, fragment.filename);
                                                let mut file = File::open(path).await.unwrap();
                                                let mut contents = vec![];
                                                file.read_to_end(&mut contents).await.unwrap();

                                                let packet_file = TransportPacket {
                                                    public_addr: (*self.my_public_addr).clone(),
                                                    act: "file".to_string(),
                                                    to: Some(from_uuid.clone()),
                                                    data: Some(TransportData::FileData(FileData {
                                                        filename: fragment
                                                            .filename
                                                            .clone()
                                                            .to_string(),
                                                        contents: base64::encode(contents),
                                                        peer_id: self
                                                            .db
                                                            .get_or_create_peer_id()
                                                            .unwrap(),
                                                    })),
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
                                                        format!(
                                                            "[Peer] Failed to send packet: {}",
                                                            e
                                                        )
                                                        .red()
                                                    );
                                                } else {
                                                    println!(
                                                        "{}",
                                                        "[Peer] Packet sent successfully".green()
                                                    );
                                                }
                                            }
                                        }
                                    } else if packet_clone.act == "file" {
                                        if let Some(TransportData::FileData(data)) =
                                            packet_file_clone.data
                                        {
                                            let dir_path: String =
                                                format!("{}/recive_files", self.db.path.as_str());
                                            if !std::path::Path::new(&dir_path).exists() {
                                                tokio::fs::create_dir_all(&dir_path).await.unwrap();
                                            }
                                            let path = format!("{}/{}", dir_path, data.filename);
                                            let mut file = File::create(path).await.unwrap();
                                            let contents = base64::decode(data.contents).unwrap();
                                            file.write_all(&contents).await.unwrap();
                                            println!(
                                                "{}",
                                                format!("[Peer] File saved: {}", data.filename)
                                                    .green()
                                            );
                                        }
                                    } else if packet_clone.act == "message_response" {
                                        println!("{}", "[Peer] Message sent successfully".green());
                                    } else if packet_clone.act == "message" {
                                        if let Some(TransportData::Message(data)) =
                                            packet_clone.data
                                        {
                                            println!(
                                                "{}",
                                                format!("[Peer] Message: {}", data.text).green()
                                            );

                                            let response_packet = TransportPacket {
                                                public_addr: (*self.my_public_addr).clone(),
                                                act: "message_response".to_string(),
                                                to: Some(from_uuid.clone()),
                                                data: Some(TransportData::Message(Message {
                                                    text: "Message received".to_string(),
                                                })),
                                                status: None,
                                                protocol: Protocol::TURN,
                                                uuid: self.db.get_or_create_peer_id().unwrap(),
                                            };
                                            if let Err(e) =
                                                connection.send_packet(response_packet).await
                                            {
                                                println!(
                                                    "{}",
                                                    format!(
                                                        "[Peer] Failed to send response: {}",
                                                        e
                                                    )
                                                    .red()
                                                );
                                            }
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
