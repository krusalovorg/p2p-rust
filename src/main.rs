use anyhow::Result;
// use async_std::path::Path;
use async_std::task::sleep;
use signal::Protocol;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::{
    env,
    //  fs, result
};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task;

mod config;
// mod db;
mod signal;
mod tunnel;

use crate::config::Config;
// use crate::db::{Fragment, P2PDatabase, Storage};
use crate::signal::{SignalServer, TransportPacket};
use crate::tunnel::Tunnel;

#[tokio::main]
async fn main() {
    // let db_path = Path::new("./storage/db"); //TempDir::new("storage").unwrap();
    // // let path = tempdir.path();

    // if !db_path.exists().await {
    //     fs::create_dir_all(db_path).expect("Failed to create database directory");
    // }

    // let db = P2PDatabase::new(db_path.as_ref());

    // let fragment = Fragment {
    //     uuid_peer: "peer1".to_string(),
    //     session_key: "key1".to_string(),
    //     session: "session1".to_string(),
    //     fragment: "fragment1".to_string(),
    // };

    // db.add_myfile_fragment("file1", fragment.clone());
    // db.add_myfile_fragment("file1", fragment.clone());

    // let storage = Storage {
    //     session: "session1".to_string(),
    //     session_key: "key1".to_string(),
    //     uuid_peer: "peer1".to_string(),
    //     fragment: "fragment1".to_string(),
    // };

    // db.add_storage_fragment(storage);

    // let myfile_fragments = db.get_myfile_fragments("file1");
    // println!("{:?}", myfile_fragments);

    // let storage_fragments = db.get_storage_fragments();
    // println!("{:?}", storage_fragments);

    let args: Vec<String> = env::args().collect();
    if args.contains(&"--signal".to_string()) {
        let signal_server = Arc::new(SignalServer::new());
        signal_server.run().await;
    } else {
        run_peer().await;
    }
}

#[derive(Debug, Clone)]
struct ConnectionTurnStatus {
    connected: bool,
    turn_connection: bool,
}

async fn run_peer() {
    let config: Config = Config::from_file("config.toml");
    let signal_server = Arc::new(RwLock::new(SignalServer::new()));
    let tunnel = Arc::new(Mutex::new(Tunnel::new().await));

    {
        let tunnel = tunnel.lock().await;
        println!(
            "You public ip:port: {}:{}",
            tunnel.public_ip, tunnel.public_port
        );
    }

    let (tx, mut rx) = mpsc::channel(16);

    let signal_server_ip_clone = config.signal_server_ip.clone();
    let signal_server_port_clone = config.signal_server_port;

    let (tunnel_public_ip, tunnel_public_port) = {
        let tunnel_guard = tunnel.lock().await;
        (tunnel_guard.public_ip.clone(), tunnel_guard.public_port)
    };
    let signal_server_clone = Arc::clone(&signal_server);

    task::spawn(async move {

        println!(
            "[task 1] Connecting to signal server {}:{}",
            signal_server_ip_clone, signal_server_port_clone
        );
        if let Err(e) = signal_server_clone
            .write()
            .await
            .connect(
                &signal_server_ip_clone,
                signal_server_port_clone,
                &tunnel_public_ip,
                tunnel_public_port,
            )
            .await
        {
            println!("Failed to connect to signal server: {}", e);
            return;
        }
        sleep(Duration::from_millis(100)).await;

        match signal_server_clone
            .write()
            .await
            .send_peer_info_request(&tunnel_public_ip, tunnel_public_port)
            .await
        {
            Ok(_) => {
                println!("Packet success sended");
            }
            Err(e) => {
                println!("Failed to send peer info request: {}", e);
            }
        }

        loop {
            match signal_server_clone.write().await.receive_message().await {
                Ok(message) => {
                    println!("Received message: {:?}", message.act);
                    tx.send(Ok(message)).await.unwrap();
                }
                Err(e) => {
                    if e == "Socket is not connected" {
                        println!("Socket is not connected");
                        sleep(Duration::from_secs(1)).await;
                        break;
                    }
                    tx.send(Err(e)).await.unwrap();
                    break;
                }
            }
        }
    });

    let processing = task::spawn(async move {
        let mut connections_turn: HashMap<String, ConnectionTurnStatus> = HashMap::new();
        while let Some(result) = rx.recv().await {
            println!("Received: {:?}", result);
            match result {
                Ok(packet) => {
                    println!("Received packet: {:?}", packet);
                    if packet.act == "wait_connection" {
                        // этап ожидания подключения от пира, у нас есть данные: его пуб айпи, получатель (то-есть мы), протокол
                        println!("Received wait_connection packet");
                        let public_addr = packet.public_addr.clone();
                        let packet_clone = packet.clone();
                        let protocol_connection = packet.protocol.clone();
                        println!("Protocol connection: {:?}", protocol_connection);
                        if protocol_connection == Protocol::STUN {
                            println!("Start stun tunnel");
                            let result_tunnel = stun_tunnel(packet, Arc::clone(&tunnel)).await;
                            match result_tunnel {
                                Ok(_) => {
                                    println!("Connection established!");
                                }
                                Err(e) => {
                                    connections_turn.insert(
                                        public_addr.clone(),
                                        ConnectionTurnStatus {
                                            connected: false,
                                            turn_connection: true,
                                        },
                                    );
                                    println!("Failed to establish connection: {}", e);
                                }
                            }
                        } else if protocol_connection == Protocol::TURN {
                            connections_turn.insert(
                                public_addr.clone(),
                                ConnectionTurnStatus {
                                    connected: false,
                                    turn_connection: true,
                                },
                            );
                        }
                        if let Some(status) = connections_turn.get_mut(&public_addr) {
                            println!("Status connection turn: {:?}", status);
                            if status.turn_connection && !status.connected {
                                let packet_hello = TransportPacket {
                                    public_addr: "192.168.0.21:8080".to_string(),
                                    act: "accept_connection".to_string(),
                                    to: Some("192.168.0.21:8080".to_string()),
                                    data: None,
                                    session_key: None,
                                    status: None,
                                    protocol: Protocol::TURN,
                                };
                                let resultt: std::result::Result<(), String> = signal_server
                                    .write()
                                    .await
                                    .send_turn_message(packet_hello)
                                    .await;

                                let result_turn_tunnel = turn_tunnel(
                                    packet_clone,
                                    Arc::clone(&tunnel),
                                    Arc::clone(&signal_server),
                                )
                                .await;
                                println!("Result turn tunnel {:?}", result_turn_tunnel);
                                match result_turn_tunnel {
                                    Ok(r) => {
                                        println!("Result turn tunnel: {}", r);
                                        if r == "successful_connection" {
                                            println!("Connection established!");
                                            status.connected = true;
                                            status.turn_connection = false;
                                        } else if r == "send_wait_connection" {
                                            println!("Wait answer acception connection...");
                                        }
                                    }
                                    Err(e) => {
                                        status.connected = false;
                                        status.turn_connection = true;
                                        println!("Fail: {}", e);
                                    }
                                }
                            } else {
                                println!("Подключено успешно, можно обрабатывать прочие сообщения")
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Failed to get peer info: {}", e),
            }
        }
    });

    let _ = processing.await;
}

async fn turn_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
    signal: Arc<RwLock<SignalServer>>,
) -> Result<String, String> {
    let tunnel_clone = tunnel.lock().await;
    let public_ip = tunnel_clone.public_ip.clone();
    let public_port = tunnel_clone.public_port.clone();
    println!("Turn tunnel creating, sending packets.. {}", packet.act);
    if packet.act == "wait_connection" {
        println!("Act {}", packet.act);
        let packet_hello = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "try_turn_connection".to_string(),
            to: Some(packet.public_addr.clone().to_string()),
            data: None,
            session_key: None,
            status: None,
            protocol: Protocol::TURN,
        };
        println!("packet for sending: {:?}", packet);
        let result = signal
            .read_owned()
            .await
            .send_turn_message(packet_hello)
            .await;
        println!("Result sending socket {:?}", result);
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
        let result: std::result::Result<(), String> = signal
            .read_owned()
            .await
            .send_turn_message(packet_hello)
            .await;
        match result {
            Ok(_) => {
                return Ok("successful_connection".to_string());
            }
            Err(e) => {
                return Err(e);
            }
        }
    }
    return Err("Peer didn't give the connection agreement".to_string());
}

async fn stun_tunnel(packet: TransportPacket, tunnel: Arc<Mutex<Tunnel>>) -> Result<(), String> {
    println!(
        "Entering stun_tunnel with public address: {:?}",
        packet.public_addr
    );
    match SignalServer::extract_addr(packet.public_addr).await {
        Ok((ip, port)) => {
            println!("Extracted address: {}:{}", ip, port);
            let ip = ip.to_string();
            println!("Received peer info: {}:{}", ip, port);
            let mut tunnel = tunnel.lock().await;
            println!("Try connecting to {}:{}", ip, port);
            match tunnel.make_connection(&ip, port, 10).await {
                Ok(()) => {
                    println!("Connection established with {}:{}!", ip, port);
                    tunnel.backlife_cycle(1);

                    loop {
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).unwrap();
                        let trimmed_input = input.trim();

                        if trimmed_input.starts_with("file ") {
                            let file_path = trimmed_input.strip_prefix("file ").unwrap();
                            println!("Sending file: {}", file_path);
                            tunnel.send_file_path(file_path).await;
                        } else {
                            tunnel.send_message(trimmed_input).await;
                        }
                    }
                }
                Err(e) => {
                    println!("[STUN] Failed to make connection: {}", e);
                    return Err("fail connection".to_string());
                }
            }
        }
        Err(e) => {
            println!("Failed to extract address: {}", e);
            return Err("fail extract address".to_string());
        }
    }
}

// use std::{str};

// mod tunnel;
// use crate::tunnel::Tunnel;

// #[tokio::main]
// async fn main() {
//     let mut tunnel = Tunnel::new().await;
//     println!(
//         "Send this to your friend {}:{}",
//         tunnel.public_ip, tunnel.public_port
//     );
//     //try connecting to your friend
//     println!("Enter your friend's IP:Port");
//     let mut input = String::new();
//     std::io::stdin().read_line(&mut input).unwrap();
//     println!("Connecting to {}", input.trim());
//     let parts: Vec<&str> = input.trim().split(':').collect();
//     let ip = parts[0];
//     let port: u16 = parts[1].parse().unwrap();
//     tunnel.make_connection(ip, port, 10).await;
//     println!("Connection established with {}:{}!", ip, port);
//     tunnel.backlife_cycle(1);

//     loop {
//         let mut input = String::new();
//         std::io::stdin().read_line(&mut input).unwrap();
//         let trimmed_input = input.trim();

//         if trimmed_input.starts_with("file ") {
//             let file_path = trimmed_input.strip_prefix("file ").unwrap();
//             tunnel.send_file_path(file_path).await;
//         } else {
//             tunnel.send_message(trimmed_input).await;
//         }
//     }
// }
