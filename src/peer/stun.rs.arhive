use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use colored::*;
use crate::packets::TransportPacket;
use crate::signal::SignalClient;
use crate::tunnel::Tunnel;

pub async fn stun_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
) -> Result<(), String> {
    println!(
        "{}",
        format!("[STUN] Entering stun_tunnel with public address: {:?}", packet.public_addr).yellow()
    );
    match SignalClient::extract_addr(packet.public_addr).await {
        Ok((ip, port)) => {
            let ip = ip.to_string();
            let mut tunnel = tunnel.lock().await;
            println!("{}", format!("[STUN] Try connecting to {}:{}", ip, port).yellow());
            match tunnel.make_connection(&ip, port, 3).await {
                Ok(()) => {
                    println!("{}", format!("[STUN] Connection established with {}:{}!", ip, port).green());
                    tunnel.backlife_cycle(1);

                    loop {
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input).unwrap();
                        let trimmed_input = input.trim();

                        if trimmed_input.starts_with("file ") {
                            let file_path = trimmed_input.strip_prefix("file ").unwrap();
                            println!("{}", format!("[STUN] Sending file: {}", file_path).cyan());
                            tunnel.send_file_path(file_path).await;
                        } else {
                            tunnel.send_message(trimmed_input).await;
                        }
                    }
                }
                Err(e) => {
                    println!("{}", format!("[STUN] Failed to make connection: {}", e).red());
                    return Err("[STUN] Fail connection".to_string());
                }
            }
        }
        Err(e) => {
            println!("{}", format!("[STUN] Failed to extract address: {}", e).red());
            return Err("[STUN] Fail extract address".to_string());
        }
    }
}
