use crate::crypto::token::get_metadata_from_token;
use crate::db::P2PDatabase;
use crate::manager::ConnectionTurnStatus;
use crate::peer::peer_api::PeerAPI;
use colored::*;
use dashmap::DashMap;
use hex;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::SystemTime;

pub fn print_welcome() {
    println!(
        "\n{}",
        r#"
    ██╗     ██╗████████╗███████╗    ██████╗ ███████╗███████╗██████╗ 
    ██║     ██║╚══██╔══╝██╔════╝    ██╔══██╗██╔════╝██╔════╝██╔══██╗
    ██║     ██║   ██║   █████╗      ██████╔╝█████╗  █████╗  ██████╔╝
    ██║     ██║   ██║   ██╔══╝      ██╔═══╝ ██╔══╝  ██╔══╝  ██╔══██╗
    ███████╗██║   ██║   ███████╗    ██║     ███████╗███████╗██║  ██║
    ╚══════╝╚═╝   ╚═╝   ╚══════╝    ╚═╝     ╚══════╝╚══════╝╚═╝  ╚═╝
    "#
        .bright_blue()
    );
    println!(
        "{}",
        "                    P2P File Sharing Network".bright_cyan()
    );
    println!("{}", "                    Version 2.1.2".bright_yellow());
    println!(
        "\n{}",
        "Добро пожаловать в LITE PEER TO LITE NODE!".bright_green()
    );
    println!(
        "{}",
        "Введите 'help' для просмотра доступных команд\n".bright_white()
    );
}

pub fn print_all_files(db: &P2PDatabase) {
    let myfiles = db.get_my_fragments();

    let uuid_peer = db.get_or_create_peer_id().unwrap();
    println!("{}", format!("[Peer] UUID: {}", uuid_peer).yellow());

    match myfiles {
        Ok(myfiles) => {
            println!("{}", "My Files:".bold().underline().blue());
            for fragment in myfiles {
                println!(
                    "  {}: {}",
                    "Storage Peer UUID".yellow(),
                    fragment.storage_peer_key
                );
                println!("  {}: {}", "Token".yellow(), fragment.token);
                println!("  {}: {}", "Filename".yellow(), fragment.filename);
                println!(
                    "  {}: {}",
                    "UUID Fragment (key db)".yellow(),
                    fragment.file_hash
                );
                println!("  {}: {}", "Size".yellow(), fragment.size);
                println!("  {}: {}", "Mime".yellow(), fragment.mime);
                println!("  {}: {}", "Public".yellow(), fragment.public);
                println!("  {}: {}", "Encrypted".yellow(), fragment.encrypted);
                println!("  {}: {}", "Compressed".yellow(), fragment.compressed);
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
                println!("  {}: {}", "Owner Peer UUID".yellow(), fragment.owner_key);
                println!(
                    "  {}: {}",
                    "Storage Peer UUID".yellow(),
                    fragment.storage_peer_key
                );
                println!("  {}: {}", "File Hash".yellow(), fragment.file_hash);
                println!("  {}: {}", "Mime".yellow(), fragment.mime);
                println!("  {}: {}", "Public".yellow(), fragment.public);
                println!("  {}: {}", "Encrypted".yellow(), fragment.encrypted);
                println!("  {}: {}", "Compressed".yellow(), fragment.compressed);
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
            println!(
                "{} {}",
                "║ Занятое место:".cyan(),
                format!("{:>34} байт ║", token_info.used_space).white()
            );
            println!(
                "{} {}",
                "║ Доступно:".cyan(),
                format!(
                    "{:>38} байт ║",
                    token_info.free_space - token_info.used_space
                )
                .white()
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
    let width = 75;
    let horizontal_line = "═".repeat(width - 2);
    let top_border = format!("╔{}╗", horizontal_line);
    let middle_border = format!("╠{}╣", horizontal_line);
    let bottom_border = format!("╚{}╝", horizontal_line);

    let commands = vec![
        ("files", "Список всех ваших файлов"),
        ("fragments", "Список всех ваших фрагментов"),
        ("tokens", "Показать все токены и их метаданные"),
        ("peers", "Список всех пиров"),
        (
            "virtual_storage",
            "Интерактивное управление виртуальным хранилищем",
        ),
        (
            "sync_fragments",
            "Синхронизировать метаданные фрагментов с сервером",
        ),
        ("search_peer <peer_id>", "Поиск конкретного пира"),
        ("connect <peer_id>", "Подключиться к пиру"),
        ("send_all <message>", "Отправить сообщение всем пирам"),
        ("<message>", "Отправить сообщение пиру"),
        ("get <session_key>", "Получить файл от пира"),
        ("upload <file_path>", "Загрузить файл на пир"),
        (
            "update <file_hash> <file_path>",
            "Обновить существующий файл",
        ),
        ("reserve <size_in_bytes>", "Зарезервировать место на пирах"),
        ("valid_token <token>", "Проверить токен хранилища"),
        (
            "set_public <file_hash> <true/false>",
            "Изменить публичный доступ к файлу",
        ),
        ("delete <file_hash>", "Удалить файл"),
        ("call_contract <contract_hash> <function_name> <data>", "Вызвать контракт с указанными данными"),
        ("get_private_key", "Получить приватный ключ"),
        ("help", "Показать доступные команды"),
    ];

    println!("\n{}", top_border.bright_blue());
    println!(
        "{}",
        format!("║{:^width$}║", "Доступные команды:", width = width - 2).bright_blue()
    );
    println!("{}", middle_border.bright_blue());

    for (cmd, desc) in commands {
        println!(
            "{}",
            format!(
                "║  {:<30} - {:<25} ║",
                cmd.bright_green(),
                desc.bright_white()
            )
            .bright_blue()
        );
    }

    println!("{}", bottom_border.bright_blue());
    println!();
}

pub async fn console_manager(
    api: Arc<PeerAPI>,
    connections_turn: Arc<DashMap<String, ConnectionTurnStatus>>,
    db: &P2PDatabase,
) {
    let mut input = String::new();
    print!("\x1b[32m[LP2LP] >\x1b[0m ");
    io::stdout().flush().unwrap();
    std::io::stdin().read_line(&mut input).unwrap();
    let trimmed_input = input.trim();

    if trimmed_input == "help" {
        print_all_commands();
    } else if trimmed_input == "get_private_key" {
        let private_key = hex::encode(db.get_private_key().unwrap().to_bytes());
        println!("{}", format!("[Peer] Приватный ключ: {}", private_key).green());
    } else if trimmed_input.starts_with("update ") {
        let args: Vec<&str> = trimmed_input.split_whitespace().collect();
        if args.len() != 3 {
            println!(
                "{}",
                "[Peer] Использование: update <file_hash> <путь_к_новому_файлу>".red()
            );
            return;
        }
        let file_hash = args[1].to_string();
        let new_file_path = args[2].to_string();

        println!("\n{}", "Настройка параметров обновления:".cyan());

        print!("{}", "Шифровать файл? (y/n): ".yellow());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let encrypted = input.trim().to_lowercase() == "y";

        print!("{}", "Сделать файл публичным? (y/n): ".yellow());
        io::stdout().flush().unwrap();
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        let public = input.trim().to_lowercase() == "y";

        print!(
            "{}",
            "Декомпрессировать файл после загрузки? (y/n): ".yellow()
        );
        io::stdout().flush().unwrap();
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        let decompress = input.trim().to_lowercase() == "y";

        println!("\n{}", "Параметры обновления:".cyan());
        println!(
            "{}",
            format!(
                "Шифрование: {}",
                if encrypted {
                    "включено"
                } else {
                    "выключено"
                }
            )
            .yellow()
        );
        println!(
            "{}",
            format!("Публичный доступ: {}", if public { "да" } else { "нет" }).yellow()
        );
        println!(
            "{}",
            format!("Декомпрессия: {}", if decompress { "да" } else { "нет" }).yellow()
        );
        println!();

        if let Err(e) = api
            .update_file(file_hash, new_file_path, encrypted, public, decompress)
            .await
        {
            println!(
                "{}",
                format!("[Peer] Ошибка при обновлении файла: {}", e).red()
            );
        } else {
            println!("{}", "[Peer] Файл успешно обновлен".green());
        }
    } else if trimmed_input == "virtual_storage" {
        if let Err(e) = api.virtual_storage_interactive().await {
            println!(
                "{}",
                format!("[Peer] Ошибка в виртуальном хранилище: {}", e).red()
            );
        }
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
    } else if trimmed_input == "sync_fragments" {
        if let Err(e) = api.sync_fragment_metadata().await {
            println!(
                "{}",
                format!("[Peer] Ошибка при синхронизации фрагментов: {}", e).red()
            );
        } else {
            println!("{}", "[Peer] Синхронизация фрагментов запущена".green());
        }
    } else if trimmed_input.starts_with("set_public ") {
        let args: Vec<&str> = trimmed_input.split_whitespace().collect();
        if args.len() != 3 {
            println!(
                "{}",
                "[Peer] Использование: set_public <file_hash> <true/false>".red()
            );
            return;
        }
        let file_hash = args[1].to_string();
        let public = match args[2].to_lowercase().as_str() {
            "true" => true,
            "false" => false,
            _ => {
                println!(
                    "{}",
                    "[Peer] Значение public должно быть true или false".red()
                );
                return;
            }
        };
        if let Err(e) = api.change_file_public_access(file_hash, public).await {
            println!(
                "{}",
                format!("[Peer] Ошибка при изменении доступа: {}", e).red()
            );
        }
    } else if trimmed_input.starts_with("delete ") {
        let file_hash = trimmed_input.strip_prefix("delete ").unwrap();
        if let Err(e) = api.delete_file(file_hash.to_string()).await {
            println!(
                "{}",
                format!("[Peer] Ошибка при удалении файла: {}", e).red()
            );
        }
    } else if trimmed_input.starts_with("call_contract ") {
        let args: Vec<&str> = trimmed_input.split_whitespace().collect();
        if args.len() < 4 {
            println!(
                "{}",
                "[Peer] Использование: call_contract <contract_hash> <function_name> <data>".red()
            );
            return;
        }
        
        let contract_hash = args[1].to_string();
        let function_name = args[2].to_string();
        let data = args[3..].join(" ");

        println!("{}", format!("[Peer] Вызов контракта {} функции {} с данными: {}", contract_hash, function_name, data).cyan());
        
        if let Err(e) = api.call_contract(contract_hash, function_name, data.into_bytes()).await {
            println!(
                "{}",
                format!("[Peer] Ошибка при вызове контракта: {}", e).red()
            );
        } else {
            println!("{}", "[Peer] Контракт успешно вызван".green());
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
        let file_path = trimmed_input.strip_prefix("upload ").unwrap().trim();
        if file_path.is_empty() {
            println!("{}", "[Peer] Использование: upload <путь_к_файлу>".red());
            return;
        }

        let path = std::path::Path::new(file_path);
        let is_directory = path.is_dir();

        if is_directory {
            print!(
                "{}",
                "Вы указали путь к директории. Загрузить все файлы? (y/n): ".yellow()
            );
            io::stdout().flush().unwrap();
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            if input.trim().to_lowercase() != "y" {
                println!("{}", "Загрузка отменена".red());
                return;
            }
        } else {
            if let Some(ext) = path.extension() {
                if ext == "wasm" {
                    print!(
                        "{}",
                        "Обнаружен WASM файл. Загрузить как контракт? (y/n): ".yellow()
                    );
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).unwrap();
                    if input.trim().to_lowercase() == "y" {
                        if let Err(e) = api.upload_contract(file_path.to_string()).await {
                            println!(
                                "{}",
                                format!("[Peer] Ошибка при загрузке контракта: {}", e).red()
                            );
                        } else {
                            println!(
                                "{}",
                                "[Peer] Запрос на загрузку контракта отправлен".green()
                            );
                        }
                        return;
                    }
                }
            }
        }

        println!("\n{}", "Настройка параметров загрузки:".cyan());

        print!("{}", "Шифровать файлы? (y/n): ".yellow());
        io::stdout().flush().unwrap();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let encrypted = input.trim().to_lowercase() == "y";

        print!("{}", "Сделать файлы публичными? (y/n): ".yellow());
        io::stdout().flush().unwrap();
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        let public = input.trim().to_lowercase() == "y";

        print!(
            "{}",
            "Декомпрессировать файлы после загрузки? (y/n): ".yellow()
        );
        io::stdout().flush().unwrap();
        input.clear();
        std::io::stdin().read_line(&mut input).unwrap();
        let decompress = input.trim().to_lowercase() == "y";

        println!("\n{}", "Параметры загрузки:".cyan());
        println!(
            "{}",
            format!(
                "Шифрование: {}",
                if encrypted {
                    "включено"
                } else {
                    "выключено"
                }
            )
            .yellow()
        );
        println!(
            "{}",
            format!("Публичный доступ: {}", if public { "да" } else { "нет" }).yellow()
        );
        println!(
            "{}",
            format!("Декомпрессия: {}", if decompress { "да" } else { "нет" }).yellow()
        );
        println!();

        if is_directory {
            if let Err(e) = api
                .upload_directory(file_path.to_string(), encrypted, public, decompress)
                .await
            {
                println!(
                    "{}",
                    format!("[Peer] Ошибка при загрузке директории: {}", e).red()
                );
            }
        } else {
            if let Err(e) = api
                .upload_file(
                    file_path.to_string(),
                    encrypted,
                    public,
                    decompress,
                    "",
                    false,
                )
                .await
            {
                println!(
                    "{}",
                    format!("[Peer] Ошибка при загрузке файла: {}", e).red()
                );
            } else {
                println!("{}", "[Peer] Запрос на загрузку файла отправлен".green());
            }
        }
    } else if connections_turn.len() > 0 {
        let connections = connections_turn.iter();
        let mut available_peers: Vec<String> = Vec::new();

        // Собираем список доступных пиров
        for entry in connections {
            if entry.key() != &api.db.get_or_create_peer_id().unwrap() {
                available_peers.push(entry.key().clone());
            }
        }

        if available_peers.is_empty() {
            println!("{}", "Нет доступных пиров для отправки сообщения".red());
            return;
        }

        // Выводим список доступных пиров
        println!("Доступные пиры для отправки сообщения:");
        for (i, peer) in available_peers.iter().enumerate() {
            println!("{}. {}", i + 1, peer);
        }

        // Запрашиваем выбор пира
        print!("Выберите номер пира для отправки сообщения: ");
        io::stdout().flush().unwrap();

        let mut choice = String::new();
        io::stdin().read_line(&mut choice).unwrap();
        let choice: usize = match choice.trim().parse() {
            Ok(num) => num,
            Err(_) => {
                println!("{}", "Неверный ввод. Пожалуйста, введите номер пира.".red());
                return;
            }
        };

        if choice < 1 || choice > available_peers.len() {
            println!("{}", "Неверный номер пира.".red());
            return;
        }

        let selected_peer = &available_peers[choice - 1];

        if let Err(e) = api
            .send_message(selected_peer.clone(), trimmed_input.to_string())
            .await
        {
            println!(
                "{}",
                format!(
                    "[Peer] Не удалось отправить сообщение {}: {}",
                    selected_peer, e
                )
                .red()
            );
        } else {
            println!("Сообщение успешно отправлено пиру: {}", selected_peer);
        }
    } else {
        println!(
            "{}",
            format!("[Peer] Команда '{}' не найдена", trimmed_input).red()
        );
    }
}
