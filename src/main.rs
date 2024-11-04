use async_std::path::Path;
use async_std::task::sleep;
use std::sync::Arc;
use std::time::Duration;
use std::{env, fs};
use tokio::sync::{mpsc, Mutex};
use tokio::task;

mod config;
mod signal;
mod tunnel;
mod db;

use crate::config::Config;
use crate::signal::{SignalServer, TransportPacket};
use crate::tunnel::Tunnel;
use crate::db::{P2PDatabase, Fragment, Storage};

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

async fn run_peer() {
    let config: Config = Config::from_file("config.toml");
    let signal_server: Arc<Mutex<SignalServer>> = Arc::new(Mutex::new(SignalServer::new()));
    let tunnel = Arc::new(Mutex::new(Tunnel::new().await));

    {
        let tunnel = tunnel.lock().await;
        println!(
            "You public ip:port: {}:{}",
            tunnel.public_ip, tunnel.public_port
        );
    }

    let (tx, mut rx) = mpsc::channel(32);

    let signal_server_ip_clone = config.signal_server_ip.clone();
    let signal_server_port_clone = config.signal_server_port;
    let signal_server_clone = Arc::clone(&signal_server);

    let (tunnel_public_ip, tunnel_public_port) = {
        let tunnel_guard = tunnel.lock().await;
        (tunnel_guard.public_ip.clone(), tunnel_guard.public_port)
    };
    
    task::spawn(async move {
        let mut signal_server = signal_server_clone.lock().await;
        println!(
            "[task 1] Connecting to signal server {}:{}",
            signal_server_ip_clone, signal_server_port_clone
        );
        if let Err(e) = signal_server
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

        match signal_server
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
            match signal_server.receive_message().await {
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

    while let Some(result) = rx.recv().await {
        println!("Received: packet");
        match result {
            Ok(packet) => {
                println!("Received packet: {:?}", packet);
                if (packet.act == "wait_connection") {
                    println!("Received wait_connection packet");
                    handle_packet(packet, Arc::clone(&tunnel)).await;
                    // match SignalServer::extract_addr(packet.public_addr.clone()).await {
                    //     Ok((ip, port)) => {
                    //         println!("Extracted address: {}:{}", ip, port);
                    //         let ip = ip.to_string();
                    //         println!("Received peer info: {}:{}", ip, port);
                    //         let mut tunnel: tokio::sync::MutexGuard<'_, Tunnel> = tunnel.lock().await;
                    //         println!("Try connecting to {}:{}", ip, port);
                    //         match tunnel.make_connection(&ip, port, 10).await {
                    //             Ok(()) => {
                    //                 println!("Connection established with {}:{}!", ip, port);
                    //                 tunnel.backlife_cycle(1);

                    //                 loop {
                    //                     let mut input = String::new();
                    //                     std::io::stdin().read_line(&mut input).unwrap();
                    //                     let trimmed_input = input.trim();

                    //                     if trimmed_input.starts_with("file ") {
                    //                         let file_path =
                    //                             trimmed_input.strip_prefix("file ").unwrap();
                    //                         println!("Sending file: {}", file_path);
                    //                         tunnel.send_file_path(file_path).await;
                    //                     } else {
                    //                         tunnel.send_message(trimmed_input).await;
                    //                     }
                    //                 }
                    //             }
                    //             Err(e) => {
                    //                 println!("[STUN] Failed to make connection: {}", e)
                    //             }
                    //         }
                    //     }
                    //     Err(e) => {
                    //         println!("Failed to extract address: {}", e);
                    //     }
                    // }
                }
            }
            Err(e) => eprintln!("Failed to get peer info: {}", e),
        }
    }
}

async fn handle_packet(packet: TransportPacket, tunnel: Arc<Mutex<Tunnel>>) {
    match SignalServer::extract_addr(packet.public_addr.clone()).await {
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
                    println!("[STUN] Failed to make connection: {}", e)
                }
            }
        }
        Err(e) => {
            println!("Failed to extract address: {}", e);
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
