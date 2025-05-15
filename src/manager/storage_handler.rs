use super::ConnectionManager::ConnectionManager;
use colored::*;
use crate::connection::Connection;
use crate::packets::{
    StorageReservationRequest, StorageReservationResponse, StorageToken, StorageValidTokenResponse,
    TransportData, TransportPacket, Protocol,
};
use base64;
use hex;
use k256;
use k256::ecdsa::signature::SignerMut;
use serde_json;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

impl ConnectionManager {
    pub async fn handle_storage_reservation_request(
        &self,
        request: StorageReservationRequest,
        connection: &Connection,
    ) -> Result<(), String> {
        let free_space = self.db.get_storage_free_space().await.unwrap();
        println!("[Peer] Free space: {}", free_space);

        if free_space < request.size_in_bytes {
            println!("[Peer] Not enough storage space available");
            return Err("Not enough storage space".to_string());
        }

        let mut signing_key = self.db.get_private_key().unwrap();
        let verifying_key = signing_key.verifying_key();
        let pub_key = verifying_key.to_encoded_point(true);
        let pub_key_hex = hex::encode(pub_key.as_bytes());

        let storage_token = StorageToken {
            file_size: request.size_in_bytes as u64,
            storage_provider: pub_key_hex,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            signature: Vec::new(),
        };

        let mut token_bytes = serde_json::to_vec(&storage_token).unwrap();
        let signature: k256::ecdsa::Signature =
            SignerMut::<k256::ecdsa::Signature>::sign(&mut signing_key, &token_bytes);

        let mut final_token = storage_token;
        final_token.signature = signature.to_bytes().to_vec();

        let token_base64 = base64::encode(serde_json::to_vec(&final_token).unwrap());

        let response = TransportPacket {
            act: "reserve_storage_response".to_string(),
            to: Some(request.peer_id.clone()),
            data: Some(TransportData::StorageReservationResponse(
                StorageReservationResponse {
                    peer_id: self.db.get_or_create_peer_id().unwrap(),
                    token: token_base64,
                    size_in_bytes: request.size_in_bytes,
                },
            )),
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        connection.send_packet(response).await.map_err(|e| e.to_string())
    }

    pub async fn handle_storage_valid_token_request(
        &self,
        token: String,
        connection: &Connection,
        from_uuid: String,
    ) -> Result<(), String> {
        let token_bytes = base64::decode(&token).unwrap();
        let token_str = String::from_utf8(token_bytes).unwrap();
        let token: StorageToken = serde_json::from_str(&token_str).unwrap();
        
        let mut signing_key = self.db.get_private_key().unwrap();
        let verifying_key = signing_key.verifying_key();
        let pub_key = verifying_key.to_encoded_point(true);
        let pub_key_hex = hex::encode(pub_key.as_bytes());

        let status = pub_key_hex == token.storage_provider;

        let response = TransportPacket {
            act: "storage_valid_token_response".to_string(),
            to: Some(from_uuid),
            data: Some(TransportData::StorageValidTokenResponse(
                StorageValidTokenResponse {
                    peer_id: self.db.get_or_create_peer_id().unwrap(),
                    status,
                },
            )),
            protocol: Protocol::SIGNAL,
            uuid: self.db.get_or_create_peer_id().unwrap(),
        };

        connection.send_packet(response).await.map_err(|e| e.to_string())
    }
} 