use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::packets::{PeerInfo, PeerSearchRequest, PeerSearchResponse, Protocol, TransportData, TransportPacket};
use crate::signal::client::SignalClient;

use super::{peer, Peer};

#[derive(Debug)]
struct SearchCache {
    found_peer_id: String,
    public_addr: String,
    expires_at: Instant,
}

#[derive(Debug)]
pub struct PeerSearchManager {
    peers: Arc<RwLock<Vec<Arc<Peer>>>>,
    public_key: String,
    my_public_addr: String,
    connected_servers: Arc<RwLock<Vec<Arc<SignalClient>>>>,
    active_searches: Arc<RwLock<HashMap<String, mpsc::Sender<PeerSearchResponse>>>>,
    search_cache: Arc<RwLock<HashMap<String, SearchCache>>>,
}

impl PeerSearchManager {
    pub fn new(public_key: String, my_public_addr: String, peers: Arc<RwLock<Vec<Arc<Peer>>>>, connected_servers: Arc<RwLock<Vec<Arc<SignalClient>>>>) -> Arc<Self> {
        Arc::new(Self {
            peers,
            public_key,
            my_public_addr,
            connected_servers,
            active_searches: Arc::new(RwLock::new(HashMap::new())),
            search_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub async fn handle_search_request(&self, request: PeerSearchRequest) -> Result<(), String> {
        let peer_id = request.peer_id.clone(); // id пира инициатора поиска
        let search_id = request.search_id.clone(); // id поиска
        
        println!("[PeerSearch] Starting search for peer {} (request from {})", search_id, peer_id);
        
        // Проверяем кэш
        if let Some(cache) = self.search_cache.read().await.get(&search_id) {
            if cache.expires_at > Instant::now() {
                println!("[PeerSearch] Found peer {} in cache", search_id);
                let response = PeerSearchResponse {
                    search_id: search_id.clone(), // id поиска
                    peer_id: peer_id.clone(), // id пира инициатора поиска
                    found_peer_id: cache.found_peer_id.clone(), // id пира найденного
                    public_addr: self.my_public_addr.clone(), // публичный адрес ноды пира
                    hops: 0, // количество прыжков
                };

                // Находим пира-инициатора поиска среди локальных пиров
                let peers = self.peers.read().await;
                for peer in peers.iter() {
                    let peer_uuid = peer.info.uuid.read().await;
                    if peer_uuid.as_ref() == Some(&peer_id) {
                        println!("[PeerSearch] Found initiator peer {} locally, sending response directly", peer_id);
                        let packet = TransportPacket {
                            act: "search_response".to_string(), // ответ на поиск
                            to: Some(peer_id.clone()), // кому отправляем
                            data: Some(TransportData::PeerSearchResponse(response)), // данные
                            status: None, // статус
                            protocol: Protocol::SIGNAL, // протокол
                            uuid: self.public_key.clone(), // публичный адрес ноды пира
                        };
                        peer.send_data(&serde_json::to_string(&packet).unwrap()).await;
                        println!("[PeerSearch] Sent cached search response to peer");
                        return Ok(());
                    }
                }
                println!("[PeerSearch] Initiator peer {} not found locally, forwarding response", peer_id);
            }
        }

        let peers = self.peers.read().await;
        println!("[PeerSearch] Checking {} local peers", peers.len());
        for peer in peers.iter() {
            let peer_uuid = peer.info.uuid.read().await;
            if peer_uuid.as_ref() == Some(&search_id) {
                println!("[PeerSearch] Found peer {} locally", search_id);
                let response = PeerSearchResponse {
                    search_id: search_id.clone(), // id поиска
                    peer_id: peer_id.clone(), // id пира инициатора поиска
                    found_peer_id: search_id.clone(), // id пира найденного
                    public_addr: self.my_public_addr.clone(), // публичный адрес ноды пира
                    hops: 0, // количество прыжков
                };

                // Кэшируем результат
                let search_id_clone = search_id.clone();
                let found_peer_id = search_id_clone.clone();
                self.search_cache.write().await.insert(
                    search_id_clone.clone(),
                    SearchCache {
                        found_peer_id,
                        public_addr: self.my_public_addr.clone(),
                        expires_at: Instant::now() + Duration::from_secs(300), // 5 минут
                    },
                );

                // Находим пира-инициатора поиска среди локальных пиров
                for initiator_peer in peers.iter() {
                    let initiator_uuid = initiator_peer.info.uuid.read().await;
                    if initiator_uuid.as_ref() == Some(&peer_id) {
                        println!("[PeerSearch] Found initiator peer {} locally, sending response directly", peer_id);
                        let packet = TransportPacket {
                            act: "search_response".to_string(),
                            to: Some(peer_id.clone()),
                            data: Some(TransportData::PeerSearchResponse(response)),
                            status: None,
                            protocol: Protocol::SIGNAL,
                            uuid: self.public_key.clone(),
                        };
                        initiator_peer.send_data(&serde_json::to_string(&packet).unwrap()).await;
                        println!("[PeerSearch] Sent search response to peer");
                        return Ok(());
                    }
                }
                println!("[PeerSearch] Initiator peer {} not found locally, forwarding response", peer_id);
            }
        }

        // Если не нашли локально и есть еще ходы
        if request.max_hops > 0 {
            println!("[PeerSearch] Peer not found locally, forwarding search request (hops left: {})", request.max_hops - 1);
            let new_request = PeerSearchRequest {
                search_id: search_id.clone(),
                peer_id: request.peer_id,
                max_hops: request.max_hops - 1,
            };

            let packet = TransportPacket {
                act: "search_peer".to_string(),
                to: None,
                data: Some(TransportData::PeerSearchRequest(new_request)),
                status: None,
                protocol: Protocol::SIGNAL,
                uuid: peer_id,
            };

            let servers = self.connected_servers.read().await;
            for server in servers.iter() {
                if let Err(e) = server.send_packet(packet.clone()).await {
                    println!("[PeerSearch] Failed to forward search request to server: {}", e);
                } else {
                    println!("[PeerSearch] Successfully forwarded search request to server");
                }
            }
        } else {
            println!("[PeerSearch] Search failed - max hops reached");
        }

        Ok(())
    }

    pub async fn handle_search_response(&self, response: PeerSearchResponse) -> Result<(), String> {
        println!("[PeerSearch] Received search response for peer {} (from {})", response.search_id, response.peer_id);
        
        // Проверяем, есть ли активный поиск с таким ID
        if let Some(tx) = self.active_searches.read().await.get(&response.search_id) {
            println!("[PeerSearch] Found active search, sending response to channel");
            // Отправляем ответ в канал
            if let Err(e) = tx.send(response).await {
                println!("[PeerSearch] Failed to send search response to channel: {}", e);
            } else {
                println!("[PeerSearch] Successfully sent response to channel");
            }
        } else {
            println!("[PeerSearch] No active search found, forwarding response to {}", response.peer_id);
            // Если это не наш поиск, пересылаем ответ дальше
            let packet = TransportPacket {
                act: "search_response".to_string(),
                to: Some(response.peer_id.clone()), // Отправляем инициатору поиска
                data: Some(TransportData::PeerSearchResponse(response)),
                status: None,
                protocol: Protocol::SIGNAL,
                uuid: self.public_key.clone(),
            };

            // Отправляем ответ всем серверам
            let servers = self.connected_servers.read().await;
            let mut finded_server = false;
            for server in servers.iter() {
                if let Err(e) = server.send_packet(packet.clone()).await {
                    println!("[PeerSearch] Failed to forward search response to server: {}", e);
                } else {
                    finded_server = true;
                }
            }
        }
        Ok(())
    }
} 