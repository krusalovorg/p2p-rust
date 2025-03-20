use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use colored::*;

use crate::connection::Connection;
use crate::signal::{Protocol, TransportPacket};
use crate::tunnel::Tunnel;

pub async fn turn_tunnel(
    packet: TransportPacket,
    tunnel: Arc<Mutex<Tunnel>>,
    signal: &Connection,
) -> Result<String, String> {
    let tunnel_clone = tunnel.lock().await;
    let public_ip = tunnel_clone.public_ip.clone();
    let public_port = tunnel_clone.public_port.clone();
    println!(
        "{}",
        format!("[TURN] Turn tunnel creating, sending packets.. {}", packet.act).yellow()
    );
    if packet.act == "wait_connection" {
        let packet_hello = TransportPacket {
            public_addr: format!("{}:{}", public_ip, public_port),
            act: "try_turn_connection".to_string(),
            to: Some(packet.public_addr.clone().to_string()),
            data: None,
            status: None,
            protocol: Protocol::TURN,
        };
        let result = signal.send_packet(packet_hello).await;
        println!(
            "{}",
            format!("[TURN] [try_turn_connection] Result sending socket {:?}", result).yellow()
        );
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
            status: None,
            protocol: Protocol::TURN,
        };
        println!("{}", "[TURN] [accept_connection] Sending accept connection".yellow());
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
