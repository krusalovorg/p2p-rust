use base64;
use colored::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Mutex,RwLock};

use crate::config::Config;
use crate::connection::Connection;
use crate::db::{Fragment, Storage};
use crate::packets::{
    PeerFileSaved, PeerUploadFile, Protocol, Status, SyncPeerInfoData,
    TransportPacket,
};
use crate::manager::ConnectionTurnStatus;
use crate::peer::{stun_tunnel, turn_tunnel};
use crate::tunnel::Tunnel;
use crate::ui::console_manager;
use crate::GLOBAL_DB;

pub async fn run_peer() {
    let config: Config = Config::from_file("config.toml");
    let tunnel = Arc::new(Mutex::new(Tunnel::new().await));

    {
        let tunnel_guard = tunnel.lock().await;
        println!(
            "{}",
            format!(
                "[Peer] You public ip:port: {}:{}",
                tunnel_guard.get_public_ip(),
                tunnel_guard.get_public_port()
            )
            .yellow()
        );
    }

    let signal_server_ip_clone = config.signal_server_ip.clone();
    let signal_server_port_clone = config.signal_server_port;

    let (tunnel_public_ip, tunnel_public_port) = {
        let tunnel_guard = tunnel.lock().await;
        (tunnel_guard.get_public_ip(), tunnel_guard.get_public_port())
    };

    let my_public_addr = format!("{}:{}", tunnel_public_ip, tunnel_public_port);

    let connection = Arc::new(
        Connection::new(
            signal_server_ip_clone,
            signal_server_port_clone,
            tunnel_public_ip,
            tunnel_public_port,
        )
        .await,
    );

    let connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>> =
        Arc::new(RwLock::new(HashMap::new()));

    connections_turn.write().await.insert(
        format!("{}:{}", config.signal_server_ip, config.signal_server_port).clone(),
        ConnectionTurnStatus {
            connected: true,
            turn_connection: true,
        },
    );

    let peer_id = GLOBAL_DB.get_or_create_peer_id().unwrap();
    println!("{}", format!("[Peer] Your UUID: {}", peer_id).yellow());

    let my_public_addr_clone = Arc::new(my_public_addr.to_string()).clone();

    tokio::spawn({
        let tunnel = Arc::clone(&tunnel);
        let connections_turn = Arc::clone(&connections_turn);
        let connection = Arc::clone(&connection);
        async move {
            loop {
                console_manager(my_public_addr_clone.clone(), connections_turn.clone(), connection.clone()).await;
            }
        }
    });

    let connections_turn_clone = Arc::clone(&connections_turn);

    loop {
        println!("{}", "[Peer] Start wait new packets...".yellow());
        let result = connection.get_response().await;
        match result {
            Ok(packet) => {
                println!(
                    "{}",
                    format!("[Peer] Received packet: {:?}", packet).yellow()
                );
                let from_public_addr = packet.public_addr.clone();
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
                                    format!("[Peer] Failed to parse peer list: {}", e).red()
                                );
                            }
                        }
                    } else {
                        println!("{}", "[Peer] Peer list data is missing.".red());
                    }
                } else if protocol_connection == Protocol::STUN && packet.act == "wait_connection" {
                    println!("{}", "[Peer] Start stun tunnel".yellow());
                    let result_tunnel = stun_tunnel(packet, Arc::clone(&tunnel)).await;
                    match result_tunnel {
                        Ok(_) => {
                            println!("{}", "[STUN] Connection established!".green());
                        }
                        Err(e) => {
                            connections_turn_clone.write().await.insert(
                                from_public_addr.clone(),
                                ConnectionTurnStatus {
                                    connected: false,
                                    turn_connection: true,
                                },
                            );
                            println!(
                                "{}",
                                format!("[Peer] Failed to establish connection: {}", e).red()
                            );
                        }
                    }
                } else if protocol_connection == Protocol::TURN && packet.act == "wait_connection" {
                    connections_turn_clone.write().await.insert(
                        from_public_addr.clone(),
                        ConnectionTurnStatus {
                            connected: false,
                            turn_connection: true,
                        },
                    );
                }
                if let Some(status) = connections_turn.write().await.get_mut(&from_public_addr) {
                    // println!("[Peer] [Turn] Status {:?}", status);
                    if status.turn_connection && !status.connected {
                        let result_turn_tunnel =
                            turn_tunnel(packet_clone, Arc::new(my_public_addr.clone().to_string()), &connection).await;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        println!(
                            "{}",
                            format!("[Peer] Result turn tunnel {:?}", result_turn_tunnel).yellow()
                        );
                        match result_turn_tunnel {
                            Ok(r) => {
                                if r == "successful_connection" {
                                    println!("{}", "[TURN] Connection established!".green());
                                    status.connected = true;
                                    status.turn_connection = false;

                                    let packet_hello = TransportPacket {
                                        public_addr: my_public_addr.clone(),
                                        act: "test_turn".to_string(),
                                        to: Some(from_public_addr.clone()),
                                        data: None,
                                        status: None,
                                        protocol: Protocol::TURN,
                                        uuid: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                                    };
                                    println!("{}", "[Peer] Sending accept connection".yellow());
                                    let _ = connection.send_packet(packet_hello).await;
                                } else if r == "send_wait_connection" {
                                    println!(
                                        "{}",
                                        "[Peer] Wait answer acceptation connection...".yellow()
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
                            let session_key = GLOBAL_DB
                                .generate_and_store_secret_key(data["peer_id"].as_str().unwrap())
                                .unwrap();

                            let filename = data["filename"].as_str().unwrap();
                            let contents =
                                base64::decode(data["contents"].as_str().unwrap()).unwrap();
                            let dir_path: String = format!("{}/files", GLOBAL_DB.path.as_str());
                            if !std::path::Path::new(&dir_path).exists() {
                                tokio::fs::create_dir_all(&dir_path).await.unwrap();
                            }
                            let path = format!("{}/{}", dir_path, filename);
                            let mut file = File::create(path).await.unwrap();
                            file.write_all(&contents).await.unwrap();

                            let _ = GLOBAL_DB.add_storage_fragment(Storage {
                                filename: filename.to_string(),
                                session_key: session_key.clone(),
                                session: session_key.clone().to_string().to_string(),
                                owner_id: data["peer_id"].as_str().unwrap().to_string(),
                                storage_peer_id: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                            });

                            println!("{}", "[Peer] File saved".green());

                            //send feedback -> file saved and return session key
                            let packet_feedback = TransportPacket {
                                public_addr: my_public_addr.clone(),
                                act: "file_saved".to_string(),
                                to: Some(from_public_addr.clone()),
                                data: Some(
                                    serde_json::to_value(PeerFileSaved {
                                        filename: filename.to_string(),
                                        session_key: session_key.clone(),
                                        peer_id: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                                    })
                                    .unwrap(),
                                ),
                                status: None,
                                protocol: Protocol::TURN,
                                uuid: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                            };

                            if let Err(e) = connection.send_packet(packet_feedback).await {
                                println!(
                                    "{}",
                                    format!("[Peer] Failed to send packet: {}", e).red()
                                );
                            }
                        } else if packet_clone.act == "file_saved" {
                            let data = packet_clone.data.unwrap();
                            let filename = data["filename"].as_str().unwrap();
                            let session_key = data["session_key"].as_str().unwrap();
                            let peer_id = data["peer_id"].as_str().unwrap();

                            // uuid_peer: (), session_key: (), session: (), fragment: ()
                            let _ = GLOBAL_DB.add_myfile_fragment(Fragment {
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
                            let contents = GLOBAL_DB.get_storage_fragments_by_key(session_key);
                            for fragment in contents.unwrap() {
                                let dir_path = format!("{}/files", GLOBAL_DB.path.as_str());
                                let path = format!("{}/{}", dir_path, fragment.filename);
                                let mut file = File::open(path).await.unwrap();
                                let mut contents = vec![];
                                file.read_to_end(&mut contents).await.unwrap();

                                let peer_upload_file = serde_json::to_value(PeerUploadFile {
                                    filename: fragment.filename.clone(),
                                    contents: base64::encode(contents),
                                    peer_id: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                                })
                                .unwrap();

                                let packet_file = TransportPacket {
                                    public_addr: my_public_addr.clone(),
                                    act: "file".to_string(),
                                    to: Some(from_public_addr.clone()),
                                    data: Some(peer_upload_file),
                                    status: Some(Status::SUCCESS),
                                    protocol: Protocol::TURN,
                                    uuid: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                                };
                                println!(
                                    "{}",
                                    format!("[Peer] Sending file: {}", fragment.filename.clone())
                                        .cyan()
                                );
                                if let Err(e) = connection.send_packet(packet_file).await {
                                    println!(
                                        "{}",
                                        format!("[Peer] Failed to send packet: {}", e).red()
                                    );
                                } else {
                                    println!("{}", "[Peer] Packet sent successfully".green());
                                }
                            }
                        } else if packet_clone.act == "file" {
                            let data = packet_file_clone.data.unwrap();
                            let filename = data["filename"].as_str().unwrap();
                            let contents = data["contents"].as_str().unwrap();
                            let dir_path: String =
                                format!("{}/recive_files", GLOBAL_DB.path.as_str());
                            if !std::path::Path::new(&dir_path).exists() {
                                tokio::fs::create_dir_all(&dir_path).await.unwrap();
                            }
                            let path = format!("{}/{}", dir_path, filename);
                            let mut file = File::create(path).await.unwrap();
                            let contents = base64::decode(contents).unwrap();
                            file.write_all(&contents).await.unwrap();
                            println!("{}", format!("[Peer] File saved: {}", filename).green());
                        } else if packet_clone.act == "message" {
                            let data = packet_clone.data.unwrap();
                            let message = data["text"].as_str().unwrap();
                            println!("{}", format!("[Peer] Message: {}", message).green());
                        }
                    }
                } else {
                    println!("{}", "[Peer] [Turn] Connection not found".red());
                }
            }
            Err(e) => {
                println!("{}", format!("[Peer] Error: {}", e).red());
                break;
            }
        }
    }
}
