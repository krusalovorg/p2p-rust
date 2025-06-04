use p2p_server::simulation::NetworkSimulation;
use p2p_server::config::Config;
use std::path::PathBuf;
use tokio::io::{self, AsyncBufReadExt};
use tokio::sync::Mutex;
use std::sync::Arc;
use clap::{Arg, Command};

fn create_command() -> Command {
    Command::new("P2P Simulation")
        .arg(Arg::new("servers")
            .long("servers")
            .short('s')
            .action(clap::ArgAction::Set)
            .value_name("NUMBER")
            .help("Количество сигнальных серверов")
            .default_value("3"))
        .arg(Arg::new("peers")
            .long("peers")
            .short('p')
            .action(clap::ArgAction::Set)
            .value_name("NUMBER")
            .help("Количество пиров")
            .default_value("5"))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = create_command().get_matches();
    
    let servers = matches.get_one::<String>("servers")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(3);
    
    let peers = matches.get_one::<String>("peers")
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(5);

    let mut simulation = NetworkSimulation::new();
    
    println!("Создаем {} сигнальных серверов и {} пиров", servers, peers);
    
    // Создаем сигнальные серверы
    for i in 0..servers {
        let config = Config {
            signal_server_ip: format!("127.0.0.{}", i + 1),
            signal_server_port: 8000 + i as i64,
            storage_size: 1024 * 1024 * 100, // 100 MB
            proxy_ip: "127.0.0.1".to_string(),
            proxy_port: 80 + i as i64,
        };
        
        let db_path = PathBuf::from(format!("./simulation/signal_server_{}", i));
        simulation.add_signal_server(&config, format!("signal_{}", i), db_path).await?;
        println!("Создан сигнальный сервер {} на {}:{}", i, config.signal_server_ip, config.signal_server_port);
    }

    // Создаем пиры
    for i in 0..peers {
        let config = Config {
            signal_server_ip: format!("127.0.0.{}", (i % servers) + 1),
            signal_server_port: 8000 + (i % servers) as i64,
            storage_size: 1024 * 1024 * 100, // 100 MB
            proxy_ip: "127.0.0.1".to_string(),
            proxy_port: 80 + i as i64,
        };
        let db_path = PathBuf::from(format!("./simulation/peer_{}", i));
        simulation.add_peer(&config, format!("peer_{}", i), db_path).await?;
        println!("Создан пир {} подключенный к {}:{}", i, config.signal_server_ip, config.signal_server_port);
    }

    // Запускаем симуляцию в отдельном потоке
    let simulation = Arc::new(Mutex::new(simulation));
    let simulation_clone = simulation.clone();
    
    tokio::spawn(async move {
        simulation_clone.lock().await.run_simulation().await.unwrap();
    });

    // Основной поток для управления выбранным пиром
    println!("Выберите ID пира для управления (peer_0 - peer_{}):", peers - 1);
    let stdin = io::BufReader::new(io::stdin());
    let mut lines = stdin.lines();

    while let Some(line) = lines.next_line().await? {
        let peer_id = line.trim();
        if let Some(peer) = simulation.lock().await.get_peer(peer_id) {
            println!("Управление пиром {}", peer_id);
            // Здесь можно добавить команды для управления пиром
            // Например, отправка сообщений, подключение к другим пирам и т.д.
        } else {
            println!("Пир с ID {} не найден", peer_id);
        }
    }

    Ok(())
} 