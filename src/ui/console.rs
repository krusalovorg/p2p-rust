use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::manager::ConnectionTurnStatus;
use std::io::{self, Write};
use std::sync::Arc;
use colored::*;
use crate::peer::peer_api::PeerAPI;
use crate::db::P2PDatabase;

pub fn print_all_files(db: &P2PDatabase) {
    let myfiles = db.get_myfile_fragments();
    
    let uuid_peer = db.get_or_create_peer_id().unwrap();
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

pub fn print_all_fragments(db: &P2PDatabase) {
    let fragments = db.get_storage_fragments();

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

pub fn print_all_commands() {
    println!("{}", "[Peer] Available commands:".yellow());
    println!("{}", "  files - List all your files");
    println!("{}", "  fragments - List all your fragments");
    println!("{}", "  peers - List all peers");
    println!("{}", "  connect <peer_id> - Connect to a peer");
    println!("{}", "  send_all <message> - Send a message to all peers");
    println!("{}", "  <message> - Send a message to the peer");
    println!("{}", "  get <session_key> - Get a file from the peer");
    println!("{}", "  upload <file_path> - Upload a file to the peer");
    println!("{}", "  help - Show available commands");
}

pub async fn console_manager(
    api: Arc<PeerAPI>,
    connections_turn: Arc<RwLock<HashMap<String, ConnectionTurnStatus>>>,
    db: &P2PDatabase,
) {
    let mut input = String::new();
    print!("\x1b[32m[P2P] >\x1b[0m ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut input).unwrap();
    let trimmed_input = input.trim();

    if trimmed_input == "help" {
        print_all_commands();
    } else if trimmed_input == "files" {
        print_all_files(db);
    } else if trimmed_input == "fragments" {
        print_all_fragments(db);
    } else if trimmed_input == "peers" {
        if let Err(e) = api.request_peer_list().await {
            println!("{}", format!("[Peer] Failed to request peer list: {}", e).red());
        }
    } else if trimmed_input.starts_with("connect ") {
        let peer_id = trimmed_input.strip_prefix("connect ").unwrap();
        println!("{}", format!("[Peer] Trying to connect to peer: {}", peer_id).cyan());
        
        if let Err(e) = api.connect_to_peer(peer_id.to_string()).await {
            println!("{}", format!("[Peer] Failed to connect to peer: {}", e).red());
        } else {
            println!("{}", "[Peer] Waiting for peer to accept connection...".yellow());
        }
    } else if connections_turn.read().await.len() > 0 {
        let connections_turn_clone = connections_turn.read().await;
        for (key, connection_turn_status) in connections_turn_clone.iter() {
            if connection_turn_status.connected && !connection_turn_status.turn_connection {
                if trimmed_input.starts_with("get ") {
                    let session_key = trimmed_input.strip_prefix("get ").unwrap();
                    if let Err(e) = api.get_file(key.clone(), session_key.to_string()).await {
                        println!("{}", format!("[Peer] Failed to get file: {}", e).red());
                    } else {
                        println!("{}", "[Peer] File request sent successfully".green());
                    }
                } else if trimmed_input.starts_with("upload ") {
                    let file_path = trimmed_input.strip_prefix("upload ").unwrap();
                    if let Err(e) = api.upload_file(key.clone(), file_path.to_string()).await {
                        println!("{}", format!("[Peer] Failed to upload file: {}", e).red());
                    } else {
                        println!("{}", "[Peer] File uploaded successfully".green());
                    }
                } else {
                    if let Err(e) = api.send_message(key.clone(), trimmed_input.to_string()).await {
                        println!("{}", format!("[Peer] Failed to send message: {}", e).red());
                    } else {
                        println!("{}", "[Peer] Message sent successfully".green());
                    }
                }
            }
        }
    }
}