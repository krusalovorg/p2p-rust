use anyhow::Result;
use colored::*;

use crate::connection::Connection;
use crate::packets::{Protocol, TransportPacket};
use crate::db::P2PDatabase;

pub async fn turn_tunnel(
    packet: TransportPacket,
    signal: &Connection,
    db: &P2PDatabase,
) -> Result<String, String> {

    println!(
        "{}",
        format!("[TURN] Turn tunnel creating, sending packets.. {}", packet.act).yellow()
    );
    if packet.act == "wait_connection" {
        let packet_hello = TransportPacket {
            act: "try_turn_connection".to_string(),
            to: Some(packet.uuid.clone().to_string()),
            data: None,
            protocol: Protocol::TURN,
            uuid: db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
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
            act: "accept_connection".to_string(),
            to: Some(packet.uuid.to_string()),
            data: None,
            protocol: Protocol::TURN,
            uuid: db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
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
