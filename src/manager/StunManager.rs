use crate::connection::Connection;
use crate::packets::{PeerWaitConnection, Protocol, TransportData, TransportPacket};
use crate::tunnel::Tunnel;
use anyhow::Result;

use super::types::PeerOpenNetInfo;
use super::ConnectionManager::ConnectionManager;

impl ConnectionManager {
    pub async fn send_wait_connection(
        &self,
        target_uuid: String,
        server_conn: &Connection,
        my_key: String,
    ) -> Result<(), String> {
        println!("[DEBUG] Starting send_wait_connection");
        println!("[DEBUG] Target UUID: {}", target_uuid);
        println!("[DEBUG] My Key: {}", my_key);

        let tunnel = Tunnel::new().await;
        println!("[DEBUG] Created new tunnel with IP: {} and Port: {}", tunnel.public_ip, tunnel.public_port);

        let net_info = PeerWaitConnection {
            connect_peer_id: target_uuid.clone(),
            public_ip: tunnel.public_ip.clone(),
            public_port: tunnel.public_port,
        };

        println!("[DEBUG] Created net_info: {:?}", net_info);

        self.connections_stun.lock().await.insert(
            target_uuid.clone(),
            PeerOpenNetInfo {
                ip: net_info.public_ip.clone(),
                port: net_info.public_port,
            },
        );

        println!("[DEBUG] Added connection to STUN connections map");

        self.add_tunnel(target_uuid.clone(), tunnel).await;
        println!("[DEBUG] Added tunnel to tunnels map");

        let packet = TransportPacket {
            act: "accept_connection".to_string(),
            to: Some(target_uuid.clone()),
            data: Some(TransportData::PeerWaitConnection(net_info)),
            protocol: Protocol::STUN,
            uuid: my_key.clone(),
        };

        println!("[DEBUG] Sending accept_connection packet: {:?}", packet);

        server_conn
            .send_packet(packet)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn receive_accept_connection(
        &self,
        packet: TransportPacket,
        my_key: String,
    ) -> Result<(), String> {
        println!("[DEBUG] Starting receive_accept_connection");
        println!("[DEBUG] Received packet: {:?}", packet);
        println!("[DEBUG] My Key: {}", my_key);

        if let Some(TransportData::PeerWaitConnection(data)) = packet.data {
            let ip = data.public_ip.clone();
            let port = data.public_port;
            println!("[DEBUG] Target IP: {}, Port: {}", ip, port);

            let tunnel_opt = self.get_tunnel(packet.uuid.clone()).await;
            println!("[DEBUG] Got tunnel for UUID {}: {:?}", packet.uuid, tunnel_opt.is_some());

            if let Some(tunnel_arc) = tunnel_opt {
                println!("[DEBUG] Attempting connection to {}:{}", ip, port);
                
                let mut tunnel_guard = tunnel_arc.lock().await;
                
                match tunnel_guard.make_connection(&ip, port, 3).await {
                    Ok(()) => {
                        tunnel_guard.backlife_cycle(3);
                        drop(tunnel_guard);
                        println!("[DEBUG] Successfully established connection");
                        Ok(())
                    }
                    Err(e) => {
                        println!("[DEBUG] Connection failed: {}", e);
                        Err("[STUN] Fail connection".to_string())
                    }
                }
            } else {
                println!("[DEBUG] No tunnel found for UUID: {}", packet.uuid);
                Err("[STUN] error get tunnel".to_string())
            }
        } else {
            println!("[DEBUG] Invalid packet data format");
            Err("[STUN] Invalid accept_connection packet".to_string())
        }
    }
}
