use async_std::sync::Mutex;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, RwLock};

use super::peer_search::PeerSearchManager;
use super::Peer;
use crate::config::Config;
use crate::db::P2PDatabase;
use crate::http::http_proxy::HttpProxy;
use crate::packets::{
    PeerWaitConnection, Protocol, SearchPathNode, SyncPeerInfo, SyncPeerInfoData, TransportData,
    TransportPacket,
};
use crate::signal::client::SignalClient;
use crate::signal::signal_servers::{
    SignalServerInfo as StoredSignalServerInfo, SignalServersList,
};
use crate::tunnel::Tunnel;

const SHOW_LOGS: bool = true;

fn log(message: &str) {
    if SHOW_LOGS {
        println!("{}", message);
    }
}

#[derive(Debug)]
pub struct SignalServer {
    pub peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    pub connected_servers: Arc<RwLock<Vec<Arc<SignalClient>>>>,
    peer_search_manager: Arc<PeerSearchManager>,
    port: i64,
    ip: String,
    message_tx: mpsc::Sender<(Arc<Peer>, String)>,
    pub response_tx: mpsc::Sender<TransportPacket>,
    pub response_rx: Arc<Mutex<mpsc::Receiver<TransportPacket>>>,
    pub proxy_http_tx: mpsc::Sender<TransportPacket>,
    my_public_addr: Arc<String>,
    my_public_key: Arc<String>,
    pub db: Arc<P2PDatabase>,
    pub http_proxy: Arc<HttpProxy>,
}

impl SignalServer {
    pub async fn new(config: &Config, db: &P2PDatabase) -> Arc<Self> {
        let (message_tx, mut message_rx) = mpsc::channel(1000);
        let (response_tx, mut response_rx) = mpsc::channel(1000);
        let (proxy_http_tx, mut proxy_http_rx) = mpsc::channel(1000);

        let tunnel = Tunnel::new().await;
        let public_ip = tunnel.get_public_ip();
        let my_public_addr = Arc::new(format!("{}:{}", public_ip, config.signal_server_port));
        let my_public_key = db.get_or_create_peer_id().unwrap();

        let peers = Arc::new(RwLock::new(Vec::new()));
        let connected_servers = Arc::new(RwLock::new(Vec::new()));
        let peer_search_manager = PeerSearchManager::new(
            db.get_or_create_peer_id().unwrap(),
            public_ip.to_string().clone(),
            config.signal_server_port.clone(),
            peers.clone(),
            connected_servers.clone(),
        );

        let proxy_http_tx_clone = proxy_http_tx.clone();
        let proxy = Arc::new(HttpProxy::new(
            Arc::new(db.clone()),
            proxy_http_tx_clone
        ));

        let proxy_clone = Arc::clone(&proxy);
        let server = SignalServer {
            peers,
            connected_servers,
            peer_search_manager,
            port: config.signal_server_port,
            message_tx,
            response_tx,
            response_rx: Arc::new(Mutex::new(response_rx)),
            proxy_http_tx,
            ip: public_ip,
            my_public_addr,
            my_public_key: Arc::new(my_public_key),
            db: Arc::new(db.clone()),
            http_proxy: proxy,
        };

        tokio::spawn(async move {
            proxy_clone.start().await;
        });
        
        let server_arc = Arc::new(server);
        if let Ok(mut servers_list) = SignalServersList::load_or_create() {
            for server_info in servers_list.servers.iter() {
                if (server_info.public_ip == "127.0.0.1"
                    && server_info.port != config.signal_server_port)
                    || server_info.public_ip != "127.0.0.1"
                {
                    let server_addr = format!("{}:{}", server_info.public_ip, server_info.port);
                    let server_clone = Arc::clone(&server_arc);
                    let public_key = server_info.public_key.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server_clone
                            .connect_to_signal_server(&server_addr, &public_key)
                            .await
                        {
                            log(&format!(
                                "[SignalServer] Failed to connect to signal server {}: {}",
                                server_addr, e
                            ));
                        }
                    });
                }
            }
        }

        let server_clone = Arc::clone(&server_arc);
        let server_clone_for_proxy = Arc::clone(&server_arc);
        let server_clone_for_proxy_http = Arc::clone(&server_arc);

        tokio::spawn(async move {
            while let Some((peer, message)) = message_rx.recv().await {
                let server_clone = server_clone.clone();
                let peer_clone = peer.clone();
                tokio::spawn(async move {
                    server_clone
                        .handle_message(&server_clone, peer_clone, message)
                        .await;
                });
            }
        });

        tokio::spawn(async move {
            let mut rx = server_clone_for_proxy_http.response_rx.lock().await;
            while let Some(response) = rx.recv().await {
                let server_clone = server_clone_for_proxy_http.clone();
                tokio::spawn(async move {
                    if let Some(TransportData::ProxyMessage(msg)) = response.data {
                        log(&format!(
                            "Received response from proxy: {:?}",
                            msg.from_peer_id
                        ));
                        let encrypted_response = base64::decode(&msg.text).unwrap();
                        let nonce = base64::decode(&msg.nonce).unwrap();
                        let nonce_array: [u8; 12] = nonce.try_into().unwrap();
                        let response_bytes = server_clone
                            .db
                            .decrypt_message(&encrypted_response, nonce_array, &msg.from_peer_id)
                            .unwrap();

                        server_clone.http_proxy
                            .set_response(msg.request_id.clone(), response_bytes)
                            .await;
                    }
                });
            }
        });

        tokio::spawn(async move {
            while let Some(packet) = proxy_http_rx.recv().await {
                let server_clone = server_clone_for_proxy.clone();
                tokio::spawn(async move {
                    server_clone.auto_send_packet(packet).await;
                });
            }
        });

        server_arc
    }

    pub async fn run(self: Arc<Self>) {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(addr.clone()).await.unwrap();
        log(&format!("[SignalServer] Running on {}", addr));

        loop {
            let (socket, _) = listener.accept().await.unwrap();
            log(&format!(
                "[SignalServer] New connection: {}",
                socket.peer_addr().unwrap()
            ));

            let peer = Peer::new(socket, None);

            self.add_peer(peer.clone(), false).await.unwrap();

            let self_clone: Arc<SignalServer> = Arc::clone(&self);
            tokio::spawn(async move {
                self_clone.handle_connection(peer.clone()).await;
            });
        }
    }

    async fn connect_to_signal_server(
        self: Arc<Self>,
        server_addr: &str,
        public_key: &str,
    ) -> Result<(), String> {
        let parts: Vec<&str> = server_addr.split(':').collect();
        if parts.len() != 2 {
            return Err(format!("Invalid server address format: {}", server_addr));
        }

        let ip = parts[0];
        let port: i64 = parts[1].parse::<i64>().map_err(|e| e.to_string())?;

        let mut client = SignalClient::new(&self.db);
        client
            .connect(ip, port, &self.ip, self.port as u16, public_key)
            .await?;

        let message_rx = client.get_message_receiver();
        let client_arc = Arc::new(client);
        self.connected_servers
            .write()
            .await
            .push(client_arc.clone());

        let self_clone: Arc<SignalServer> = Arc::clone(&self);
        if let Some(mut message_rx) = message_rx {
            tokio::spawn(async move {
                while let Some(packet) = message_rx.recv().await {
                    log(&format!(
                        "[SignalServer] Received packet from signal server: {:?}",
                        packet
                    ));
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
                            log(&format!(
                                "[SignalServer] Received search request from signal server for peer {} (from {})",
                                request.search_id, request.peer_id
                            ));
                            if let Err(e) = self
                                .peer_search_manager
                                .handle_search_request(request.clone())
                                .await
                            {
                                log(&format!(
                                    "[SignalServer] Failed to handle search request: {}",
                                    e
                                ));
                            }
                        }
                        TransportData::PeerSearchResponse(response) => {
                            log(&format!(
                                "[SignalServer] Received search response from signal server for peer {} (from {})",
                                response.search_id, response.peer_id
                            ));
                            log(&format!(
                                "[SignalServer] Response details - found_peer: {}, public_ip: {}, public_port: {}, hops: {}",
                                response.found_peer_id, response.public_ip, response.public_port, response.hops
                            ));
                            if let Err(e) = self
                                .peer_search_manager
                                .handle_search_response(response.clone())
                                .await
                            {
                                log(&format!(
                                    "[SignalServer] Failed to handle search response: {}",
                                    e
                                ));
                            }
                        }
                        _ => {
                            log(&format!(
                                "[SignalServer] Received unknown packet: {:?}",
                                packet
                            ));
                            self.auto_send_packet(packet).await;
                        }
                    }
                }
            }
            _ => {
                log(&format!(
                    "[SignalServer] Received unknown packet: {:?}",
                    packet
                ));
                self.auto_send_packet(packet).await;
            }
        }
    }

    async fn handle_connection(self: Arc<Self>, peer: Arc<Peer>) {
        loop {
            let message = match peer.receive().await {
                Ok(msg) => msg,
                Err(e) => {
                    log(&format!(
                        "[SignalServer] Failed to receive message from peer {}: {}",
                        peer.info.local_addr, e
                    ));
                    if e == "Peer disconnected" {
                        self.remove_peer(&peer).await;
                        break;
                    }
                    continue;
                }
            };
            log(&format!("[SignalServer] Received message: {}", message));

            if let Err(e) = self.message_tx.send((peer.clone(), message)).await {
                log(&format!(
                    "[SignalServer] Failed to send message to handler: {}",
                    e
                ));
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
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
            nodes: vec![],
        };

        let packet = serde_json::to_string(&packet).unwrap();
        if let Err(e) = peer.send(packet).await {
            log(&format!(
                "[SignalServer] Failed to send peer list to peer {}: {}",
                peer_uuid_clone.clone().as_str(),
                e
            ));
        } else {
            log(&format!(
                "[SignalServer] Successfully sent peer list to peer {}",
                peer_uuid
            ));
        }
    }

    async fn handle_message(&self, server: &Arc<SignalServer>, peer: Arc<Peer>, message: String) {
        log(&format!(
            "[SignalServer] Handling message from peer {}: {}",
            peer.info.local_addr, message
        ));
        let message: TransportPacket = match serde_json::from_str(&message) {
            Ok(msg) => msg,
            Err(e) => {
                log(&format!(
                    "[SignalServer] Failed to parse message from peer {}: {}. Message: {}",
                    peer.info.local_addr, e, message
                ));
                return;
            }
        };

        if message.act == "http_proxy_response" {
            if let Some(target_peer_id) = &message.to {
                if *target_peer_id != *self.my_public_key {
                    log(&format!("Sending response to peer: {:?}", message.to));
                    self.auto_send_packet(message).await;
                } else {
                    log("Sending response to channel response");
                    self.response_tx.send(message).await;
                }
            }
            return;
        }

        let peer_uuid = &message.uuid;
        let is_peer_wait_connection = message.act == "wait_connection";
        let is_peer_accept_connection = message.act == "accept_connection";

        peer.set_wait_connection(is_peer_wait_connection || is_peer_accept_connection)
            .await;
        peer.set_uuid(peer_uuid.clone()).await;

        if let Some(data) = &message.data {
            match data {
                TransportData::SignalServerInfo(server_info) => {
                    log(&format!(
                        "[SignalServer] Received signal server info from peer {}: {:?}",
                        peer_uuid, server_info
                    ));

                    // Сохраняем информацию о сигнальном сервере
                    let stored_info = StoredSignalServerInfo {
                        public_key: server_info.public_key.clone(),
                        public_ip: server_info.public_ip.clone(),
                        port: server_info.port,
                    };

                    if let Ok(mut servers_list) = SignalServersList::load_or_create() {
                        if let Err(e) = servers_list.add_server(stored_info.clone()) {
                            log(&format!(
                                "[SignalServer] Failed to add signal server to list: {}",
                                e
                            ));
                        } else {
                            // Инициализируем соединение с новым сигнальным сервером
                            let server_addr =
                                format!("{}:{}", stored_info.public_ip, stored_info.port);
                            let server_clone = Arc::clone(server);
                            tokio::spawn(async move {
                                if let Err(e) = server_clone
                                    .connect_to_signal_server(&server_addr, &stored_info.public_key)
                                    .await
                                {
                                    log(&format!(
                                        "[SignalServer] Failed to connect to signal server {}: {}",
                                        server_addr, e
                                    ));
                                }
                            });
                        }
                    }
                }
                TransportData::PeerSearchRequest(request) => {
                    log(&format!(
                        "[SignalServer] Received search request for peer {} from {}",
                        request.search_id, request.peer_id
                    ));
                    if let Err(e) = self
                        .peer_search_manager
                        .handle_search_request(request.clone())
                        .await
                    {
                        log(&format!(
                            "[SignalServer] Failed to handle search request: {}",
                            e
                        ));
                    }
                }
                TransportData::PeerSearchResponse(response) => {
                    log(&format!(
                        "[SignalServer] Received search response for peer {} from {}",
                        response.search_id, response.peer_id
                    ));
                    if let Err(e) = self
                        .peer_search_manager
                        .handle_search_response(response.clone())
                        .await
                    {
                        log(&format!(
                            "[SignalServer] Failed to handle search response: {}",
                            e
                        ));
                    }
                }
                TransportData::PeerInfo(info) => {
                    log(&format!(
                        "[SignalServer] Setting peer UUID: {}",
                        info.peer_id
                    ));
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
                            log(&format!(
                                "[SignalServer] Forwarding storage reservation request to peer: {}",
                                p.get_key().await.unwrap_or_default()
                            ));
                            let packet = TransportPacket {
                                act: "reserve_storage".to_string(),
                                to: Some(p.get_key().await.unwrap_or_default()),
                                data: Some(TransportData::StorageReservationRequest(
                                    request.clone(),
                                )),
                                protocol: Protocol::SIGNAL,
                                uuid: self.db.get_or_create_peer_id().unwrap(),
                                nodes: vec![SearchPathNode {
                                    uuid: self.db.get_or_create_peer_id().unwrap(),
                                    public_ip: self.ip.clone(),
                                    public_port: self.port,
                                }],
                            };
                            if let Err(e) = p.send(serde_json::to_string(&packet).unwrap()).await {
                                log(&format!("[SignalServer] Failed to forward storage reservation request: {}", e));
                            }
                        }
                    }
                }
                TransportData::StorageReservationResponse(response) => {
                    let packet = TransportPacket {
                        act: "reserve_storage_response".to_string(),
                        to: Some(response.peer_id.clone()),
                        data: Some(TransportData::StorageReservationResponse(response.clone())),
                        protocol: Protocol::SIGNAL,
                        uuid: self.db.get_or_create_peer_id().unwrap(),
                        nodes: vec![],
                    };
                    self.auto_send_packet(packet).await;
                }
                TransportData::FragmentMetadataSync(data) => {
                    log(&format!(
                        "[SignalServer] Получены метаданные фрагментов от пира {}",
                        data.peer_id
                    ));
                    
                    for fragment in data.fragments.clone() {
                        let storage = crate::db::Storage {
                            file_hash: fragment.file_hash,
                            filename: String::new(), 
                            token: String::new(), 
                            token_hash: None,
                            uploaded_via_token: None,
                            owner_key: fragment.owner_key,
                            storage_peer_key: fragment.storage_peer_key,
                            mime: fragment.mime,
                            public: fragment.public,
                            encrypted: fragment.encrypted,
                            compressed: fragment.compressed,
                            auto_decompress: fragment.auto_decompress,
                            size: fragment.size,
                        };

                        if let Err(e) = self.db.add_storage_fragment(storage) {
                            log(&format!(
                                "[SignalServer] Ошибка при сохранении метаданных фрагмента: {}",
                                e
                            ));
                        }
                    }

                    log("[SignalServer] Метаданные фрагментов успешно сохранены");

                    let packet = TransportPacket {
                        act: "sync_fragments".to_string(),
                        to: None,
                        data: Some(TransportData::FragmentMetadataSync(data.clone())),
                        protocol: Protocol::SIGNAL,
                        uuid: self.db.get_or_create_peer_id().unwrap(),
                        nodes: vec![],
                    };
                    self.broadcast_to_servers(packet).await;
                }
                _ => {}
            }
        }

        if message.act == "info" {
            if let Some(TransportData::PeerInfo(info)) = &message.data {
                peer.set_is_signal_server(info.is_signal_server).await;
            }

            log("[SignalServer] =================");
            log("[SignalServer] CONNECTED PEER INFO:");
            log(&format!("[SignalServer] PUBLIC ADDRESS: {}", peer_uuid));
            log(&format!(
                "[SignalServer] LOCAL ADDRESS: {}",
                peer.info.local_addr
            ));
            log(&format!(
                "[SignalServer] IS SIGNAL SERVER: {}",
                peer.is_signal_server().await
            ));
            if let Some(uuid) = peer.info.uuid.read().await.clone() {
                log(&format!("[SignalServer] PEER UUID: {}", uuid));
            } else {
                log("[SignalServer] PEER UUID: Not set");
            }
            log("[SignalServer] =================");

            server.clone().sync_peers(peer.clone()).await;
        }

        match message.protocol {
            Protocol::STUN => {
                if is_peer_wait_connection {
                    log(&format!(
                        "[SignalServer] Peer is ready to connect: {}",
                        peer_uuid
                    ));
                    if let Some(TransportData::PeerWaitConnection(data)) = message.data {
                        log(&format!(
                            "[SignalServer] Looking for peer with UUID: {}",
                            data.connect_peer_id
                        ));
                        let peers_guard = server.peers.read().await;
                        for target_peer in peers_guard.iter() {
                            if let Some(uuid) = target_peer.info.uuid.read().await.clone() {
                                let open_tunnel =
                                    target_peer.get_open_tunnel(&data.connect_peer_id).await;
                                if uuid == data.connect_peer_id {
                                    log(&format!(
                                        "[SignalServer] Found peer with matching UUID: {}",
                                        data.connect_peer_id
                                    ));
                                    log("peer finded");
                                    if open_tunnel.is_some() {
                                        log("[SignalServer] Peer have open tunnel. Start connect peers");
                                        let server_clone = Arc::clone(server);
                                        server_clone
                                            .connect_peers(peer.clone(), target_peer.clone())
                                            .await;
                                    } else {
                                        log("[SignalServer] Peer without open tunnel, send wait connection");
                                        let packet = TransportPacket {
                                            act: message.act.to_string(),
                                            to: message.to.clone(),
                                            data: Some(TransportData::PeerWaitConnection(
                                                data.clone(),
                                            )),
                                            protocol: Protocol::STUN,
                                            uuid: message.uuid.to_string(),
                                            nodes: vec![],
                                        };
                                        let packet_json = serde_json::to_string(&packet).unwrap();
                                        target_peer.send(packet_json).await;
                                        log("sended packet json");
                                    }
                                    return;
                                }
                            }
                        }
                        log(&format!(
                            "[SignalServer] Peer with UUID {} not found",
                            data.connect_peer_id
                        ));
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
                                        log("[SignalServer] Both peers have open tunnels. Connecting");
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
                    log(&format!(
                        "[SignalServer] Received turn packet: {:?}",
                        message
                    ));
                    let peers_guard = server.peers.read().await;
                    for item in peers_guard.iter() {
                        if *item.info.uuid.read().await == Some(to.clone()) {
                            log(&format!(
                                "[SignalServer] Send turn packet: {} {:?}",
                                peer.info.local_addr, message
                            ));

                            let turn_packet = TransportPacket {
                                act: message.act.to_string(),
                                to: message.to.clone(),
                                data: message.data.clone(),
                                protocol: Protocol::TURN,
                                uuid: message.uuid.to_string(),
                                nodes: vec![],
                            };
                            let turn_packet = serde_json::to_string(&turn_packet).unwrap();
                            if let Err(e) = item.send(turn_packet).await {
                                log(&format!(
                                    "[SignalServer] Failed to send turn packet to peer {}: {}",
                                    item.info.local_addr, e
                                ));
                            } else {
                                log(&format!(
                                    "[SignalServer] Successfully send turn packet to peer {}",
                                    item.info.local_addr
                                ));
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
                        log("[SignalServer] Processing peers for wait_connection:");
                        for peer in &data.peers {
                            // println!("  - Public Address: {}", peer.public_addr);
                            log(&format!("    UUID: {}", peer.uuid));
                        }
                    } else {
                        log("[SignalServer] No peers found in the data for wait_connection.");
                    }
                } else if message.to.is_some() {
                    log(&format!(
                        "[SignalServer] Sending packet to peer: {}",
                        message.to.clone().unwrap()
                    ));
                    self.send_to_peer_by_packet(message.clone()).await;
                }
            }
        }
    }

    pub async fn auto_send_packet(&self, message: TransportPacket) {
        let target_peer_id = message.to.clone().unwrap();
        let from_peer_id = message.uuid.clone();
        let mut sended = false;

        if from_peer_id == *target_peer_id {
            return;
        }

        for peer in self.peers.read().await.iter() {
            if peer.info.uuid.read().await.clone().unwrap() == target_peer_id
                || peer.is_signal_server().await
            {
                peer.send(serde_json::to_string(&message).unwrap()).await;
                sended = true;
            }
        }
        if !sended {
            for server in self.connected_servers.read().await.iter() {
                if server.public_key != from_peer_id {
                    log(&format!(
                        "[SignalServer] Sending packet to signal server: {:?}. from uuid: {}",
                        server.public_key, from_peer_id
                    ));
                    server.send_packet(message.clone()).await;
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
        log("[SignalServer] Connecting peers");

        {
            log(&format!(
                "[SignalServer] Sending packet to: {}",
                second_peer.info.local_addr
            ));
            SignalServer::send_peer_info(second_peer.clone(), first_peer.clone()).await;
            log(&format!(
                "[SignalServer] Sent packet to peer: {}",
                second_peer.info.local_addr
            ));
        }
        second_peer.set_wait_connection(false).await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        {
            log(&format!(
                "[SignalServer] Sending packet to: {}",
                first_peer.info.local_addr
            ));
            SignalServer::send_peer_info(first_peer.clone(), second_peer.clone()).await;
            log(&format!(
                "[SignalServer] Sent packet to peer: {}",
                first_peer.info.local_addr
            ));
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
                log(&format!(
                    "Peer already in the list: {}",
                    peer.info.local_addr
                ));
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
                    protocol: Protocol::STUN,
                    uuid: about_peer.info.uuid.read().await.clone().unwrap(),
                    nodes: vec![],
                };
                let wait_packet = serde_json::to_string(&wait_packet).unwrap();

                log(&format!(
                    "[SignalServer] Sending wait packet to peer: {}",
                    to_peer.info.local_addr
                ));
                let result = to_peer.send(wait_packet).await;

                match result {
                    Ok(_) => log(&format!(
                        "[SignalServer] Successfully sent packet to peer: {}. Peer connecting to: {}",
                        key_peer,
                        pub_id,
                    )),
                    Err(e) => log(&format!("[SignalServer] Failed to send peer to peer info: {}", e)),
                }
            }
        }
    }

    async fn broadcast_to_servers(&self, packet: TransportPacket) {
        let servers = self.connected_servers.read().await;
        for server in servers.iter() {
            if let Err(e) = server.send_packet(packet.clone()).await {
                log(&format!(
                    "[SignalServer] Failed to broadcast to server: {}",
                    e
                ));
            }
        }
    }
}
