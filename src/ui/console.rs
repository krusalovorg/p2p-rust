use serde_json::json;
use std::collections::HashMap;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use async_std::sync::RwLock;
use base64;
use crate::connection::Connection;
use crate::packets::PeerUploadFile;
use std::io::{self, Write};
use std::sync::Arc;

use colored::*;

use crate::{signal::{Protocol, TransportPacket}, tunnel::Tunnel, GLOBAL_DB};
use crate::peer::ConnectionTurnStatus;

pub fn print_all_files() {
    let myfiles = GLOBAL_DB.get_myfile_fragments();
    
    //UUID Peer
    let uuid_peer = GLOBAL_DB.get_or_create_peer_id().unwrap();
    println!("{}", format!("[Peer] UUID: {}", uuid_peer).yellow());

    match myfiles {
        Ok(myfiles) => {
            println!("{}", "My Files:".bold().underline().blue());
            for fragment in myfiles {
                println!("  {}: {}", "UUID Peer".yellow(), fragment.uuid_peer);
                println!("  {}: {}", "Session Key".yellow(), fragment.session_key);
                println!("  {}: {}", "Session".yellow(), fragment.session);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!();
            }
        },
        Err(_) => (()),
    }
}

pub fn print_all_fragments() {
    let fragments = GLOBAL_DB.get_storage_fragments();

    match fragments {
        Ok(fragments) => {
            println!("{}", "All fragments:".bold().underline().blue());
            for fragment in fragments {
                println!("  {}: {}", "Owner Peer UUID".yellow(), fragment.owner_id);
                println!("  {}: {}", "Storage Peer UUID".yellow(), fragment.storage_peer_id);
                println!("  {}: {}", "Session Key".yellow(), fragment.session_key);
                println!("  {}: {}", "Session".yellow(), fragment.session);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!();
            }
        },
        Err(_) => (()),
    }
}

// Console manager for use send files use tunnel class or connection class (stun or turn protocol)
pub async fn console_manager(
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
    } else if trimmed_input == "fragments" {
        print_all_fragments();
    } else if trimmed_input.starts_with("connect ") {
        let peer_id = trimmed_input.strip_prefix("connect ").unwrap();
        println!("{}", format!("[Peer] Trying to connect to peer: {}", peer_id).cyan());
        
        let packet = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "wait_connection".to_string(),
            to: None,
            data: Some(json!({
                "connect_peer_id": peer_id,
                "peer_id": GLOBAL_DB.get_or_create_peer_id().unwrap()
            })),
            status: None,
            protocol: Protocol::STUN,
        };

        if let Err(e) = connection.send_packet(packet).await {
            println!("{}", format!("[Peer] Failed to send connection request: {}", e).red());
        } else {
            println!("{}", "[Peer] Connection request sent successfully".green());
            println!("{}", "[Peer] Waiting for peer to accept connection...".yellow());
        }
    } else if is_connected {
        if (trimmed_input.starts_with("file ")) {
            let file_path = trimmed_input.strip_prefix("file ").unwrap();
            println!("{}", format!("[Peer] Sending file: {}", file_path).cyan());
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
                        status: None,
                        protocol: Protocol::TURN,
                    };
                    if let Err(e) = connection.send_packet(packet).await {
                        println!("{}", format!("[Peer] Failed to send packet: {}", e).red());
                    } else {
                        println!("{}", "[Peer] Packet sent successfully".green());
                    }
                } else if trimmed_input.starts_with("file ") {
                    let file_path = trimmed_input.strip_prefix("file ").unwrap();
                    println!("{}", format!("[Peer] Sending file: {}", file_path).cyan());
                    if let Ok(mut file) = File::open(file_path).await {
                        let mut contents = vec![];
                        file.read_to_end(&mut contents).await.unwrap();

                        let peer_upload_file = serde_json::to_value(PeerUploadFile {
                            filename: file_path.to_string(),
                            contents: base64::encode(contents),
                            peer_id: GLOBAL_DB.get_or_create_peer_id().unwrap(),
                        }).unwrap();

                        let packet = TransportPacket {
                            public_addr: format!("{}:{}", public_ip, public_port),
                            act: "save_file".to_string(),
                            to: Some(key.clone()),
                            data: Some(peer_upload_file),
                            status: None,
                            protocol: Protocol::TURN,
                        };
                        if let Err(e) = connection.send_packet(packet).await {
                            println!("{}", format!("[Peer] Failed to send packet: {}", e).red());
                        } else {
                            println!("{}", "[Peer] Packet sent successfully".green());
                        }
                    } else {
                        println!("{}", format!("[Peer] Failed to open file: {}", file_path).red());
                    }
                } else {
                    let packet = TransportPacket {
                        public_addr: format!("{}:{}", public_ip, public_port),
                        act: "message".to_string(),
                        to: Some(key.clone()),
                        data: Some(json!({"text": trimmed_input.to_string()})),
                        status: None,
                        protocol: Protocol::TURN,
                    };
                    if let Err(e) = connection.send_packet(packet).await {
                        println!("{}", format!("[Peer] Failed to send packet: {}", e).red());
                    } else {
                        println!("{}", "[Peer] Packet sent successfully".green());
                    }
                }
            }
        }
    }
}