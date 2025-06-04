use anyhow::Result;
use colored::*;

use crate::crypto::crypto::generate_uuid;
use crate::manager::ConnectionManager::ConnectionManager;
use crate::packets::{Protocol, TransportPacket};
use crate::db::P2PDatabase;

pub async fn turn_tunnel(
    manager: &ConnectionManager,
    packet: TransportPacket,
    db: &P2PDatabase,
) -> Result<String, String> {

    println!(
        "{}",
        format!("[TURN] Turn tunnel creating, sending packets.. {}", packet.act).yellow()
    );
    if packet.act == "wait_connection" {
        let packet_hello = TransportPacket {
            act: "try_turn_connection".to_string(),
            to: Some(packet.peer_key.clone().to_string()),
            data: None,
            protocol: Protocol::TURN,
            peer_key: db.get_or_create_peer_id().unwrap(),
            uuid: generate_uuid(),
            nodes: vec![],
            signature: None,
        };
        let result = manager.auto_send_packet(packet_hello).await;
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
            act: "accept_connection".to_string(),
            to: Some(packet.peer_key.to_string()),
            data: None,
            protocol: Protocol::TURN,
            peer_key: db.get_or_create_peer_id().unwrap(),
            uuid: generate_uuid(),
            nodes: vec![],
            signature: None,
        };
        println!("{}", "[TURN] [accept_connection] Sending accept connection".yellow());
        let result = manager.auto_send_packet(packet_hello).await;
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
