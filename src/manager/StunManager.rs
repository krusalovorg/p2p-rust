use crate::packets::TransportPacket;
use crate::signal::SignalClient;
use crate::tunnel::Tunnel;
use anyhow::Result;
use colored::*;

use super::ConnectionManager::ConnectionManager;

impl ConnectionManager {
    pub async fn stun_tunnel(&self, packet: TransportPacket) -> Result<(), String> {
        let mut tunnel = Tunnel::new().await;
        println!(
            "{}",
            format!(
                "[STUN] Entering stun_tunnel with public address: {:?}",
                packet.public_addr
            )
            .yellow()
        );
        let public_addr_clone = packet.public_addr.clone();
        match SignalClient::extract_addr(public_addr_clone).await {
            Ok((ip, port)) => {
                let ip = ip.to_string();
                println!(
                    "{}",
                    format!("[STUN] Try connecting to {}:{}", ip, port).yellow()
                );
                match tunnel.make_connection(&ip, port, 3).await {
                    Ok(()) => {
                        println!(
                            "{}",
                            format!("[STUN] Connection established with {}:{}!", ip, port).green()
                        );
                        tunnel.backlife_cycle(3);
                        self.add_tunnel(packet.public_addr.clone(), tunnel);
                        return Ok(());
                    }
                    Err(e) => {
                        drop(tunnel);
                        println!(
                            "{}",
                            format!("[STUN] Failed to make connection: {}", e).red()
                        );
                        return Err("[STUN] Fail connection".to_string());
                    }
                }
            }
            Err(e) => {
                drop(tunnel);
                println!(
                    "{}",
                    format!("[STUN] Failed to extract address: {}", e).red()
                );
                return Err("[STUN] Fail extract address".to_string());
            }
        }
    }
}
