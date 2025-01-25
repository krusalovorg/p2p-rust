use anyhow::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::env;
use tokio::sync::Mutex;

mod config;
mod connection;
mod signal;
mod tunnel;

use crate::config::Config;
use crate::connection::Connection;
use crate::signal::{Protocol, SignalClient, SignalServer, TransportPacket};
use crate::tunnel::Tunnel;

#[tokio::main]
async fn main() {
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
    let tunnel = Arc::new(Mutex::new(Tunnel::new().await));

    {
        let tunnel = tunnel.lock().await;
        println!(
            "[Peer] You public ip:port: {}:{}",
            tunnel.public_ip, tunnel.public_port
        );
    }

    let signal_server_ip_clone = config.signal_server_ip.clone();
    let signal_server_port_clone = config.signal_server_port;

    let (tunnel_public_ip, tunnel_public_port) = {
        let tunnel_guard = tunnel.lock().await;
        (tunnel_guard.public_ip.clone(), tunnel_guard.public_port)
    };

    let my_public_addr = format!("{}:{}", tunnel_public_ip, tunnel_public_port);

    let connection = Connection::new(
        signal_server_ip_clone,
        signal_server_port_clone,
        tunnel_public_ip,
        tunnel_public_port,
    )
    .await;

    let mut connections_turn: HashMap<String, ConnectionTurnStatus> = HashMap::new();

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
                            connections_turn.insert(
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
                    connections_turn.insert(
                        from_public_addr.clone(),
                        ConnectionTurnStatus {
                            connected: false,
                            turn_connection: true,
                        },
                    );
                }
                if let Some(status) = connections_turn.get_mut(&from_public_addr) {
                    println!("[Peer] [Turn] Status {:?}", status);
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
                        println!("[Peer] Connected successfully, you can process other packets")
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

async fn turn_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
    signal: &Connection,
) -> Result<String, String> {
    let tunnel_clone = tunnel.lock().await;
    let public_ip = tunnel_clone.public_ip.clone();
    let public_port = tunnel_clone.public_port.clone();
    println!("[TURN] Turn tunnel creating, sending packets.. {}", packet.act);
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
        println!("[TURN] [try_turn_connection] Result sending socket {:?}", result);
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

async fn stun_tunnel(packet: TransportPacket, tunnel: Arc<Mutex<Tunnel>>) -> Result<(), String> {
    println!(
        "[STUN] Entering stun_tunnel with public address: {:?}",
        packet.public_addr
    );
    match SignalClient::extract_addr(packet.public_addr).await {
        Ok((ip, port)) => {
            let ip = ip.to_string();
            let mut tunnel = tunnel.lock().await;
            println!("[STUN] Try connecting to {}:{}", ip, port);
            match tunnel.make_connection(&ip, port, 10).await {
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
