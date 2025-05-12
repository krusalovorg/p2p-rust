use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};

use super::peer_search::PeerSearchManager;
use super::Peer;
use crate::config::Config;
use crate::db::P2PDatabase;
use crate::packets::{
    PeerWaitConnection, Protocol, SyncPeerInfo, SyncPeerInfoData, TransportData, TransportPacket,
};
use crate::signal::client::SignalClient;
use crate::tunnel::Tunnel;

#[derive(Debug)]
pub struct SignalServer {
    pub peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    pub connected_servers: Arc<RwLock<Vec<Arc<SignalClient>>>>,
    peer_search_manager: Arc<PeerSearchManager>,
    port: i64,
    ip: String,
    message_tx: mpsc::Sender<(Arc<Peer>, String)>,
    my_public_addr: Arc<String>,
    db: Arc<P2PDatabase>,
}

impl SignalServer {
    pub async fn new(db: &P2PDatabase) -> Arc<Self> {
        let config: Config = Config::from_file("config.toml");
        let (message_tx, mut message_rx) = mpsc::channel(100);

        let tunnel = Tunnel::new().await;
        let public_ip = tunnel.get_public_ip();
        let my_public_addr = Arc::new(format!("{}:{}", public_ip, config.signal_server_port));

        let peers = Arc::new(RwLock::new(Vec::new()));
        let connected_servers = Arc::new(RwLock::new(Vec::new()));
        let peer_search_manager = PeerSearchManager::new(
            db.get_or_create_peer_id().unwrap(),
            my_public_addr.to_string().clone(),
            peers.clone(),
            connected_servers.clone(),
        );

        let server = SignalServer {
            peers,
            connected_servers,
            peer_search_manager,
            port: config.signal_server_port,
            message_tx,
            ip: public_ip,
            my_public_addr,
            db: Arc::new(db.clone()),
        };

        let server_arc = Arc::new(server);

        let server_clone = Arc::clone(&server_arc);
        tokio::spawn(async move {
            while let Some((peer, message)) = message_rx.recv().await {
                server_clone
                    .handle_message(&server_clone, peer, message)
                    .await;
            }
        });

        server_arc
    }

    pub async fn run(self: Arc<Self>) {
        let config: Config = Config::from_file("config.toml");

        // Запускаем подключение к другим сигнальным серверам
        for server_addr in &config.other_signal_servers {
            let server_addr = server_addr.clone();
            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = self_clone.connect_to_signal_server(&server_addr).await {
                    println!(
                        "[SignalServer] Failed to connect to signal server {}: {}",
                        server_addr, e
                    );
                }
            });
        }

        // Запускаем основной сервер
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(addr.clone()).await.unwrap();
        println!("[SignalServer] Running on {}", addr);

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            println!(
                "[SignalServer] New connection: {}",
                socket.peer_addr().unwrap()
            );

            let peer = Peer::new(socket, None);

            self.add_peer(peer.clone(), false).await.unwrap();

            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                self_clone.handle_connection(peer.clone()).await;
            });
        }
    }

    async fn connect_to_signal_server(&self, server_addr: &str) -> Result<(), String> {
        let parts: Vec<&str> = server_addr.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid server address format: {}", server_addr));
        }

        let ip = parts[0];
        let port: i64 = parts[1].parse::<i64>().map_err(|e| e.to_string())?;

        let mut client = SignalClient::new(&self.db);
        client.connect(ip, port, &self.ip, self.port as u16).await?;

        let client_arc = Arc::new(client);
        self.connected_servers
            .write()
            .await
            .push(client_arc.clone());

        // Запускаем обработчик сообщений от сигнального сервера
        let self_clone = Arc::clone(&self);
        if let Some(mut message_rx) = client_arc.get_message_receiver() {
            tokio::spawn(async move {
                while let Some(packet) = message_rx.recv().await {
                    println!("[SignalServer] Received packet from signal server: {:?}", packet);
                    self_clone.handle_signal_server_packet(packet).await;
                }
            });
        }

        Ok(())
    }

    async fn handle_signal_server_packet(&self, packet: TransportPacket) {
        match packet.protocol {
            Protocol::SIGNAL => {
                if let Some(data) = &packet.data {
                    match data {
                        TransportData::PeerSearchRequest(request) => {
                            println!(
                                "[SignalServer] Received search request from signal server for peer {}",
                                request.search_id
                            );
                            if let Err(e) = self
                                .peer_search_manager
                                .handle_search_request(request.clone())
                                .await
                            {
                                println!("[SignalServer] Failed to handle search request: {}", e);
                            }
                        }
                        TransportData::PeerSearchResponse(response) => {
                            println!(
                                "[SignalServer] Received search response from signal server for peer {}",
                                response.search_id
                            );
                            if let Err(e) = self
                                .peer_search_manager
                                .handle_search_response(response.clone())
                                .await
                            {
                                println!("[SignalServer] Failed to handle search response: {}", e);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    async fn handle_connection(self: Arc<Self>, peer: Arc<Peer>) {
        loop {
            let message = match peer.receive().await {
                Ok(msg) => msg,
                Err(e) => {
                    println!(
                        "[SignalServer] Failed to receive message from peer {}: {}",
                        peer.info.local_addr, e
                    );
                    if e == "Peer disconnected" {
                        self.remove_peer(&peer).await;
                        break;
                    }
                    continue;
                }
            };
            println!("[SignalServer] Received message: {}", message);

            if let Err(e) = self.message_tx.send((peer.clone(), message)).await {
                println!("[SignalServer] Failed to send message to handler: {}", e);
            }
        }
    }

    async fn remove_peer(self: Arc<Self>, peer: &Arc<Peer>) -> bool {
        let mut peers = self.peers.write().await;
        let peer_index = peers
            .iter()
            .position(|p| p.info.local_addr == peer.info.local_addr);

        if let Some(index) = peer_index {
            peers.remove(index);
            true
        } else {
            false
        }
    }

    async fn sync_peers(self: Arc<Self>, peer: Arc<Peer>) {
        let peers_info: Vec<SyncPeerInfo> = {
            let peers_guard = self.peers.read().await;
            let mut peers_info = Vec::new();
            for p in peers_guard.iter() {
                let uuid = p
                    .info
                    .uuid
                    .read()
                    .await
                    .clone()
                    .unwrap_or_else(|| "Not set".to_string());
                peers_info.push(SyncPeerInfo { uuid: uuid });
            }
            peers_info
        };

        let peer_uuid = peer
            .info
            .uuid
            .read()
            .await
            .clone()
            .unwrap_or_else(|| "Not set".to_string());
        let peer_uuid_clone = peer_uuid.clone();

        let packet = TransportPacket {
            act: "peer_list".to_string(),
            to: Some(peer_uuid.clone().to_string()),
            data: Some(TransportData::SyncPeerInfoData(SyncPeerInfoData {
                peers: peers_info,
            })),
            status: None,
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        let packet = serde_json::to_string(&packet).unwrap();
        if let Err(e) = peer.send(packet).await {
            println!(
                "[SignalServer] Failed to send peer list to peer {}: {}",
                peer_uuid_clone.clone().as_str(),
                e
            );
        } else {
            println!(
                "[SignalServer] Successfully sent peer list to peer {}",
                peer_uuid
            );
        }
    }

    async fn handle_message(&self, server: &Arc<SignalServer>, peer: Arc<Peer>, message: String) {
        println!(
            "[SignalServer] Handling message from peer {}: {}",
            peer.info.local_addr, message
        );
        let message: TransportPacket = match serde_json::from_str(&message) {
            Ok(msg) => msg,
            Err(e) => {
                println!(
                    "[SignalServer] Failed to parse message from peer {}: {}. Message: {}",
                    peer.info.local_addr, e, message
                );
                return;
            }
        };

        let peer_uuid = &message.uuid;
        let is_peer_wait_connection = message.act == "wait_connection";
        let is_peer_accept_connection = message.act == "accept_connection";

        peer.set_wait_connection(is_peer_wait_connection || is_peer_accept_connection)
            .await;
        peer.set_uuid(peer_uuid.clone()).await;

        if let Some(data) = &message.data {
            match data {
                TransportData::PeerSearchRequest(request) => {
                    println!(
                        "[SignalServer] Received search request for peer {} from {}",
                        request.search_id, request.peer_id
                    );
                    if let Err(e) = self
                        .peer_search_manager
                        .handle_search_request(request.clone())
                        .await
                    {
                        println!("[SignalServer] Failed to handle search request: {}", e);
                    }    
                }
                TransportData::PeerSearchResponse(response) => {
                    println!(
                        "[SignalServer] Received search response for peer {} from {}",
                        response.search_id, response.peer_id
                    );
                    if let Err(e) = self
                        .peer_search_manager
                        .handle_search_response(response.clone())
                        .await
                    {
                        println!("[SignalServer] Failed to handle search response: {}", e);
                    }
                }
                TransportData::PeerInfo(info) => {
                    println!("[SignalServer] Setting peer UUID: {}", info.peer_id);
                    peer.set_uuid(info.peer_id.clone()).await;
                }
                TransportData::PeerWaitConnection(data) => {
                    peer.add_open_tunnel(
                        &data.connect_peer_id,
                        data.public_ip.clone(),
                        data.public_port,
                    )
                    .await;
                }
                TransportData::StorageReservationRequest(request) => {
                    // Отправляем запрос всем пирам
                    let peers = self.peers.read().await;
                    for p in peers.iter() {
                        if p.get_key().await.as_ref() != Some(&request.peer_id) {
                            let packet = TransportPacket {
                                act: "reserve_storage".to_string(),
                                to: Some(p.get_key().await.unwrap_or_default()),
                                data: Some(TransportData::StorageReservationRequest(
                                    request.clone(),
                                )),
                                status: None,
                                protocol: Protocol::SIGNAL,
                                uuid: self.db.get_or_create_peer_id().unwrap(),
                            };
                            if let Err(e) = p.send(serde_json::to_string(&packet).unwrap()).await {
                                println!("[SignalServer] Failed to forward storage reservation request: {}", e);
                            }
                        }
                    }
                }
                TransportData::StorageReservationResponse(response) => {
                    let packet = TransportPacket {
                        act: "reserve_storage_response".to_string(),
                        to: Some(response.peer_id.clone()),
                        data: Some(TransportData::StorageReservationResponse(response.clone())),
                        status: None,
                        protocol: Protocol::SIGNAL,
                        uuid: self.db.get_or_create_peer_id().unwrap(),
                    };
                    self.send_to_peer_by_packet(packet).await;
                }
                _ => {}
            }
        }

        if message.act == "info" {
            println!("[SignalServer] =================");
            println!("[SignalServer] CONNECTED PEER INFO:");
            println!("[SignalServer] PUBLIC ADDRESS: {}", peer_uuid);
            println!("[SignalServer] LOCAL ADDRESS: {}", peer.info.local_addr);
            if let Some(uuid) = peer.info.uuid.read().await.clone() {
                println!("[SignalServer] PEER UUID: {}", uuid);
            } else {
                println!("[SignalServer] PEER UUID: Not set");
            }
            println!("[SignalServer] =================");

            server.clone().sync_peers(peer.clone()).await;
        }

        match message.protocol {
            Protocol::STUN => {
                if is_peer_wait_connection {
                    println!("[SignalServer] Peer is ready to connect: {}", peer_uuid);
                    if let Some(TransportData::PeerWaitConnection(data)) = message.data {
                        println!(
                            "[SignalServer] Looking for peer with UUID: {}",
                            data.connect_peer_id
                        );
                        let peers_guard = server.peers.read().await;
                        for target_peer in peers_guard.iter() {
                            if let Some(uuid) = target_peer.info.uuid.read().await.clone() {
                                let open_tunnel =
                                    target_peer.get_open_tunnel(&data.connect_peer_id).await;
                                if uuid == data.connect_peer_id {
                                    println!(
                                        "[SignalServer] Found peer with matching UUID: {}",
                                        data.connect_peer_id
                                    );
                                    println!("peer finded");
                                    if open_tunnel.is_some() {
                                        println!(
                                            "[SignalServer] Peer have open tunnel. Start connect peers"
                                        );
                                        let server_clone = Arc::clone(server);
                                        server_clone
                                            .connect_peers(peer.clone(), target_peer.clone())
                                            .await;
                                    } else {
                                        println!(
                                            "[SignalServer] Peer without open tunnel, send wait connection"
                                        );
                                        let packet = TransportPacket {
                                            act: message.act.to_string(),
                                            to: message.to.clone(),
                                            data: Some(TransportData::PeerWaitConnection(
                                                data.clone(),
                                            )),
                                            status: message.status.clone(),
                                            protocol: Protocol::STUN,
                                            uuid: message.uuid.to_string(),
                                        };
                                        let packet_json = serde_json::to_string(&packet).unwrap();
                                        target_peer.send(packet_json).await;
                                        println!("sended packet json")
                                    }
                                    return;
                                }
                            }
                        }
                        println!(
                            "[SignalServer] Peer with UUID {} not found",
                            data.connect_peer_id
                        );
                    }
                } else if is_peer_accept_connection {
                    if let Some(TransportData::PeerWaitConnection(data)) = message.data.clone() {
                        let peers_guard = server.peers.read().await;
                        for target_peer in peers_guard.iter() {
                            if let Some(uuid) = target_peer.info.uuid.read().await.clone() {
                                if uuid == data.connect_peer_id {
                                    let open_tunnel_a = peer.get_open_tunnel(&uuid).await;
                                    let open_tunnel_b =
                                        target_peer.get_open_tunnel(&peer_uuid).await;
                                    if open_tunnel_a.is_some() && open_tunnel_b.is_some() {
                                        println!("[SignalServer] Both peers have open tunnels. Connecting");
                                        server
                                            .connect_peers(peer.clone(), target_peer.clone())
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            Protocol::TURN => {
                if let Some(to) = &message.to {
                    println!("[SignalServer] Received turn packet: {:?}", message);
                    let peers_guard = server.peers.read().await;
                    for item in peers_guard.iter() {
                        if *item.info.uuid.read().await == Some(to.clone()) {
                            println!(
                                "[SignalServer] Send turn packet: {} {:?}",
                                peer.info.local_addr, message
                            );

                            let turn_packet = TransportPacket {
                                act: message.act.to_string(),
                                to: message.to.clone(),
                                data: message.data.clone(),
                                status: message.status.clone(),
                                protocol: Protocol::TURN,
                                uuid: message.uuid.to_string(),
                            };
                            let turn_packet = serde_json::to_string(&turn_packet).unwrap();
                            if let Err(e) = item.send(turn_packet).await {
                                println!(
                                    "[SignalServer] Failed to send turn packet to peer {}: {}",
                                    item.info.local_addr, e
                                );
                            } else {
                                println!(
                                    "[SignalServer] Successfully send turn packet to peer {}",
                                    item.info.local_addr
                                );
                            }
                            break;
                        }
                    }
                }
            }
            Protocol::SIGNAL => {
                if message.act == "peer_list" {
                    server.clone().sync_peers(peer.clone()).await;
                } else if message.protocol == Protocol::STUN && message.act == "wait_connection" {
                    if let Some(TransportData::SyncPeerInfoData(data)) = &message.data {
                        println!("[SignalServer] Processing peers for wait_connection:");
                        for peer in &data.peers {
                            // println!("  - Public Address: {}", peer.public_addr);
                            println!("    UUID: {}", peer.uuid);
                        }
                    } else {
                        println!("[SignalServer] No peers found in the data for wait_connection.");
                    }
                } else if message.to.is_some() {
                    println!(
                        "[SignalServer] Sending packet to peer: {}",
                        message.to.clone().unwrap()
                    );
                    self.send_to_peer_by_packet(message.clone()).await;
                }
            }
        }
    }

    async fn send_to_peer_by_packet(&self, message: TransportPacket) {
        for peer in self.peers.read().await.iter() {
            if peer.info.uuid.read().await.clone().unwrap() == message.to.clone().unwrap() {
                peer.send(serde_json::to_string(&message).unwrap()).await;
            }
        }
    }

    async fn connect_peers(&self, first_peer: Arc<Peer>, second_peer: Arc<Peer>) {
        println!("[SignalServer] Connecting peers");

        {
            println!(
                "[SignalServer] Sending packet to: {}",
                second_peer.info.local_addr
            );
            SignalServer::send_peer_info(second_peer.clone(), first_peer.clone()).await;
            println!(
                "[SignalServer] Sent packet to peer: {}",
                second_peer.info.local_addr
            );
        }
        second_peer.set_wait_connection(false).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            println!(
                "[SignalServer] Sending packet to: {}",
                first_peer.info.local_addr
            );
            SignalServer::send_peer_info(first_peer.clone(), second_peer.clone()).await;
            println!(
                "[SignalServer] Sent packet to peer: {}",
                first_peer.info.local_addr
            );
        }
        first_peer.set_wait_connection(false).await;
    }

    async fn add_peer(
        &self,
        peer: Arc<Peer>,
        is_peer_wait_connection: bool,
    ) -> Result<Arc<Peer>, String> {
        let mut peers_guard = self.peers.write().await;
        let mut peer_added = false;
        let mut peer_res: Option<Arc<Peer>> = None;
        for item in peers_guard.iter() {
            if *item.info.local_addr == peer.info.local_addr {
                println!("Peer already in the list: {}", peer.info.local_addr);
                peer_added = true;
                if is_peer_wait_connection {
                    let mut wait_connection = item.info.wait_connection.write().await;
                    *wait_connection = true;
                }
                peer_res = Some(item.clone());
                break;
            }
        }
        if !peer_added {
            peer_res = Some(peer.clone());
            peers_guard.push(peer.clone());
        }
        if peer_res.is_none() {
            return Err("Failed to add peer to the list".to_string());
        }
        return Ok(peer_res.clone().unwrap());
    }

    async fn send_peer_info(to_peer: Arc<Peer>, about_peer: Arc<Peer>) {
        let pub_id = about_peer.info.uuid.read().await.clone().unwrap();
        if let Some(key_peer) = to_peer.get_key().await {
            let data_open_tunnel = about_peer.get_open_tunnel(&key_peer.to_string()).await;
            if let Some(open_tunnel) = data_open_tunnel {
                let wait_packet = TransportPacket {
                    act: "accept_connection".to_string(), // TODO: было wait_connection
                    to: Some(pub_id.clone()),             // UUID кому отправляем данный пакет
                    data: Some(TransportData::PeerWaitConnection(PeerWaitConnection {
                        connect_peer_id: pub_id.clone(),
                        public_ip: open_tunnel.ip,
                        public_port: open_tunnel.port,
                    })),
                    status: None,
                    protocol: Protocol::STUN,
                    uuid: about_peer.info.uuid.read().await.clone().unwrap(),
                };
                let wait_packet = serde_json::to_string(&wait_packet).unwrap();

                println!(
                    "[SignalServer] Sending wait packet to peer: {}",
                    to_peer.info.local_addr
                );
                let result = to_peer.send(wait_packet).await;

                match result {
                    Ok(_) => println!(
                        "[SignalServer] Successfully sent packet to peer: {}. Peer connecting to: {}",
                        key_peer,
                        pub_id,
                    ),
                    Err(e) => println!("[SignalServer] Failed to send peer to peer info: {}", e),
                }
            }
        }
    }

    async fn broadcast_to_servers(&self, packet: TransportPacket) {
        let servers = self.connected_servers.read().await;
        for server in servers.iter() {
            if let Err(e) = server.send_packet(packet.clone()).await {
                println!("[SignalServer] Failed to broadcast to server: {}", e);
            }
        }
    }
}
