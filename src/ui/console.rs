use crate::crypto::token::get_metadata_from_token;
use crate::db::P2PDatabase;
use crate::manager::ConnectionTurnStatus;
use crate::peer::peer_api::PeerAPI;
use colored::*;
use hex;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

pub fn print_all_files(db: &P2PDatabase) {
    let myfiles = db.get_myfile_fragments();

    let uuid_peer = db.get_or_create_peer_id().unwrap();
    println!("{}", format!("[Peer] UUID: {}", uuid_peer).yellow());

    match myfiles {
        Ok(myfiles) => {
            println!("{}", "My Files:".bold().underline().blue());
            for (key, fragment) in myfiles {
                println!("  {}: {}", "UUID Peer".yellow(), fragment.uuid_peer);
                println!("  {}: {}", "Token".yellow(), fragment.token);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!("  {}: {}", "UUID Fragment (key db)".yellow(), key);
                println!();
            }
        }
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
                println!(
                    "  {}: {}",
                    "Storage Peer UUID".yellow(),
                    fragment.storage_peer_id
                );
                println!("  {}: {}", "Token".yellow(), fragment.token);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!();
            }
        }
        Err(_) => (()),
    }
}

pub async fn print_tokens_info(db: &P2PDatabase) {
    let tokens = match db.get_all_tokens() {
        Ok(tokens) => tokens,
        Err(e) => {
            println!(
                "{}",
                format!("[Peer] Ошибка при получении токенов: {}", e).red()
            );
            return;
        }
    };

    println!(
        "\n{}",
        "╔════════════════════════════════════════════════════════════╗".yellow()
    );
    println!(
        "{}",
        "║                    ИНФОРМАЦИЯ О ТОКЕНАХ                    ║".yellow()
    );
    println!(
        "{}",
        "╠════════════════════════════════════════════════════════════╣".yellow()
    );

    if tokens.is_empty() {
        println!(
            "{}",
            "║ Нет доступных токенов                                      ║".red()
        );
    } else {
        for (peer_id, token_info) in tokens {
            println!(
                "{}",
                "╠════════════════════════════════════════════════════════════╣".yellow()
            );
            println!(
                "{} {}",
                "║ Peer ID:".cyan(),
                format!("{:>40} ║", peer_id).white()
            );
            println!(
                "{} {}",
                "║ Токен:".cyan(),
                format!("{:>40} ║", token_info.token).white()
            );
            println!(
                "{} {}",
                "║ Свободное место:".cyan(),
                format!("{:>32} байт ║", token_info.free_space).white()
            );

            let timestamp = token_info.timestamp;
            let datetime = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp);
            let datetime_str = format!("{:?}", datetime);
            println!(
                "{} {}",
                "║ Время создания:".cyan(),
                format!("{:>35} ║", datetime_str).white()
            );

            // Пытаемся получить метаданные токена
            if let Ok(metadata) = get_metadata_from_token(token_info.token.clone()).await {
                println!(
                    "{} {}",
                    "║ Размер файла:".cyan(),
                    format!("{:>37} байт ║", metadata.file_size).white()
                );
                println!(
                    "{} {}",
                    "║ Провайдер:".cyan(),
                    format!("{:>41} ║", metadata.storage_provider).white()
                );
                println!(
                    "{} {}",
                    "║ Подпись:".cyan(),
                    format!("{:>43} ║", hex::encode(&metadata.signature)).white()
                );
            } else {
                println!(
                    "{}",
                    "║ Не удалось получить метаданные токена                ║".red()
                );
            }
        }
    }

    println!(
        "{}",
        "╚════════════════════════════════════════════════════════════╝".yellow()
    );
    println!();
}

pub fn print_all_commands() {
    println!("{}", "[Peer] Available commands:".yellow());
    println!("{}", "  files - List all your files");
    println!("{}", "  fragments - List all your fragments");
    println!("{}", "  tokens - Show all tokens and their metadata");
    println!("{}", "  peers - List all peers");
    println!("{}", "  search_peer <peer_id> - Search for a specific peer");
    println!("{}", "  connect <peer_id> - Connect to a peer");
    println!("{}", "  send_all <message> - Send a message to all peers");
    println!("{}", "  <message> - Send a message to the peer");
    println!("{}", "  get <session_key> - Get a file from the peer");
    println!("{}", "  upload <file_path> - Upload a file to the peer");
    println!(
        "{}",
        "  reserve <size_in_bytes> - Reserve storage space on peers"
    );
    println!("{}", "  valid_token <token> - Validate a storage token");
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
    } else if trimmed_input == "tokens" {
        print_tokens_info(db).await;
    } else if trimmed_input == "peers" {
        if let Err(e) = api.request_peer_list().await {
            println!(
                "{}",
                format!("[Peer] Failed to request peer list: {}", e).red()
            );
        }
    } else if trimmed_input.starts_with("search_peer ") {
        let peer_id = trimmed_input.strip_prefix("search_peer ").unwrap();
        if let Err(e) = api.search_peer(peer_id.to_string()).await {
            println!("{}", format!("[Peer] Failed to search peer: {}", e).red());
        }
    } else if trimmed_input.starts_with("reserve ") {
        let size_str = trimmed_input.strip_prefix("reserve ").unwrap();
        match size_str.parse::<u64>() {
            Ok(size) => {
                if let Err(e) = api.reserve_storage(size).await {
                    println!(
                        "{}",
                        format!("[Peer] Failed to reserve storage: {}", e).red()
                    );
                } else {
                    println!(
                        "{}",
                        format!("[Peer] Storage reservation request sent for {} bytes", size)
                            .green()
                    );
                }
            }
            Err(_) => println!(
                "{}",
                "[Peer] Invalid size format. Please provide a number in bytes.".red()
            ),
        }
    } else if trimmed_input.starts_with("valid_token ") {
        let token = trimmed_input.strip_prefix("valid_token ").unwrap();
        if let Err(e) = api.valid_token(token.to_string()).await {
            println!(
                "{}",
                format!("[Peer] Failed to validate token: {}", e).red()
            );
        } else {
            println!("{}", format!("[Peer] Token validated successfully").green());
        }
    } else if trimmed_input.starts_with("connect ") {
        let peer_id = trimmed_input.strip_prefix("connect ").unwrap();
        println!(
            "{}",
            format!("[Peer] Trying to connect to peer: {}", peer_id).cyan()
        );

        if let Err(e) = api.connect_to_peer(peer_id.to_string()).await {
            println!(
                "{}",
                format!("[Peer] Failed to connect to peer: {}", e).red()
            );
        } else {
            println!(
                "{}",
                "[Peer] Waiting for peer to accept connection...".yellow()
            );
        }
    } else if trimmed_input.starts_with("get ") {
        let filename = trimmed_input.strip_prefix("get ").unwrap();
        if let Err(e) = api.get_file(filename.to_string()).await {
            println!(
                "{}",
                format!("[Peer] Failed to get file {}: {}", filename, e).red()
            );
        } else {
            println!(
                "{}",
                format!("[Peer] File {} request sent successfully", filename).green()
            );
        }
    } else if trimmed_input.starts_with("upload ") {
        let file_path = trimmed_input.strip_prefix("upload ").unwrap();
        if let Err(e) = api.upload_file(file_path.to_string()).await {
            println!("{}", format!("[Peer] Failed to upload file: {}", e).red());
        }
    } else {
        let connections = connections_turn.read().await;
        for (peer_id, _) in connections.iter() {
            if let Err(e) = api
                .send_message(peer_id.clone(), trimmed_input.to_string())
                .await
            {
                println!(
                    "{}",
                    format!("[Peer] Failed to send message to {}: {}", peer_id, e).red()
                );
            }
        }
    }
}
