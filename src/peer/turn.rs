use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use colored::*;

use crate::connection::Connection;
use crate::packets::{Protocol, TransportPacket};
use crate::tunnel::Tunnel;
use crate::GLOBAL_DB;

pub async fn turn_tunnel(
    packet: TransportPacket,
    my_public_addr: Arc<String>,
    signal: &Connection,
) -> Result<String, String> {

    println!(
        "{}",
        format!("[TURN] Turn tunnel creating, sending packets.. {}", packet.act).yellow()
    );
    if packet.act == "wait_connection" {
        let packet_hello = TransportPacket {
            public_addr: my_public_addr.clone().to_string(),
            act: "try_turn_connection".to_string(),
            to: Some(packet.public_addr.clone().to_string()),
            data: None,
            status: None,
            protocol: Protocol::TURN,
            uuid: GLOBAL_DB.get_or_create_peer_id().unwrap(),
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
            public_addr: my_public_addr.clone().to_string(),
            act: "accept_connection".to_string(),
            to: Some(packet.public_addr.to_string()),
            data: None,
            status: None,
            protocol: Protocol::TURN,
            uuid: GLOBAL_DB.get_or_create_peer_id().unwrap(),
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
