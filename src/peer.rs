use anyhow::Result;
use async_std::sync::RwLock;
use base64;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::config::Config;
use crate::connection::Connection;
use crate::db::{Fragment, P2PDatabase, Storage};
use crate::signal::{Protocol, SignalClient, TransportPacket};
use crate::tunnel::Tunnel;
use crate::ui::print_all_files;
use crate::GLOBAL_DB;
use std::io::{self, Write};

#[derive(Debug, Clone)]
struct ConnectionTurnStatus {
    connected: bool,
    turn_connection: bool,
}

// Console manager for use send files use tunnel class or connection class (stun or turn protocol)
async fn console_manager(
    tunnel: Arc<Mutex<Tunnel>>,
    connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>>,
    connection: Arc<Connection>,
) {
    let mut input = String::new();
    print!("\x1b[32m[P2P] >\x1b[0m ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut input).unwrap();
    let trimmed_input = input.trim();

    let public_ip = tunnel.lock().await.get_public_ip();
    let public_port = tunnel.lock().await.get_public_port();
    let is_connected = tunnel.lock().await.is_connected().await;

    if trimmed_input == "files" {
        print_all_files();
    } else if is_connected {
        if trimmed_input.starts_with("file ") {
            let file_path = trimmed_input.strip_prefix("file ").unwrap();
            println!("[Peer] Sending file: {}", file_path);
            tunnel.lock().await.send_file_path(file_path).await;
        } else {
            tunnel.lock().await.send_message(trimmed_input).await;
        }
    } else if connections_turn.read().await.len() > 0 {
        let connections_turn_clone = connections_turn.read().await;
        for (key, connection_turn_status) in connections_turn_clone.iter() {
            if connection_turn_status.connected && !connection_turn_status.turn_connection {
                if trimmed_input.starts_with("get ") {
                    //send packet with get file by session key
                    let session_key = trimmed_input.strip_prefix("get ").unwrap();
                    let packet = TransportPacket {
                        public_addr: format!("{}:{}", public_ip, public_port),
                        act: "get_file".to_string(),
                        to: Some(key.clone()),
                        data: Some(json!({"session_key": session_key})),
                        session_key: None,
                        status: None,
                        protocol: Protocol::TURN,
                    };
                    if let Err(e) = connection.send_packet(packet).await {
                        println!("[Peer] Failed to send packet: {}", e);
                    } else {
                        println!("[Peer] Packet sent successfully");
                    }
                } else if trimmed_input.starts_with("file ") {
                    let file_path = trimmed_input.strip_prefix("file ").unwrap();
                    println!("[Peer] Sending file: {}", file_path);
                    if let Ok(mut file) = File::open(file_path).await {
                        let mut contents = vec![];
                        file.read_to_end(&mut contents).await.unwrap();
                        let packet = TransportPacket {
                            public_addr: format!("{}:{}", public_ip, public_port),
                            act: "save_file".to_string(),
                            to: Some(key.clone()),
                            data: Some(json!({
                                    "filename": file_path,
                                    "contents": base64::encode(contents),
                                    "peer_id": GLOBAL_DB.get_peer_id()
                            })),
                            session_key: None,
                            status: None,
                            protocol: Protocol::TURN,
                        };
                        if let Err(e) = connection.send_packet(packet).await {
                            println!("[Peer] Failed to send packet: {}", e);
                        } else {
                            println!("[Peer] Packet sent successfully");
                        }
                    } else {
                        println!("[Peer] Failed to open file: {}", file_path);
                    }
                } else {
                    let packet = TransportPacket {
                        public_addr: format!("{}:{}", public_ip, public_port),
                        act: "message".to_string(),
                        to: Some(key.clone()),
                        data: Some(json!({"text": trimmed_input.to_string()})),
                        session_key: None,
                        status: None,
                        protocol: Protocol::TURN,
                    };
                    if let Err(e) = connection.send_packet(packet).await {
                        println!("[Peer] Failed to send packet: {}", e);
                    } else {
                        println!("[Peer] Packet sent successfully");
                    }
                }
            }
        }
    }
}

pub async fn run_peer(db: &P2PDatabase) {
    let config: Config = Config::from_file("config.toml");
    let tunnel = Arc::new(Mutex::new(Tunnel::new().await));

    {
        let tunnel_guard = tunnel.lock().await;
        println!(
            "[Peer] You public ip:port: {}:{}",
            tunnel_guard.get_public_ip(),
            tunnel_guard.get_public_port()
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

    let peer_id = db.get_peer_id();
    println!("[Peer] Your UUID: {}", peer_id);

    tokio::spawn({
        let tunnel = Arc::clone(&tunnel);
        let connections_turn = Arc::clone(&connections_turn);
        let connection = Arc::clone(&connection);
        async move {
            loop {
                console_manager(tunnel.clone(), connections_turn.clone(), connection.clone()).await;
            }
        }
    });

    let connections_turn_clone = Arc::clone(&connections_turn);

    loop {
        let result = connection.get_response().await;
        match result {
            Ok(packet) => {
                println!("[Peer] Received packet: {:?}", packet);
                let from_public_addr = packet.public_addr.clone();
                let packet_clone = packet.clone();
                let protocol_connection = packet.protocol.clone();
                if protocol_connection == Protocol::STUN && packet.act == "wait_connection" {
                    println!("[Peer] Start stun tunnel");
                    let result_tunnel = stun_tunnel(packet, Arc::clone(&tunnel)).await;
                    match result_tunnel {
                        Ok(_) => {
                            println!("[STUN] Connection established!");
                        }
                        Err(e) => {
                            connections_turn_clone.write().await.insert(
                                from_public_addr.clone(),
                                ConnectionTurnStatus {
                                    connected: false,
                                    turn_connection: true,
                                },
                            );
                            println!("[Peer] Failed to establish connection: {}", e);
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
                            turn_tunnel(packet_clone, Arc::clone(&tunnel), &connection).await;
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        println!("[Peer] Result turn tunnel {:?}", result_turn_tunnel);
                        match result_turn_tunnel {
                            Ok(r) => {
                                if r == "successful_connection" {
                                    println!("[TURN] Connection established!");
                                    status.connected = true;
                                    status.turn_connection = false;

                                    let packet_hello = TransportPacket {
                                        public_addr: my_public_addr.clone(),
                                        act: "test_turn".to_string(),
                                        to: Some(from_public_addr.clone()),
                                        data: None,
                                        session_key: None,
                                        status: None,
                                        protocol: Protocol::TURN,
                                    };
                                    println!("[Peer] Sending accept connection");
                                    let _ = connection.send_packet(packet_hello).await;
                                } else if r == "send_wait_connection" {
                                    println!("[Peer] Wait answer acceptation connection...");
                                }
                            }
                            Err(e) => {
                                status.connected = false;
                                status.turn_connection = true;
                                println!("[Peer] Fail: {}", e);
                            }
                        }
                        println!("[Peer] Wait new packets...");
                    } else {
                        let packet_file_clone = packet_clone.clone();
                        if packet_clone.act == "save_file" {
                            let data = packet_file_clone.data.unwrap();
                            let session_key =
                                db.generate_and_store_secret_key(data["peer_id"].as_str().unwrap());
                            let filename = data["filename"].as_str().unwrap();
                            let contents =
                                base64::decode(data["contents"].as_str().unwrap()).unwrap();
                            let dir_path = format!("{}/files", GLOBAL_DB.path.to_str().unwrap());
                            if !std::path::Path::new(&dir_path).exists() {
                                tokio::fs::create_dir_all(&dir_path).await.unwrap();
                            }
                            let path = format!("{}/{}", dir_path, filename);
                            let mut file = File::create(path).await.unwrap();
                            file.write_all(&contents).await.unwrap();

                            GLOBAL_DB.add_storage_fragment(Storage {
                                filename: filename.to_string(),
                                session_key: session_key.clone(),
                                session: session_key.clone(),
                                uuid_peer: data["peer_id"].as_str().unwrap().to_string(),
                            });

                            println!("[Peer] File saved");

                            //send feedback -> file saved and return session key
                            let packet_feedback = TransportPacket {
                                public_addr: my_public_addr.clone(),
                                act: "file_saved".to_string(),
                                to: Some(from_public_addr.clone()),
                                data: Some(json!({
                                    "filename": filename,
                                    "session_key": session_key,
                                    "peer_id": GLOBAL_DB.get_peer_id()
                                })),
                                session_key: None,
                                status: None,
                                protocol: Protocol::TURN,
                            };

                            if let Err(e) = connection.send_packet(packet_feedback).await {
                                println!("[Peer] Failed to send packet: {}", e);
                            } else {
                                println!("[Peer] Packet sent successfully");
                            }
                        }

                        if packet_clone.act == "file_saved" {
                            let data = packet_clone.data.unwrap();
                            let filename = data["filename"].as_str().unwrap();
                            let session_key = data["session_key"].as_str().unwrap();
                            let peer_id = data["peer_id"].as_str().unwrap();

                            // uuid_peer: (), session_key: (), session: (), fragment: ()
                            GLOBAL_DB.add_myfile_fragment(
                                filename,
                                Fragment {
                                    uuid_peer: peer_id.to_string(),
                                    session_key: session_key.to_string(),
                                    session: session_key.to_string(),
                                    filename: filename.to_string(),
                                },
                            );

                            println!(
                                "\x1b[32m[Peer] File saved. Session key: {}\x1b[0m",
                                session_key
                            );
                        }
                    }
                } else {
                    println!("[Peer] [Turn] Connection not found");
                }
            }
            Err(e) => {
                println!("[Peer] Error: {}", e);
                break;
            }
        }
    }
}

pub async fn turn_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
    signal: &Connection,
) -> Result<String, String> {
    let tunnel_clone = tunnel.lock().await;
    let public_ip = tunnel_clone.public_ip.clone();
    let public_port = tunnel_clone.public_port.clone();
    println!(
        "[TURN] Turn tunnel creating, sending packets.. {}",
        packet.act
    );
    if packet.act == "wait_connection" {
        let packet_hello = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "try_turn_connection".to_string(),
            to: Some(packet.public_addr.clone().to_string()),
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::TURN,
        };
        let result = signal.send_packet(packet_hello).await;
        println!(
            "[TURN] [try_turn_connection] Result sending socket {:?}",
            result
        );
        match result {
            Ok(_) => {
                return Ok("send_wait_connection".to_string());
            }
            Err(e) => {
                return Err(e);
            }
        }
    } else if packet.act == "accept_connection" || packet.act == "try_turn_connection" {
        let packet_hello = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "accept_connection".to_string(),
            to: Some(packet.public_addr.to_string()),
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::TURN,
        };
        println!("[TURN] [accept_connection] Sending accept connection");
        let result = signal.send_packet(packet_hello).await;
        match result {
            Ok(_) => {
                return Ok("successful_connection".to_string());
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    return Err("[TURN] Peer didn't give the connection agreement".to_string());
}

pub async fn stun_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
) -> Result<(), String> {
    println!(
        "[STUN] Entering stun_tunnel with public address: {:?}",
        packet.public_addr
    );
    match SignalClient::extract_addr(packet.public_addr).await {
        Ok((ip, port)) => {
            let ip = ip.to_string();
            let mut tunnel = tunnel.lock().await;
            println!("[STUN] Try connecting to {}:{}", ip, port);
            match tunnel.make_connection(&ip, port, 3).await {
                Ok(()) => {
                    println!("[STUN] Connection established with {}:{}!", ip, port);
                    tunnel.backlife_cycle(1);

                    loop {
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).unwrap();
                        let trimmed_input = input.trim();

                        if trimmed_input.starts_with("file ") {
                            let file_path = trimmed_input.strip_prefix("file ").unwrap();
                            println!("[STUN] Sending file: {}", file_path);
                            tunnel.send_file_path(file_path).await;
                        } else {
                            tunnel.send_message(trimmed_input).await;
                        }
                    }
                }
                Err(e) => {
                    println!("[STUN] Failed to make connection: {}", e);
                    return Err("[STUN] Fail connection".to_string());
                }
            }
        }
        Err(e) => {
            println!("[STUN] Failed to extract address: {}", e);
            return Err("[STUN] Fail extract address".to_string());
        }
    }
}
